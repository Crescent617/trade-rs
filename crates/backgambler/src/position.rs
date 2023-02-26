use serde::Serialize;

use crate::{
    data::{Bar, Symbol},
    errors::ErrorRepr,
    order::Fill,
};

#[derive(Clone, Debug, Default, Serialize)]
pub struct Position {
    pub sym: Symbol,
    pub qty: i32,
    pub latest_market_close: Option<f64>,
    #[serde(flatten)]
    pub stats: PositionStats,
}

#[derive(Debug, Clone, Serialize)]
pub struct PositionStats {
    pub pnl: f64,
    pub pnl_ratio: f64,
    pub max_pnl: f64,
    pub min_pnl: f64,
    pub qty_sold: i32,
    pub qty_bought: i32,
    pub value_sold: f64,
    pub value_bought: f64,
    pub cost: f64,
    pub max_cash: f64,
    pub transactions: Vec<Fill>,
}

impl Default for PositionStats {
    fn default() -> Self {
        Self {
            pnl: 0.0,
            pnl_ratio: 0.0,
            max_pnl: f64::MIN,
            min_pnl: f64::MAX,
            qty_sold: 0,
            qty_bought: 0,
            value_sold: 0.0,
            value_bought: 0.0,
            cost: 0.0,
            max_cash: 0.0,
            transactions: vec![],
        }
    }
}

impl PositionStats {
    fn avg_price(&self) -> f64 {
        if self.qty_bought + self.qty_sold > 0 {
            (self.value_sold + self.value_bought) / (self.qty_bought + self.qty_sold) as f64
        } else {
            0.0
        }
    }

    fn update_from_fill(&mut self, fill: &Fill) {
        self.transactions.push(fill.clone());
        let (prc, qty, cost) = (fill.price, fill.qty, fill.cost);
        self.cost += cost;
        let cur_val = prc * qty as f64;

        if qty < 0 {
            self.qty_sold += -qty;
            self.value_sold += -cur_val;
        } else {
            self.qty_bought += qty;
            self.value_bought += cur_val;
            self.max_cash = self.max_cash.max(cur_val + cost - self.pnl);
        }
    }

    fn update_pnl(&mut self, pnl: f64) {
        self.pnl = pnl;
        self.min_pnl = self.min_pnl.min(pnl);
        self.max_pnl = self.max_pnl.max(pnl);
        if self.max_cash != 0.0 {
            self.pnl_ratio = self.pnl / self.max_cash
        }
    }
}

impl Position {
    pub fn update_from_fill(&mut self, fill: &Fill) -> Result<(), ErrorRepr> {
        let qty = fill.qty;
        if self.qty + qty < 0 {
            return Err(ErrorRepr::OutOfBounds(format!(
                "no enough quantity. current: {:.2}, need: {:.2}",
                self.qty, qty
            )));
        }
        self.qty += qty;
        self.stats.update_from_fill(fill);
        self.stats.update_pnl(self.pnl());
        Ok(())
    }

    pub fn update_from_market(&mut self, data: Bar) {
        self.latest_market_close.replace(data.close);
        self.stats.update_pnl(self.pnl());
    }

    pub fn pnl(&self) -> f64 {
        self.qty as f64 * self.latest_market_close.unwrap_or(self.stats.avg_price())
            + self.stats.value_sold
            - self.stats.value_bought
            - self.stats.cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_fill(qty: i32, price: f64, cost: f64) -> Fill {
        Fill {
            time: chrono::Utc::now(),
            qty,
            sym: "test".into(),
            price,
            cost,
        }
    }

    #[test]
    fn test_position() {
        let mut pos = Position::default();
        let fill = build_test_fill(-1, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(10, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));
        assert_eq!(pos.qty, 10);
        assert_eq!(pos.stats.qty_sold, 0);
        assert_eq!(pos.stats.qty_bought, 10);
        assert_eq!(pos.stats.value_sold, 0.0);
        assert_eq!(pos.stats.value_bought, 100.0);
        assert_eq!(pos.stats.cost, 1.0);
        assert_eq!(pos.latest_market_close, None);

        let fill = build_test_fill(-5, 20.0, 2.0);
        let mut bar = Bar::default();
        bar.close = 20.0;
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));

        pos.latest_market_close.replace(bar.close);
        assert_eq!(pos.qty, 5);
        assert_eq!(pos.stats.qty_sold, 5);
        assert_eq!(pos.stats.qty_bought, 10);
        assert_eq!(pos.stats.value_sold, 100.0);
        assert_eq!(pos.stats.value_bought, 100.0);
        assert_eq!(pos.stats.cost, 3.0);
        assert_eq!(pos.latest_market_close, Some(bar.close));

        assert_eq!(pos.pnl(), 97.0);

        let fill = build_test_fill(-6, 10.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Err(_)));

        let fill = build_test_fill(-5, 8.0, 1.0);
        assert!(matches!(pos.update_from_fill(&fill), Ok(_)));
        assert_eq!(pos.qty, 0);
        assert_eq!(pos.stats.qty_sold, 10);
        assert_eq!(pos.stats.qty_bought, 10);
        assert_eq!(pos.stats.value_sold, 140.0);
        assert_eq!(pos.stats.value_bought, 100.0);
        assert_eq!(pos.stats.cost, 4.0);
    }
}
