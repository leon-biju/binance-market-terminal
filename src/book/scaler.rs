use rust_decimal::Decimal;
use std::str::FromStr;
use num_traits::ToPrimitive;

#[derive(Debug, Clone)]
pub struct Scaler {
    price_scale: u64,
    qty_scale: u64,
    tick_size: Decimal,
    step_size: Decimal,
}

impl Scaler {
    pub fn new(tick_size: Decimal, step_size: Decimal) -> Self {
        let price_scale = Self::decimal_to_scale(&tick_size);
        let qty_scale = Self::decimal_to_scale(&step_size);

        Self {
            price_scale,
            qty_scale,
            tick_size,
            step_size,
        }
    }

    fn decimal_to_scale(d: &Decimal) -> u64 {
        let normalized = d.normalize(); // remove trailing zeros
        let places = normalized.scale(); // digits after decimal point
        10u64.pow(places as u32)
    }

    pub fn price_to_ticks(&self, price: &str) -> Option<u64> {
        let d = Decimal::from_str(price).ok()?;
        let ticks = d / self.tick_size; // divide by tick size
        ticks.to_u64()
    }

    pub fn qty_to_ticks(&self, qty: &str) -> Option<u64> {
        let d = Decimal::from_str(qty).ok()?;
        let ticks = d / self.step_size;
        ticks.to_u64()
    }

    pub fn ticks_to_price(&self, ticks: u64) -> Decimal {
        Decimal::from(ticks) * self.tick_size
    }

    pub fn ticks_to_qty(&self, ticks: u64) -> Decimal {
        Decimal::from(ticks) * self.step_size
    }
}
