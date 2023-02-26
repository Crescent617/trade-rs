use derive_builder::Builder;
use serde::Serialize;

use crate::{data::*, errors::ErrorRepr, position::Position, strategy::DecisionKind};

use super::strategy::Decision;

#[derive(Debug, Clone, Builder)]
pub struct Order {
    #[builder(default)]
    pub sym: Symbol,
    #[builder(default = "OrderKind::Market")]
    pub kind: OrderKind,
    #[builder(default)]
    pub qty: i32,
    #[builder(default = "chrono::Utc::now()")]
    pub time: DateTime,
    #[builder(default)]
    pub lifetime: Option<usize>,
    #[builder(default)]
    pub status: OrderStatus,
}

impl Order {
    pub fn is_expired(&self) -> bool {
        self.lifetime == Some(0)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum OrderKind {
    Market,
    Limit { limit: f64, stop: Option<f64> },
}

#[derive(Debug, Clone, Copy, Default)]
pub enum OrderStatus {
    #[default]
    Created,
    Completed,
    PartialCompleted,
    Expired,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fill {
    pub sym: Symbol,
    pub qty: i32,
    pub price: f64,
    pub cost: f64,
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
pub struct FixedValueOrderManager {
    pub val: f64,
}

impl OrderManager for FixedValueOrderManager {
    fn make_order(
        &mut self,
        decision: &Decision,
        position: Option<&Position>,
    ) -> Result<Option<Order>, ErrorRepr> {
        use DecisionKind::*;

        let mut b = OrderBuilder::default();

        match decision.kind {
            Buy => {
                let price = position.unwrap().latest_market_close.unwrap();
                b.qty((self.val / price).floor() as i32);
            }
            Sell | Close => {
                let current = position.map_or(0, |x| x.qty);
                b.qty(-current);
            }
            _ => return Ok(None),
        }

        b.time(decision.time).sym(decision.sym.clone());

        let ord = b.build().unwrap();
        Ok(if ord.qty != 0 {
            Some(ord)
        } else {
            log::warn!("cannot make order with qty == 0. order: {:?}", ord);
            None
        })
    }
}

#[derive(Clone)]
pub struct FixedSizeOrderManager {
    pub size: i32,
}

impl OrderManager for FixedSizeOrderManager {
    fn make_order(
        &mut self,
        decision: &Decision,
        position: Option<&Position>,
    ) -> Result<Option<Order>, ErrorRepr> {
        use DecisionKind::*;

        let mut b = OrderBuilder::default();
        let current = position.map_or(0, |x| x.qty);

        match decision.kind {
            Buy => {
                b.qty(self.size);
            }
            Sell => {
                b.qty(-self.size.min(current));
            }
            Close => {
                b.qty(-current);
            }
            _ => return Ok(None),
        }

        b.time(decision.time).sym(decision.sym.clone());

        let ord = b.build().unwrap();
        Ok(if ord.qty != 0 {
            Some(ord)
        } else {
            log::warn!("cannot make order with qty == 0");
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_make_order() {
        let time = chrono::Utc::now();
        let sym = "test".to_owned();

        let d = Decision {
            time,
            sym: sym.clone(),
            kind: DecisionKind::Hold,
        };
        let mut m = FixedSizeOrderManager { size: 10 };
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
