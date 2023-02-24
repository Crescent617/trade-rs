use chrono::Utc;

pub type DateTime = chrono::DateTime<Utc>;
pub type Symbol = String;

#[derive(Debug)]
pub struct Bar {
    pub sym: Symbol,
    pub time: DateTime,
    pub open: f32,
    pub close: f32,
    pub high: f32,
    pub low: f32,
    pub vol: u32,
}
