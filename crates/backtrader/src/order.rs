use crate::{data::*, errors::ErrorRepr, portfolio::Position, strategy::DecisionKind};

use super::strategy::Decision;

#[derive(Debug, Clone)]
pub struct Order {
    pub sym: Symbol,
    pub kind: OrderKind,
    pub qty: i32,
    pub time: DateTime,
}

#[derive(Debug, Clone, Copy)]
pub enum OrderKind {
    Market,
    Limit { limit: f32, stop: f32 },
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub sym: Symbol,
    pub qty: i32,
    pub price: f32,
    pub cost: f32,
    pub time: DateTime,
}

pub trait OrderAllocator {
    fn allocate_order(&mut self, decision: &Decision) -> Result<Option<Order>, ErrorRepr>;
}

pub trait OrderManager {
    fn make_order(
        &mut self,
        decision: &Decision,
        position: Option<&Position>,
    ) -> Result<Option<Order>, ErrorRepr>;
}

#[derive(Clone)]
pub struct SimpleOrderManager {
    pub size: i32,
}

impl OrderManager for SimpleOrderManager {
    fn make_order(
        &mut self,
        decision: &Decision,
        position: Option<&Position>,
    ) -> Result<Option<Order>, ErrorRepr> {
        Ok(match decision.kind {
            DecisionKind::Hold => None,
            DecisionKind::Buy => Some(Order {
                time: decision.time,
                qty: self.size,
                sym: decision.sym.clone(),
                kind: OrderKind::Market,
            }),
            DecisionKind::Sell => Some(Order {
                time: decision.time,
                qty: -position
                    .ok_or(ErrorRepr::OutOfBounds("should not sell before buy"))?
                    .qty,
                sym: decision.sym.clone(),
                kind: OrderKind::Market,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_make_order() {
        let time = chrono::Utc::now();
        let strength = 1.0;
        let sym = "test".to_owned();

        let d = Decision {
            time,
            sym: sym.clone(),
            kind: DecisionKind::Hold,
        };
        let mut m = SimpleOrderManager { size: 10 };
        assert!(matches!(m.make_order(&d, None), Ok(None)));

        let d = Decision {
            time,
            sym: sym.clone(),
            kind: DecisionKind::Buy,
        };

        let ord = m
            .make_order(&d, None)
            .expect("should be Ok")
            .expect("should be Some");

        assert_eq!(ord.time, time);
        assert_eq!(ord.sym, sym);
        assert_eq!(ord.qty, 10);
        assert_eq!(ord.time, time);
        assert!(matches!(ord.kind, OrderKind::Market));

        let d = Decision {
            time,
            sym: sym.clone(),
            kind: DecisionKind::Sell,
        };

        m.make_order(&d, None).expect_err("should be Err");

        let mut p = Position::default();
        p.qty = 10;

        let ord = m.make_order(&d, Some(&p)).unwrap().unwrap();
        assert_eq!(ord.time, time);
        assert_eq!(ord.sym, sym);
        assert_eq!(ord.qty, -10);
        assert_eq!(ord.time, time);
        assert!(matches!(ord.kind, OrderKind::Market));
    }
}
