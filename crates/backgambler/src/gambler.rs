use crate::{
    broker::*,
    data::{Bar, Symbol},
    errors::ErrorRepr,
    event::*,
    order::{Fill, Order, OrderAllocator, OrderStatus},
    portfolio::PositionManager,
    strategy::{Decision, DecisionMaker},
};
use derive_builder::Builder;
use parking_lot::Mutex;
use std::{collections::VecDeque, sync::Arc};

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Gambler<Strategy, Data, Broker, Portfolio> {
    #[builder(setter(into))]
    sym: Symbol,
    strategy: Strategy,
    broker: Broker,
    data: Data,
    portfolio: Arc<Mutex<Portfolio>>,
    #[builder(setter(skip))]
    event_q: VecDeque<Event>,
    #[builder(setter(skip))]
    deferred_event_q: VecDeque<Event>,
    #[builder(setter(skip))]
    unfulfilled_orders: Vec<Order>,
    #[builder(default)]
    event_hooks: Vec<Box<dyn Fn(Symbol, &Event) + Send>>,
}

impl<Strategy, Data, Exector, Portfolio> Gambler<Strategy, Data, Exector, Portfolio>
where
    Strategy: DecisionMaker,
    Data: Iterator<Item = Bar>,
    Exector: Broker,
    Portfolio: PositionManager + OrderAllocator + Wallet,
{
    pub fn call_event_hook(&self, event: &Event) {
        for f in &self.event_hooks {
            f(self.sym.clone(), event)
        }
    }

    pub fn add_event_hook<F: Fn(Symbol, &Event) + 'static + Send>(&mut self, f: F) {
        self.event_hooks.push(Box::new(f));
    }

    fn on_data(&mut self, bar: &Bar) {
        let e = Event::Decision(self.strategy.make_decision(bar));
        self.event_q.push_back(e);
    }

    fn on_decision(&mut self, decision: &Decision, is_deferred: bool) {
        let opt = self
            .portfolio
            .lock()
            .allocate_order(decision)
            .expect("allocate_order failed");

        if let Some(ord) = opt {
            self.strategy.on_order(&ord);

            let e = Event::Order(ord);
            if is_deferred {
                self.deferred_event_q.push_back(e);
            } else {
                self.event_q.push_back(e);
            }
        }
    }

    fn on_fill(&mut self, fill: &Fill) {
        let r = self.portfolio.lock().update_from_fill(fill);
        match r {
            Err(err) => self.on_err(err),
            Ok(_) => self.strategy.on_fill(fill),
        }
    }

    fn on_order(&mut self, ord: &mut Order, is_deferred: bool) {
        let mut wallet = self.portfolio.lock();

        let fill = match self.broker.exec_order(ord, &mut *wallet) {
            Ok(f) => f,
            Err(ErrorRepr::NotSatisfied(_)) => {
                let mut ord = ord.clone();
                ord.lifetime = ord.lifetime.map(|x| x.saturating_sub(1));
                return self.unfulfilled_orders.push(ord.to_owned());
            }
            Err(ErrorRepr::OrderExpired(_)) => {
                ord.status = OrderStatus::Expired;
                self.strategy.on_order(ord);
                return;
            }
            Err(err) => panic!("Unhandled ERROR: {:?}", err),
        };

        ord.status = OrderStatus::Completed;
        self.strategy.on_order(ord);

        let e = Event::Fill(fill);
        if is_deferred {
            self.deferred_event_q.push_back(e);
        } else {
            self.event_q.push_back(e);
        }
    }

    fn enqueue_unfulfilled_orders(&mut self) {
        while let Some(ord) = self.unfulfilled_orders.pop() {
            self.deferred_event_q.push_back(Event::Order(ord));
        }
    }

    fn on_err(&mut self, err: ErrorRepr) {
        log::error!("{}", err);
    }

    pub async fn run(&mut self) {
        'outer: loop {
            match self.data.next() {
                Some(bar) => {
                    self.event_q.push_back(Event::Market(bar));
                }
                _ => break 'outer,
            }

            self.enqueue_unfulfilled_orders();

            while let Some(mut evt) = self.event_q.pop_front() {
                match &mut evt {
                    Event::Market(bar) => {
                        // update before the deferred queue
                        self.broker.set_lastest_bar(bar);
                        self.portfolio
                            .lock()
                            .update_from_market(bar)
                            .expect("update position failed");
                        self.strategy.on_data(bar);

                        while let Some(mut evt) = self.deferred_event_q.pop_front() {
                            match &mut evt {
                                Event::Order(ord) => self.on_order(ord, true),
                                Event::Fill(fill) => self.on_fill(fill),
                                _ => unreachable!(),
                            }
                            self.call_event_hook(&evt);
                        }

                        // update after the deferred queue
                        self.on_data(bar)
                    }
                    Event::Decision(d) => self.on_decision(d, true),
                    Event::Order(ord) => self.on_order(ord, false),
                    Event::Fill(fill) => self.on_fill(fill),
                }
                self.call_event_hook(&evt);
            }
        }
    }
}

pub struct Casino<A, B, C, D> {
    gamblers: Vec<Gambler<A, B, C, D>>,
}

impl<Strategy, Data, Exector, Portfolio> Casino<Strategy, Data, Exector, Portfolio>
where
    Strategy: DecisionMaker + Send + 'static,
    Data: Iterator<Item = Bar> + Send + 'static,
    Exector: Broker + Send + 'static,
    Portfolio: PositionManager + OrderAllocator + Wallet + Send + 'static,
{
    pub fn new(gamblers: Vec<Gambler<Strategy, Data, Exector, Portfolio>>) -> Self {
        Self { gamblers }
    }

    pub async fn run(&mut self) {
        let mut join_handlers = tokio::task::JoinSet::new();

        while let Some(mut g) = self.gamblers.pop() {
            join_handlers.spawn(async move {
                g.run().await;
            });
        }

        while let Some(res) = join_handlers.join_next().await {
            res.unwrap();
        }
    }
}
