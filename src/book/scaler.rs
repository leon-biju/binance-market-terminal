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
        let ticks = d / self.tick_size;
        if !ticks.is_integer() {
            tracing::warn!(
                price = %d,
                tick_size = %self.tick_size,
                ticks = %ticks,
                "Price not aligned to step size, rounding"
            );

            return ticks.round().to_u64();
        }
        ticks.to_u64()
    }

    pub fn qty_to_ticks(&self, qty: &str) -> Option<u64> {
        let d = Decimal::from_str(qty).ok()?;
        let ticks = d / self.step_size;
        if !ticks.is_integer() {
            tracing::warn!(
                qty = %d,
                step_size = %self.step_size,
                ticks = %ticks,
                "Quantity not aligned to step size, rounding"
            );

            return ticks.round().to_u64();
        }
        ticks.to_u64()
    }

    pub fn ticks_to_price(&self, ticks: u64) -> Decimal {
        Decimal::from(ticks) * self.tick_size
    }

    pub fn ticks_to_qty(&self, ticks: u64) -> Decimal {
        Decimal::from(ticks) * self.step_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_scales_from_tick_and_step() {
        let scaler = Scaler::new(Decimal::from_str("0.0001000").unwrap(), Decimal::from_str("0.0500").unwrap());

        assert_eq!(scaler.price_scale, 10_000); // 4 decimal places after normalizing 0.0001000
        assert_eq!(scaler.qty_scale, 100);      // 2 decimal places after normalizing 0.0500
    }

    #[test]
    fn converts_price_and_qty_round_trip() {
        let scaler = Scaler::new(Decimal::from_str("0.01").unwrap(), Decimal::from_str("0.001").unwrap());

        let price = "1234.56";
        let price_ticks = scaler.price_to_ticks(price).unwrap();
        assert_eq!(price_ticks, 123_456);
        assert_eq!(scaler.ticks_to_price(price_ticks), Decimal::from_str(price).unwrap());

        let qty = "0.123";
        let qty_ticks = scaler.qty_to_ticks(qty).unwrap();
        assert_eq!(qty_ticks, 123);
        assert_eq!(scaler.ticks_to_qty(qty_ticks), Decimal::from_str(qty).unwrap());
    }

    #[test]
    fn rejects_values_not_aligned_to_tick_or_step() {
        let scaler = Scaler::new(Decimal::from_str("0.01").unwrap(), Decimal::from_str("0.1").unwrap());

        // Price 0.015 is 1.5 ticks -> not an integer number of ticks
        assert!(scaler.price_to_ticks("0.015").is_none());

        // Qty 0.25 is 2.5 steps when step size is 0.1
        assert!(scaler.qty_to_ticks("0.25").is_none());
    }
}
