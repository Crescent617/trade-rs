use derive_builder::Builder;

use crate::{
    data::{Bar, Symbol},
    errors::ErrorRepr,
    order::{Fill, OrderAllocator, OrderManager},
};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Position {
    pub sym: Symbol,
    pub qty: i32,
    pub qty_sold: i32,
    pub qty_bought: i32,
    pub value_sold: f32,
    pub value_bought: f32,
    pub cost: f32,
    pub latest_market: Option<Bar>,
}

impl Position {
    pub(crate) fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
        let (prc, qty, cost) = (fill.price, fill.qty, fill.cost);
        if self.qty + qty < 0 {
            return Err(ErrorRepr::OutOfBounds("no enough quantity"));
        }

        self.qty += qty;
        self.cost += cost;
        let cur_val = prc * qty as f32;

        if qty < 0 {
            self.qty_sold += -qty;
            self.value_sold += -cur_val;
        } else {
            self.qty_bought += qty;
            self.value_bought += cur_val;
        }
        Ok(())
    }

    pub(crate) fn update_from_market(&mut self, data: Bar) {
        self.latest_market.replace(data);
    }

    fn avg_price(&self) -> f32 {
        if self.qty_bought + self.qty_sold > 0 {
            (self.value_sold + self.value_bought) / (self.qty_bought + self.qty_sold) as f32
        } else {
            0.0
        }
    }

    fn pnl(&self) -> f32 {
        self.qty as f32
            * self
                .latest_market
                .as_ref()
                .map_or(self.avg_price(), |x| x.close)
            + self.value_sold
            - self.value_bought
            - self.cost
    }
}

pub trait PositionManager {
    fn update_from_market(&mut self, data: &Bar) -> Result<(), ErrorRepr>;
    fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr>;
}

#[derive(Builder, Clone)]
pub struct SimplePortfolio<T> {
    #[builder(setter(custom))]
    pub init_cash: f32,
    #[builder(setter(custom))]
    pub cash: f32,
    order_manager: T,
    #[builder(setter(skip))]
    pub positions: HashMap<Symbol, Position>,
}

impl<T> SimplePortfolioBuilder<T> {
    pub fn cash(&mut self, value: f32) -> &mut Self {
        self.cash = Some(value);
        self.init_cash = Some(value);
        self
    }
}

impl<T> SimplePortfolio<T> {
    pub fn new(cash: f32, order_maker: T) -> Self {
        Self {
            init_cash: cash,
            cash,
            order_manager: order_maker,
            positions: HashMap::default(),
        }
    }
}

impl<T> PositionManager for SimplePortfolio<T> {
    fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
        let pos = self.positions.entry(fill.sym.clone()).or_insert_with(|| {
            let mut p = Position::default();
            p.sym = fill.sym.clone();
            p
        });

        let cur_val = fill.price * fill.qty as f32;
        if self.cash < cur_val + fill.cost {
            return Err(ErrorRepr::OutOfBounds("no enough cash"));
        }

        pos.update_from_fill(fill)?;

        self.cash -= cur_val + fill.cost;
        Ok(())
    }

    fn update_from_market(&mut self, data: &Bar) -> Result<(), ErrorRepr> {
        if let Some(pos) = self.positions.get_mut(&data.sym) {
            pos.update_from_market(data.clone());
        }
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

pub trait Statistics {
    type Stats;
    fn stats(&self) -> Self::Stats;
}

#[derive(Debug)]
pub struct PortfolioStats {
    pub pnl: f32,
    pub init_cash: f32,
    pub cash: f32,
    pub positions: HashMap<Symbol, Position>,
}

impl<T> Statistics for SimplePortfolio<T> {
    type Stats = PortfolioStats;

    fn stats(&self) -> Self::Stats {
        PortfolioStats {
            pnl: self.positions.values().map(|x| x.pnl()).sum(),
            init_cash: self.init_cash,
            cash: self.cash,
            positions: self.positions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position() {
        let mut pos = Position::default();
        let fill = build_test_fill(-1, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(10, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));
        assert_eq!(
            pos,
            Position {
                sym: "".into(),
                qty: 10,
                qty_sold: 0,
                qty_bought: 10,
                value_sold: 0.0,
                value_bought: 100.0,
                cost: 1.0,
                latest_market: None,
            }
        );

        let fill = build_test_fill(-5, 20.0, 2.0);
        let mut bar = Bar::default();
        bar.close = 20.0;
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));

        pos.latest_market.replace(bar.clone());
        assert_eq!(
            pos,
            Position {
                sym: "".into(),
                qty: 5,
                qty_sold: 5,
                qty_bought: 10,
                value_sold: 100.0,
                value_bought: 100.0,
                cost: 3.0,
                latest_market: Some(bar.clone())
            }
        );

        assert_eq!(pos.pnl(), 97.0);

        let fill = build_test_fill(-6, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(-5, 8.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));
        assert_eq!(
            pos,
            Position {
                sym: "".into(),
                qty: 0,
                qty_sold: 10,
                qty_bought: 10,
                value_sold: 140.0,
                value_bought: 100.0,
                cost: 4.0,
                latest_market: Some(bar.clone())
            }
        );
    }

    #[test]
    fn test_portfolio_handle_fill() {
        let mut p = SimplePortfolioBuilder::<Option<()>>::default()
            .cash(50.0)
            .order_manager(None)
            .build()
            .unwrap();

        let fill = build_test_fill(10, 5.0, 1.0);
        assert!(matches!(p.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(10, 5.0, 0.0);
        assert!(matches!(p.update_from_fill(&fill), Ok(_)));
        assert_eq!(p.cash, 0.0);
        assert_eq!(p.init_cash, 50.0);
        assert_eq!(
            p.positions[&fill.sym],
            Position {
                sym: "test".into(),
                qty: 10,
                qty_sold: 0,
                qty_bought: 10,
                value_sold: 0.0,
                value_bought: 50.0,
                cost: 0.0,
                latest_market: None
            }
        );

        let fill = build_test_fill(-5, 6.0, 1.0);

        assert!(matches!(p.update_from_fill(&fill), Ok(_)));
        assert_eq!(p.cash, 29.0);
        assert_eq!(p.init_cash, 50.0);
        assert_eq!(
            p.positions[&fill.sym],
            Position {
                sym: "test".into(),
                qty: 5,
                qty_sold: 5,
                qty_bought: 10,
                value_sold: 30.0,
                value_bought: 50.0,
                cost: 1.0,
                latest_market: None
            }
        );
        assert_eq!(
            p.positions[&fill.sym].pnl(),
            29.0 + 80.0 / 15.0 * 5.0 - 50.0
        );

        let mut bar = Bar::default();
        bar.sym = "test".into();

        p.update_from_market(&bar).unwrap();
        assert_eq!(p.positions[&bar.sym].latest_market, Some(bar));
    }

    fn build_test_fill(qty: i32, price: f32, cost: f32) -> Fill {
        Fill {
            time: chrono::Utc::now(),
            qty,
            sym: "test".into(),
            price,
            cost,
        }
    }
}
