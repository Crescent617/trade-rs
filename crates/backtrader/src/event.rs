use crate::{
    data::*,
    order::{Fill, Order},
};

use super::strategy::Decision;

#[derive(Debug)]
pub enum Event {
    Market(Bar),
    Signal(Decision),
    Order(Order),
    Fill(Fill),
}
