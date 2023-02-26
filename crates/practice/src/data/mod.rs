use std::path::Path;

use backgambler::data::Bar;
use chrono::Utc;
use tushare::TushareBar;

pub fn load_tushare_index_from_csv(path: impl AsRef<Path>) -> Vec<tushare::TushareIndex> {
    let rdr = csv::Reader::from_path(path).unwrap();
    rdr.into_deserialize().map(Result::unwrap).collect()
}

pub fn load_tushare_bar_from_csv(
    path: impl AsRef<Path>,
    start_date: &str,
    end_date: &str,
) -> anyhow::Result<Vec<Bar>> {
    let rdr = csv::Reader::from_path(path)?;
    let start = format!("{} 00:00:00Z", start_date).parse::<chrono::DateTime<Utc>>()?;
    let end = format!("{} 00:00:00Z", end_date).parse::<chrono::DateTime<Utc>>()?;

    Ok(rdr
        .into_deserialize()
        .map(|x| x.unwrap())
        .take_while(|x: &TushareBar| &x.time >= &start)
        .filter(move |x| x.time >= start && x.time <= end)
        .map(|x| x.into())
        .collect())
}

mod tushare {
    use backgambler::data::Bar;
    use chrono::Utc;
    use serde::{Deserialize, Deserializer, Serialize};

    fn datefmt<'de, D>(deserializer: D) -> Result<chrono::DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = format!("{}-{}-{} 00:00:00Z", &s[..4], &s[4..6], &s[6..8]);
        s.parse::<chrono::DateTime<Utc>>()
            .map_err(serde::de::Error::custom)
    }

    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    #[serde(default)]
    pub struct TushareBar {
        #[serde(alias = "ts_code")]
        pub sym: String,
        #[serde(alias = "trade_date")]
        #[serde(deserialize_with = "datefmt")]
        pub time: chrono::DateTime<Utc>,
        pub open: f64,
        pub close: f64,
        pub high: f64,
        pub low: f64,
        pub vol: f64,
    }

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct TushareIndex {
        #[serde(alias = "ts_code")]
        pub sym: String,
        pub name: String,
        pub area: String,
        pub industry: String,
        #[serde(deserialize_with = "datefmt")]
        pub list_date: chrono::DateTime<Utc>,
    }

    impl Into<Bar> for TushareBar {
        fn into(self) -> Bar {
            serde_json::from_str(&serde_json::to_string(&self).unwrap()).unwrap()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use chrono::prelude::*;

        #[test]
        fn test_tushare_csv() {
            let data = "ts_code,trade_date,open,high,low,close,pre_close,change,pct_chg,vol,amount
000001.SZ,20230201,1712.4611,1718.1579,1653.2143,1674.8621,1707.9036,-33.04150000000004,-1.9346,1653421.48,2426471.973";
            let mut rdr = csv::Reader::from_reader(data.as_bytes());
            let mut res = TushareBar::default();
            for r in rdr.deserialize().take(1) {
                res = r.unwrap();
            }

            let bar: Bar = res.into();
            assert_eq!(&bar.sym, "000001.SZ");
            assert_eq!(
                bar.time,
                "2023-02-01T00:00:00Z"
                    .parse::<chrono::DateTime<Utc>>()
                    .unwrap()
            );
            assert_eq!(bar.open, 1712.4611);
            assert_eq!(bar.high, 1718.1579);
            assert_eq!(bar.low, 1653.2143);
            assert_eq!(bar.close, 1674.8621);
            assert_eq!(bar.vol, 1653421.48);
        }

        #[test]
        fn test_date() {
            let d: DateTime<Utc> = "2023-12-01 00:00:00Z".parse().unwrap();
            let d1: DateTime<Utc> = "2023-12-02 00:00:00Z".parse().unwrap();
            assert!(d < d1);
        }
    }
}
