use crate::{data::*, errors::ErrorRepr};

use super::strategy::Decision;

#[derive(Debug)]
pub struct Order {
    pub time: DateTime,
    pub qty: i32,
    pub sym: Symbol,
    pub ty: OrderType,
}

#[derive(Debug)]
pub enum OrderType {
    Market,
    Limit(OrderLimit),
}

#[derive(Debug)]
pub struct OrderLimit {
    pub limit: f32,
    pub stop: f32,
}

#[derive(Debug)]
pub struct Fill {
    pub time: DateTime,
    pub qty: i32,
    pub sym: Symbol,
    pub price: f32,
    pub cost: f32,
}

pub trait OrderMaker {
    fn make_order(&mut self, decision: &Decision) -> Result<Option<Order>, ErrorRepr>;
}

pub trait OrderExecutor {
    fn exec_order(&mut self, order: &Order) -> Result<Fill, ErrorRepr>;
}

pub trait FillHandler {
    fn handle_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr>;
}

pub struct DefaultOrderMaker {
    pub size: i32,
}

impl OrderMaker for DefaultOrderMaker {
    fn make_order(&mut self, decision: &Decision) -> Result<Option<Order>, ErrorRepr> {
        Ok(match decision {
            Decision::Hold => None,
            Decision::Buy(d) => Some(Order {
                time: d.time,
                qty: self.size,
                sym: d.sym.clone(),
                ty: OrderType::Market,
            }),
            Decision::Sell(d) => Some(Order {
                time: d.time,
                qty: -self.size,
                sym: d.sym.clone(),
                ty: OrderType::Market,
            }),
        })
    }
}
