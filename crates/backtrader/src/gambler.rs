use crate::{data::Symbol, event::*};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Gambler<Strategy, Data, Exector, Portfolio> {
    sym: Symbol,
    strategy: Strategy,
    executor: Exector,
    data: Data,
    portfolio: Arc<Mutex<Portfolio>>,
    event_q: VecDeque<Event>,
    event_hook: Vec<Box<dyn Fn(Event)>>,
}

pub struct Casino {}
