use crate::{
    data::Symbol,
    errors::ErrorRepr,
    event::*,
    order::{Fill, FillHandler},
};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Position {
    pub sym: Symbol,
    pub qty: i32,
    pub qty_sold: i32,
    pub qty_bought: i32,
    pub value_sold: f32,
    pub value_bought: f32,
    pub cost: f32,
    pub value: f32,
    pub pnl: f32,
}

impl Position {
    fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
        let (prc, qty, cost) = (fill.price, fill.qty, fill.cost);
        if self.qty < qty {
            return Err(ErrorRepr::OutOfBounds("no enough quantity"));
        }

        let cur_val = prc * qty as f32;
        self.qty += qty;
        self.value += cur_val;
        self.cost += cost;

        if qty < 0 {
            self.qty_sold += -qty;
            self.value_sold += -cur_val;
        } else {
            self.qty_bought += qty;
            self.value_bought += cur_val;
        }
        Ok(())
    }

    fn pnl(&self) -> f32 {
        self.value + self.value_sold - self.value_bought - self.cost
    }
}

pub struct SimplePortfolio<T> {
    init_cash: f32,
    cash: f32,
    order_maker: T,
    positions: HashMap<Symbol, Position>,
    event_hook: Vec<Box<dyn Fn(Event)>>,
}

impl<T> SimplePortfolio<T> {
    pub fn new(
        cash: f32,
        order_maker: T,
        positions: HashMap<Symbol, Position>,
        event_hook: Vec<Box<dyn Fn(Event)>>,
    ) -> Self {
        Self {
            init_cash: cash,
            cash,
            order_maker,
            positions,
            event_hook,
        }
    }
}

impl<Maker> FillHandler for SimplePortfolio<Maker> {
    fn handle_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
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
}

#[cfg(test)]
mod tests {
    // 注意这个惯用法：在 tests 模块中，从外部作用域导入所有名字。
    use super::*;

    #[test]
    fn test_position() {
        let mut pos = Position::default();
        let fill = get_test_fill();
        assert!(matches!(pos.update_from_fill(&fill), Err(_)));
    }

    fn get_test_fill() -> Fill {
        Fill {
            time: chrono::Utc::now(),
            qty: 10,
            sym: "test".into(),
            price: 10.0,
            cost: 1.0,
        }
    }
}
