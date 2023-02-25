use crate::{
    data::{Bar, DateTime, Symbol},
    order::{Fill, Order},
};

#[derive(Debug, Clone)]
pub struct Decision {
    pub sym: Symbol,
    pub kind: DecisionKind,
    pub time: DateTime,
}

#[derive(Debug, Clone, Copy)]
pub enum DecisionKind {
    Hold,
    Buy,
    Sell,
}

pub trait DecisionMaker {
    fn make_decision(&mut self, data: &Bar) -> Decision;
    fn on_fill(&mut self, _: &Fill) {}
    fn on_order(&mut self, _: &Order) {}
    fn on_data(&mut self, _: &Bar) {}
}
