/// Resource pool — the shared global resource that organisms compete for.
pub struct ResourcePool {
    pub available: f64,
    pub replenishment_rate: f64,
    pub max_capacity: f64,
    /// Amplitude of seasonal variation (0.0 = stable, 0.9 = extreme swings)
    pub volatility: f64,
    /// Period of seasonal cycle in ticks
    pub season_period: u64,
}

impl ResourcePool {
    pub fn new(
        initial: f64,
        replenishment_rate: f64,
        max_capacity: f64,
        volatility: f64,
        season_period: u64,
    ) -> Self {
        Self {
            available: initial,
            replenishment_rate,
            max_capacity,
            volatility: volatility.clamp(0.0, 0.95),
            season_period: season_period.max(1),
        }
    }

    /// Seasonal replenishment. Modulated by a sine wave:
    /// actual = base * (1 + volatility * sin(2π * tick / period))
    /// At peak: base * (1 + volatility). At trough: base * (1 - volatility).
    pub fn replenish(&mut self, tick: u64) {
        let phase = 2.0 * std::f64::consts::PI * (tick as f64) / (self.season_period as f64);
        let modifier = 1.0 + self.volatility * phase.sin();
        let amount = self.replenishment_rate * modifier;
        self.available = (self.available + amount.max(0.0)).min(self.max_capacity);
    }

    /// Debit from pool. Returns actual amount consumed (may be less than requested).
    pub fn consume(&mut self, amount: f64) -> f64 {
        let actual = amount.min(self.available);
        self.available -= actual;
        actual
    }

    /// Current seasonal modifier (for logging/viz).
    pub fn season_modifier(&self, tick: u64) -> f64 {
        let phase = 2.0 * std::f64::consts::PI * (tick as f64) / (self.season_period as f64);
        1.0 + self.volatility * phase.sin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replenish_caps_at_max() {
        let mut pool = ResourcePool::new(9000.0, 5000.0, 10000.0, 0.0, 1000);
        pool.replenish(0);
        assert!((pool.available - 10000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn consume_returns_available_when_insufficient() {
        let mut pool = ResourcePool::new(100.0, 50.0, 1000.0, 0.0, 1000);
        let consumed = pool.consume(200.0);
        assert!((consumed - 100.0).abs() < f64::EPSILON);
        assert!(pool.available.abs() < f64::EPSILON);
    }

    #[test]
    fn seasonal_variation() {
        let mut pool = ResourcePool::new(0.0, 1000.0, 100000.0, 0.5, 100);
        // At tick 0: sin(0) = 0, modifier = 1.0
        pool.replenish(0);
        assert!((pool.available - 1000.0).abs() < 1.0);

        // At tick 25 (quarter period): sin(π/2) = 1.0, modifier = 1.5
        pool.available = 0.0;
        pool.replenish(25);
        assert!((pool.available - 1500.0).abs() < 1.0);

        // At tick 75 (3/4 period): sin(3π/2) = -1.0, modifier = 0.5
        pool.available = 0.0;
        pool.replenish(75);
        assert!((pool.available - 500.0).abs() < 1.0);
    }
}
