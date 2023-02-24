use crate::data::{Bar, DateTime, Symbol};

#[derive(Debug)]
pub enum Decision {
    Hold,
    Buy(DcnData),
    Sell(DcnData),
}

#[derive(Debug)]
pub struct DcnData {
    pub time: DateTime,
    pub sym: Symbol,
    pub strength: f32,
}

pub trait DecisionMaker {
    fn make_decision(&mut self, data: &Bar) -> Decision;
}
