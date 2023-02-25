use crate::{
    data::{Bar, Symbol},
    errors::ErrorRepr,
    event::*,
    order::{Fill, Order, OrderAllocator},
    portfolio::PositionManager,
    strategy::{Decision, DecisionMaker},
};
use derive_builder::Builder;
use parking_lot::Mutex;
use std::sync::Arc;
use std::{collections::VecDeque, future::Future};

#[derive(Builder)]
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
    event_hooks: Vec<Box<dyn Fn(Symbol, &Event) + Send>>,
}

impl<Strategy, Data, Exector, Portfolio> Gambler<Strategy, Data, Exector, Portfolio>
where
    Strategy: DecisionMaker,
    Data: Iterator<Item = Bar>,
    Exector: Broker,
    Portfolio: PositionManager + OrderAllocator,
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
            let e = Event::Order(ord);
            if is_deferred {
                self.deferred_event_q.push_back(e);
            } else {
                self.event_q.push_back(e);
            }
        }
    }

    fn on_fill(&mut self, fill: &Fill) {
        match self.portfolio.lock().update_from_fill(fill) {
            Err(err) => self.event_q.push_back(Event::Error(err)),
            Ok(_) => {
                self.strategy.on_fill(fill);
            }
        };
    }

    fn on_order(&mut self, ord: &Order, is_deferred: bool) {
        let fill = self.broker.exec_order(&ord).expect("exec_order failed");
        let e = Event::Fill(fill);

        if is_deferred {
            self.deferred_event_q.push_back(e);
        } else {
            self.event_q.push_back(e);
        }
        self.strategy.on_order(ord);
    }

    fn on_err(&mut self, _: &ErrorRepr) {}

    pub async fn run(&mut self) {
        'outer: loop {
            match self.data.next() {
                Some(bar) => {
                    self.event_q.push_back(Event::Market(bar));
                }
                _ => break 'outer,
            }

            while let Some(evt) = self.event_q.pop_front() {
                self.call_event_hook(&evt);
                match &evt {
                    Event::Market(bar) => {
                        // update before the deferred queue
                        self.broker.set_lastest_bar(bar);
                        self.portfolio
                            .lock()
                            .update_from_market(bar)
                            .expect("update position failed");
                        self.strategy.on_data(bar);

                        while let Some(evt) = self.deferred_event_q.pop_front() {
                            self.call_event_hook(&evt);
                            match &evt {
                                Event::Order(ord) => self.on_order(ord, true),
                                Event::Fill(fill) => self.on_fill(fill),
                                _ => unreachable!(),
                            }
                        }

                        // update after the deferred queue
                        self.on_data(bar)
                    }
                    Event::Decision(d) => self.on_decision(d, true),
                    Event::Error(err) => self.on_err(err),
                    // Event::Order(ord) => self.on_order(&ord, false),
                    // Event::Fill(fill) => self.on_fill(&fill),
                    _ => unreachable!(),
                }
            }
        }
    }
}

pub trait Broker {
    fn exec_order(&mut self, order: &Order) -> Result<Fill, ErrorRepr>;

    /// just for back test
    fn set_lastest_bar(&mut self, bar: &Bar);
}

#[derive(Clone)]
pub struct SimulatedBroker {
    pub latest: Option<Bar>,
    pub commission: f32,
}

impl Broker for SimulatedBroker {
    fn exec_order(&mut self, order: &Order) -> Result<Fill, ErrorRepr> {
        let bar = self
            .latest
            .as_ref()
            .ok_or(ErrorRepr::NotExists("latest price"))?;
        let price = bar.open;
        let cost = order.qty.abs() as f32 * price * self.commission;
        Ok(Fill {
            time: bar.time,
            qty: order.qty,
            sym: order.sym.clone(),
            price,
            cost,
        })
    }

    fn set_lastest_bar(&mut self, bar: &Bar) {
        self.latest.replace(bar.clone());
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
    Portfolio: PositionManager + OrderAllocator + Send + 'static,
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
