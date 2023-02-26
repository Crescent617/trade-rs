use derive_builder::Builder;

use crate::{
    data::Bar,
    errors::ErrorRepr,
    order::{Fill, Order, OrderKind},
};

pub trait Broker {
    fn exec_order(&mut self, order: &Order, wallet: &mut impl Wallet) -> Result<Fill, ErrorRepr>;

    /// just for backtest
    fn set_lastest_bar(&mut self, bar: &Bar);
}

#[derive(Clone)]
pub enum Cost {
    Ratio(f64),
    Fixed(f64),
}

#[derive(Clone, Builder)]
pub struct SimulatedBroker {
    #[builder(default)]
    pub latest: Option<Bar>,
    #[builder(default)]
    pub commission: f64,
    // TODO implement
    #[builder(default = "Cost::Ratio(0.0)")]
    pub slippage: Cost,
    #[builder(default)]
    position: i32,
}

impl Broker for SimulatedBroker {
    fn exec_order(&mut self, order: &Order, wallet: &mut impl Wallet) -> Result<Fill, ErrorRepr> {
        use OrderKind::*;

        if order.is_expired() {
            return Err(ErrorRepr::OrderExpired(format!("{:?}", order)));
        }

        let bar = self
            .latest
            .as_ref()
            .ok_or(ErrorRepr::NotExists("latest price"))?;

        let price = bar.open;
        let mut qty = order.qty;

        let cash = wallet.balance();

        if qty > 0 {
            // buy
            let cost = qty.abs() as f64 * price * self.commission;
            qty = qty
                .min(bar.vol.floor() as i32)
                .min(((cash - cost) / price).floor() as i32);
        } else {
            // sell
            qty = qty.max(-self.position);
        }

        let cost = qty.abs() as f64 * price * self.commission;
        let fill = Fill {
            time: bar.time,
            qty,
            sym: order.sym.clone(),
            price,
            cost,
        };

        let ok_fill = match order.kind {
            Market => Ok(fill),
            Limit { limit, stop, .. } => {
                if qty < 0 && (price >= limit || Some(price) <= stop) {
                    // sell
                    return Ok(fill);
                } else if qty > 0 && (price <= limit) {
                    // buy
                    Ok(fill)
                } else {
                    Err(ErrorRepr::NotSatisfied("limit order"))
                }
            }
        };

        if let Ok(Fill {
            qty, price, cost, ..
        }) = &ok_fill
        {
            wallet
                .pay(*qty as f64 * price + cost)
                .expect("should have enough money");
            self.position += qty;
        }

        ok_fill
    }

    fn set_lastest_bar(&mut self, bar: &Bar) {
        self.latest.replace(bar.clone());
    }
}

pub trait Wallet {
    fn balance(&self) -> f64;
    fn set_balance(&mut self, money: f64);
    fn pay(&mut self, money: f64) -> Option<f64> {
        let rem = self.balance() - money;
        if rem < 0.0 {
            None
        } else {
            self.set_balance(rem);
            Some(rem)
        }
    }
}

#[cfg(test)]
mod tests {
    use more_asserts::assert_lt;

    use crate::order::{FixedSizeOrderManager, OrderBuilder};
    use crate::portfolio::SimplePortfolioBuilder;

    use super::*;

    #[test]
    fn test_broker_market_order() {
        let mut bro = SimulatedBrokerBuilder::default()
            .commission(0.001)
            .build()
            .unwrap();
        let mut bar = Bar::default();
        bar.open = 10.0;
        bar.vol = 10000.0;
        bro.set_lastest_bar(&bar);

        let mut port = SimplePortfolioBuilder::default()
            .cash(1000.0)
            .order_manager(FixedSizeOrderManager { size: 10 })
            .build()
            .unwrap();
        let mut ord = OrderBuilder::default()
            .sym("test".into())
            .qty(10)
            .build()
            .unwrap();

        bro.exec_order(&ord, &mut port).unwrap();
        assert_eq!(port.cash, 1000.0 - 10.0 * 10.0 * 1.001);
        assert_eq!(port.init_cash, 1000.0);

        ord.qty = 1000;
        let fill = bro.exec_order(&ord, &mut port).unwrap();
        assert_eq!(fill.qty, 88);
        assert_eq!(fill.price, 10.0);
        assert_lt!((1000.0 - 98.0 * 10.0 * 1.001 - port.cash).abs(), 0.001);

        ord.qty = -1000;
        let fill = bro.exec_order(&ord, &mut port).unwrap();
        assert_eq!(fill.qty, -98);
        assert_eq!(fill.price, 10.0);
        assert_lt!(
            (2.0 * 98.0 * 10.0 * 0.001 - (port.init_cash - port.cash)).abs(),
            0.001
        );
    }

    #[test]
    fn test_broker_limit_order() {
        let mut bro = SimulatedBrokerBuilder::default()
            .commission(0.001)
            .build()
            .unwrap();
        let mut bar = Bar::default();
        bar.open = 10.0;
        bar.vol = 10000.0;
        bro.set_lastest_bar(&bar);

        let mut port = SimplePortfolioBuilder::default()
            .cash(1000.0)
            .order_manager(FixedSizeOrderManager { size: 10 })
            .build()
            .unwrap();

        let mut ord = OrderBuilder::default()
            .sym("test".into())
            .qty(10)
            .kind(OrderKind::Limit {
                limit: 9.0,
                stop: Some(12.0),
            })
            .build()
            .unwrap();

        bro.exec_order(&ord, &mut port).expect_err("NotSatisfied");
        assert_eq!(port.cash, port.init_cash);

        bar.open = 8.0;
        bro.set_lastest_bar(&bar);

        ord.qty = 1000;

        let fill = bro.exec_order(&ord, &mut port).unwrap();
        assert_eq!(fill.qty, 124);
        assert_eq!(fill.price, 8.0);
        assert_lt!((1000.0 - 124.0 * 8.0 * 1.001 - port.cash).abs(), 0.001);

        ord.qty = -1000;
        ord.kind = OrderKind::Limit { limit: 12.0, stop: Some(8.0) };

        bar.open = 12.0;
        bro.set_lastest_bar(&bar);

        let fill = bro.exec_order(&ord, &mut port).unwrap();
        assert_eq!(fill.qty, -124);
        assert_eq!(fill.price, 12.0);
    }
}
