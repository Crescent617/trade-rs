use chrono::Utc;
use serde::{Deserialize, Deserializer, Serialize};

pub type DateTime = chrono::DateTime<Utc>;
pub type Symbol = String;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Bar {
    #[serde(rename = "symbol")]
    pub sym: Symbol,
    #[serde(alias = "Date")]
    #[serde(deserialize_with = "datefmt")]
    pub time: DateTime,
    #[serde(alias = "Open")]
    pub open: f32,
    #[serde(alias = "Close")]
    pub close: f32,
    #[serde(alias = "High")]
    pub high: f32,
    #[serde(alias = "Low")]
    pub low: f32,
    #[serde(rename = "volume")]
    #[serde(alias = "Volume")]
    pub vol: u32,
}

fn datefmt<'de, D>(deserializer: D) -> Result<chrono::DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)? + "T00:00:00Z";
    s.parse::<chrono::DateTime<Utc>>()
        .map_err(serde::de::Error::custom)
}

pub trait DataEvtHandler {
    fn on_data(&mut self);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn get_test_data() -> Vec<Bar> {
        let mut rdr = csv::Reader::from_path("src/data/test/orcl-1995-2014.txt").unwrap();
        rdr.deserialize().into_iter().map(|x| x.unwrap()).collect()
    }
}
