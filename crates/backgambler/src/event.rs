use crate::{
    data::*,
    order::{Fill, Order},
};

use super::strategy::Decision;

#[derive(Debug, Clone)]
pub enum Event {
    Market(Bar),
    Decision(Decision),
    Order(Order),
    Fill(Fill),
}
