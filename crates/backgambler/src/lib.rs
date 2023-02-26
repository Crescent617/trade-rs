pub mod broker;
pub mod data;
pub mod errors;
pub mod event;
pub mod gambler;
pub mod order;
pub mod portfolio;
pub mod strategy;
pub mod position;

#[cfg(test)]
mod tests {
    use more_asserts::*;
    use std::sync::Arc;

    use parking_lot::Mutex;

    use crate::{
        data::Bar, gambler::Casino, portfolio::Statistics,
        strategy::DecisionMaker,
    };

    use super::*;

    #[derive(Clone)]
    struct TestStrategy {
        idx: i32,
    }

    impl DecisionMaker for TestStrategy {
        fn make_decision(&mut self, data: &Bar) -> strategy::Decision {
            self.idx += 1;
            strategy::Decision {
                time: data.time,
                sym: data.sym.clone(),
                kind: if self.idx % 2 == 1 {
                    strategy::DecisionKind::Buy
                } else {
                    strategy::DecisionKind::Sell
                },
            }
        }
    }

    #[tokio::test]
    async fn test_gambler() {
        let portfolio = portfolio::SimplePortfolioBuilder::default()
            .order_manager(order::FixedSizeOrderManager { size: 100 })
            .cash(10000.0)
            .build()
            .unwrap();

        let portfolio = Arc::new(Mutex::new(portfolio));
        let bars = vec![
            build_bar(5.0, 6.0),
            build_bar(7.0, 8.0),
            build_bar(1.0, 2.0),
            build_bar(1.0, 2.0),
        ];

        let mut g = gambler::GamblerBuilder::default()
            .sym("test")
            .strategy(TestStrategy { idx: 0 })
            .data(bars.into_iter())
            .broker(
                broker::SimulatedBrokerBuilder::default()
                    .commission(0.001)
                    .build()
                    .unwrap(),
            )
            .portfolio(Arc::clone(&portfolio))
            .build()
            .unwrap();
        g.add_event_hook(|s, evt| println!(">>> ({}) event: {:?}", s, evt));
        g.run().await;

        let p = portfolio.lock();
        assert_eq!(p.init_cash, 10_000.0);
        assert_le!(
            (10_000.0 - 700.0 + 100.0 - 100.0 - 0.9 - p.cash).abs(),
            0.01
        );
    }

    #[tokio::test]
    async fn test_casino() {
        let portfolio = portfolio::SimplePortfolioBuilder::default()
            .order_manager(order::FixedSizeOrderManager { size: 100 })
            .cash(10000.0)
            .build()
            .unwrap();

        let portfolio = Arc::new(Mutex::new(portfolio));
        let bars = vec![
            build_bar(5.0, 6.0),
            build_bar(7.0, 8.0),
            build_bar(1.0, 2.0),
            build_bar(1.0, 2.0),
        ];

        let mut g = gambler::GamblerBuilder::default()
            .sym("test")
            .strategy(TestStrategy { idx: 0 })
            .data(bars.into_iter())
            .broker(
                broker::SimulatedBrokerBuilder::default()
                    .commission(0.001)
                    .build()
                    .unwrap(),
            )
            .portfolio(Arc::clone(&portfolio))
            .build()
            .unwrap();

        g.add_event_hook(|s, evt| println!(">>> ({}) event: {:?}", s, evt));

        let mut casino = Casino::new(vec![g]);
        casino.run().await;

        let p = portfolio.lock();
        assert_eq!(p.init_cash, 10_000.0);
        assert_le!(
            (10_000.0 - 700.0 + 100.0 - 100.0 - 0.9 - p.cash).abs(),
            0.01
        );
    }

    #[derive(Clone, Default, Debug)]
    struct TestStrategy2 {
        pending_ord: i32,
        prev_close: std::collections::VecDeque<f64>,
        idx: i32,
        bar_executed: i32,
        qty: i32,
    }

    impl DecisionMaker for TestStrategy2 {
        fn make_decision(&mut self, data: &Bar) -> strategy::Decision {
            let mut d = strategy::Decision {
                time: data.time,
                sym: data.sym.clone(),
                kind: strategy::DecisionKind::Hold,
            };

            if self.pending_ord != 0 {
                return d;
            }

            if self.qty == 0 {
                let n = self.prev_close.len();
                if n >= 3 {
                    if self.prev_close[n - 2] > self.prev_close[n - 1]
                        && self.prev_close[n - 3] > self.prev_close[n - 2]
                    {
                        d.kind = strategy::DecisionKind::Buy;
                        println!("BUY created, close: {:.2}, debug: {:?}\n", data.close, self);
                    }
                }
            } else {
                if self.idx >= self.bar_executed + 5 {
                    d.kind = strategy::DecisionKind::Sell;
                    println!(
                        "SELL created, close: {:.2}, debug: {:?}\n",
                        data.close, self
                    );
                }
            }

            d
        }

        fn on_data(&mut self, data: &Bar) {
            self.idx += 1;
            self.prev_close.push_back(data.close);
            while self.prev_close.len() > 3 {
                self.prev_close.pop_front();
            }
        }

        fn on_order(&mut self, ord: &order::Order) {
            use order::OrderStatus::*;
            match ord.status {
                Created => self.pending_ord += 1,
                _ => self.pending_ord -= 1,
            }
        }

        fn on_fill(&mut self, fill: &order::Fill) {
            self.bar_executed = self.idx;
            self.qty += fill.qty;
            if fill.qty > 0 {
                println!("BUY executed, fill: {0:.2}", fill.price);
            } else {
                println!("SELL executed, fill: {0:.2}", fill.price,);
            }
        }
    }

    #[tokio::test]
    async fn test_real_data() {
        let portfolio = portfolio::SimplePortfolioBuilder::default()
            .order_manager(order::FixedSizeOrderManager { size: 1 })
            .cash(100000.0)
            .build()
            .unwrap();

        let portfolio = Arc::new(Mutex::new(portfolio));
        let bars = data::tests::get_test_data();

        let mut g = gambler::GamblerBuilder::default()
            .sym("test")
            .strategy(TestStrategy2::default())
            .data(bars.iter().cloned())
            .broker(broker::SimulatedBrokerBuilder::default().build().unwrap())
            .portfolio(Arc::clone(&portfolio))
            .build()
            .unwrap();

        // g.add_event_hook(|s, evt| {
        //     if matches!(evt, event::Event::Market(_)) {
        //         println!("EVENT ({}): {:?}", s, evt);
        //     }
        // });

        g.run().await;

        let p = portfolio.lock();
        let stats = p.stats();
        stats.printstd();

        // calculated by py backtrader
        assert_eq!(((stats.init_cash + stats.pnl) * 100.0).round(), 10001968.0);
    }

    fn build_bar(open: f64, close: f64) -> Bar {
        Bar {
            sym: "test".into(),
            time: chrono::Utc::now(),
            open,
            close,
            high: 0.0,
            low: 0.0,
            vol: 10000.0,
        }
    }
}
