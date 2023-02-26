use derive_builder::Builder;
use serde::Serialize;

use crate::{
    broker::Wallet,
    data::{Bar, Symbol},
    errors::ErrorRepr,
    order::{Fill, OrderAllocator, OrderManager},
    position::Position,
};
use std::collections::HashMap;

pub trait PositionManager {
    fn update_from_market(&mut self, data: &Bar) -> Result<(), ErrorRepr>;
    fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr>;
}

#[derive(Builder, Clone)]
pub struct SimplePortfolio<T> {
    #[builder(setter(custom))]
    pub init_cash: f64,
    #[builder(setter(custom))]
    pub cash: f64,
    // TODO extract order_manager
    order_manager: T,
    #[builder(setter(skip))]
    pub positions: HashMap<Symbol, Position>,
}

impl<T> SimplePortfolioBuilder<T> {
    pub fn cash(&mut self, value: f64) -> &mut Self {
        self.cash = Some(value);
        self.init_cash = Some(value);
        self
    }
}

impl<T> SimplePortfolio<T> {
    fn get_position_mut(&mut self, sym: &str) -> &mut Position {
        self.positions.entry(sym.to_owned()).or_insert_with(|| {
            let mut p = Position::default();
            p.sym = sym.to_owned();
            p
        })
    }
}

impl<T> PositionManager for SimplePortfolio<T> {
    fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
        let pos = self.get_position_mut(&fill.sym);
        pos.update_from_fill(fill)
    }

    fn update_from_market(&mut self, data: &Bar) -> Result<(), ErrorRepr> {
        let pos = self.get_position_mut(&data.sym);
        pos.update_from_market(data.clone());
        Ok(())
    }
}

impl<T: OrderManager> OrderAllocator for SimplePortfolio<T> {
    fn allocate_order(
        &mut self,
        decision: &crate::strategy::Decision,
    ) -> Result<Option<crate::order::Order>, ErrorRepr> {
        self.order_manager
            .make_order(decision, self.positions.get(&decision.sym))
    }
}

impl<T> Wallet for SimplePortfolio<T> {
    fn balance(&self) -> f64 {
        self.cash
    }
    fn set_balance(&mut self, money: f64) {
        self.cash = money;
    }
}

pub trait Statistics {
    type Stats;
    fn stats(&self) -> Self::Stats;
}

#[derive(Debug, Default, Serialize)]
pub struct PortfolioStats {
    pub pnl: f64,
    pub init_cash: f64,
    pub cash: f64,
    pub pnl_ratio: f64,
    pub positions: Vec<Position>,
}

impl PortfolioStats {
    pub fn printstd(&self) {
        println!("{:#?}", self);
    }
}

impl<T> Statistics for SimplePortfolio<T> {
    type Stats = PortfolioStats;

    fn stats(&self) -> Self::Stats {
        let mut positions = self.positions.values().cloned().collect::<Vec<_>>();
        positions.sort_by(|a, b| b.stats.pnl_ratio.partial_cmp(&a.stats.pnl_ratio).unwrap());

        let pnl = positions.iter().map(|x| x.stats.pnl).sum();
        PortfolioStats {
            pnl,
            init_cash: self.init_cash,
            cash: self.cash,
            pnl_ratio: pnl / self.init_cash,
            positions,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_portfolio_handle_fill() {
        let mut p = SimplePortfolioBuilder::<Option<()>>::default()
            .cash(50.0)
            .order_manager(None)
            .build()
            .unwrap();

        // let fill = build_test_fill(10, 5.0, 1.0);
        // assert!(matches!(p.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(10, 5.0, 0.0);
        assert!(matches!(p.update_from_fill(&fill), Ok(_)));
        // assert_eq!(p.cash, 0.0);
        assert_eq!(p.init_cash, 50.0);

        let pos = &p.positions[&fill.sym];
        assert_eq!(pos.qty, 10);
        assert_eq!(pos.stats.qty_sold, 0);
        assert_eq!(pos.stats.qty_bought, 10);
        assert_eq!(pos.stats.value_sold, 0.0);
        assert_eq!(pos.stats.value_bought, 50.0);
        assert_eq!(pos.stats.cost, 0.0);

        let fill = build_test_fill(-5, 6.0, 1.0);

        assert!(matches!(p.update_from_fill(&fill), Ok(_)));
        // assert_eq!(p.cash, 29.0);
        let pos = &p.positions[&fill.sym];
        assert_eq!(p.init_cash, 50.0);
        assert_eq!(pos.qty, 5);
        assert_eq!(pos.stats.qty_sold, 5);
        assert_eq!(pos.stats.qty_bought, 10);
        assert_eq!(pos.stats.value_sold, 30.0);
        assert_eq!(pos.stats.value_bought, 50.0);
        assert_eq!(pos.stats.cost, 1.0);
        assert_eq!(
            p.positions[&fill.sym].pnl(),
            29.0 + 80.0 / 15.0 * 5.0 - 50.0
        );

        let stats = p.stats();
        assert_eq!(p.positions[&fill.sym].pnl(), stats.pnl);

        let mut bar = Bar::default();
        bar.sym = "test".into();

        p.update_from_market(&bar).unwrap();
        assert_eq!(p.positions[&bar.sym].latest_market_close, Some(bar.close));
    }

    fn build_test_fill(qty: i32, price: f64, cost: f64) -> Fill {
        Fill {
            time: chrono::Utc::now(),
            qty,
            sym: "test".into(),
            price,
            cost,
        }
    }
}
