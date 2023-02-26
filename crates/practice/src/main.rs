pub mod data;

use std::sync::Arc;

use backgambler::{
    broker, gambler, order,
    portfolio::{self, Statistics},
    strategy,
};
use data::{load_tushare_bar_from_csv, load_tushare_index_from_csv};
use log::{debug, error, info, warn};
use parking_lot::Mutex;
use ta::Next;

#[derive(Clone)]
struct MyStrategy {
    ma: ta::indicators::SimpleMovingAverage,
    ma2: ta::indicators::SimpleMovingAverage,
    pending_ord: i32,
    qty: i32,
}

impl MyStrategy {
    fn new() -> Self {
        Self {
            ma: ta::indicators::SimpleMovingAverage::new(5).unwrap(),
            ma2: ta::indicators::SimpleMovingAverage::new(20).unwrap(),
            pending_ord: 0,
            qty: 0,
        }
    }
}

impl strategy::DecisionMaker for MyStrategy {
    fn make_decision(&mut self, data: &backgambler::data::Bar) -> strategy::Decision {
        let ma = self.ma.next(data.close);
        let ma2 = self.ma2.next(data.close);

        let mut d = strategy::Decision {
            sym: data.sym.clone(),
            kind: strategy::DecisionKind::Hold,
            time: data.time,
        };

        if self.pending_ord > 0 {
            return d;
        }

        if self.qty == 0 && ma > ma2 {
            d.kind = strategy::DecisionKind::Buy;
        }

        if self.qty > 0 && ma < ma2 {
            d.kind = strategy::DecisionKind::Sell;
        }
        d
    }

    fn on_order(&mut self, ord: &order::Order) {
        use order::OrderStatus::*;
        match ord.status {
            Created => {
                self.pending_ord += 1;
                debug!("ORDER CREATE: {:#?}", ord);
            }
            _ => {
                debug!("ORDER OVER: {:#?}", ord);
                self.pending_ord -= 1;
            }
        }
    }

    fn on_fill(&mut self, f: &order::Fill) {
        self.qty += f.qty;
        info!("FILL EVENT: {:#?}", f);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let data_path = std::path::Path::new("/home/hrli/GitHub/trade/data/a-shares");
    let start_date = "2022-01-01";
    let end_date = "2022-12-31";

    let index = load_tushare_index_from_csv(data_path.join("index.csv"));

    let mut bars_list = vec![];
    for idx in index.iter().filter(|x| !x.name.contains("ST")) {
        let res = load_tushare_bar_from_csv(
            data_path.join(idx.sym.to_owned() + ".csv"),
            start_date,
            end_date,
        );

        match res {
            Ok(bars) if bars.len() > 0 => bars_list.push(bars),
            Ok(_) => warn!("empty data: {}.csv", idx.sym),
            Err(err) => error!("load {}.csv fail: {}", idx.sym, err),
        }
    }

    let cash = 1_000_000_000.0;
    let portfolio = portfolio::SimplePortfolioBuilder::default()
        .order_manager(order::FixedValueOrderManager {
            val: cash / bars_list.len() as f64,
        })
        .cash(cash)
        .build()
        .unwrap();

    let portfolio = Arc::new(Mutex::new(portfolio));
    // let portfolio_ = Arc::clone(&portfolio);
    // let hook = move |_, evt: &event::Event| {
    //     if let event::Event::Fill(f) = evt {
    //         info!("EVENT FILL: {}", f.time);
    //         portfolio_.lock().stats().printstd();
    //     }
    // };

    let mut gamblers = Vec::with_capacity(bars_list.len());
    for bars in bars_list {
        info!("create bars: {}", bars[0].sym.clone());
        let g = gambler::GamblerBuilder::default()
            .sym(bars[0].sym.clone())
            .strategy(MyStrategy::new())
            .data(bars.into_iter().rev())
            .broker(
                broker::SimulatedBrokerBuilder::default()
                    .commission(0.001)
                    .build()
                    .unwrap(),
            )
            .portfolio(Arc::clone(&portfolio))
            // .event_hooks(vec![Box::new(hook)])
            .build()
            .unwrap();
        gamblers.push(g);
    }

    let mut casino = gambler::Casino::new(gamblers);
    casino.run().await;

    let p = portfolio.lock();
    p.stats().printstd();
    Ok(())
}
