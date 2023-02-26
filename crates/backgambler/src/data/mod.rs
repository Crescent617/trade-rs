use chrono::Utc;
use serde::{Deserialize, Serialize};

pub type DateTime = chrono::DateTime<Utc>;
pub type Symbol = String;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bar {
    pub sym: Symbol,
    pub time: DateTime,
    pub open: f64,
    pub close: f64,
    pub high: f64,
    pub low: f64,
    pub vol: f64,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize};

    fn datefmt<'de, D>(deserializer: D) -> Result<chrono::DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)? + "T00:00:00Z";
        s.parse::<chrono::DateTime<Utc>>()
            .map_err(serde::de::Error::custom)
    }

    #[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
    #[serde(default)]
    pub struct TestBar {
        pub sym: Symbol,
        #[serde(alias = "Date")]
        #[serde(deserialize_with = "datefmt")]
        pub time: DateTime,
        #[serde(alias = "Open")]
        pub open: f64,
        #[serde(alias = "Close")]
        pub close: f64,
        #[serde(alias = "High")]
        pub high: f64,
        #[serde(alias = "Low")]
        pub low: f64,
        #[serde(alias = "Volume")]
        pub vol: u32,
    }

    impl Into<Bar> for TestBar {
        fn into(self) -> Bar {
            serde_json::from_str(&serde_json::to_string(&self).unwrap()).unwrap()
        }
    }

    pub fn get_test_data() -> Vec<Bar> {
        let mut rdr = csv::Reader::from_path("src/data/test/orcl-1995-2014.txt").unwrap();
        rdr.deserialize()
            .into_iter()
            .map(|x| x.unwrap())
            .map(|x: TestBar| x.into())
            .collect()
    }
}
