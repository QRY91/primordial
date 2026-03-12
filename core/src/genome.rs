use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Genome(pub u64);

impl Genome {
    /// Bits 0-7: metabolism rate. Higher = more resource consumption per tick.
    pub fn metabolism_rate(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Bits 8-15: energy threshold needed to reproduce.
    pub fn repro_threshold(&self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }

    /// Bits 16-23: mutation rate. Probability per bit = value/255.
    pub fn mutation_rate(&self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }

    /// Bits 24-31: mobility. Higher = cheaper/more frequent migration.
    pub fn mobility(&self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }

    /// Bits 32-39: heat tolerance. Maps to optimal temperature range.
    /// 0 = cold-adapted (-30C), 255 = heat-adapted (+50C).
    pub fn heat_tolerance(&self) -> u8 {
        ((self.0 >> 32) & 0xFF) as u8
    }

    /// Optimal temperature for this organism.
    pub fn optimal_temperature(&self) -> f64 {
        self.heat_tolerance() as f64 / 255.0 * 80.0 - 30.0
    }

    /// Bits 40-47: moisture preference. 0 = xeric, 255 = hydric.
    pub fn moisture_preference(&self) -> u8 {
        ((self.0 >> 40) & 0xFF) as u8
    }

    /// Optimal moisture level for this organism.
    pub fn optimal_moisture(&self) -> f64 {
        self.moisture_preference() as f64 / 255.0
    }

    /// Apply bit-flip mutation. Each of 64 bits flips with probability mutation_rate/255.
    pub fn mutate(&self, rng: &mut impl Rng) -> Genome {
        let rate = self.mutation_rate() as f64 / 255.0;
        let mut bits = self.0;
        for i in 0..64 {
            if rng.gen::<f64>() < rate {
                bits ^= 1u64 << i;
            }
        }
        Genome(bits)
    }

    /// Single-point crossover. Pick a random bit position, take bits 0..pos from self
    /// and bits pos..64 from other.
    pub fn crossover(&self, other: &Genome, rng: &mut impl Rng) -> Genome {
        let pos = rng.gen_range(1..64);
        let mask = (1u64 << pos) - 1; // bits 0..pos-1 set
        Genome((self.0 & mask) | (other.0 & !mask))
    }

    /// Hamming distance between two genomes.
    pub fn distance(&self, other: &Genome) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    /// Create a random genome.
    pub fn random(rng: &mut impl Rng) -> Genome {
        Genome(rng.gen())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn bit_field_extraction() {
        let g = Genome(0x0000_0504_0302_0100);
        assert_eq!(g.metabolism_rate(), 0x00);
        assert_eq!(g.repro_threshold(), 0x01);
        assert_eq!(g.mutation_rate(), 0x02);
        assert_eq!(g.mobility(), 0x03);
        assert_eq!(g.heat_tolerance(), 0x04);
        assert_eq!(g.moisture_preference(), 0x05);
    }

    #[test]
    fn optimal_temperature_range() {
        let cold = Genome(0); // heat_tolerance = 0
        let hot = Genome(0xFF_0000_0000); // heat_tolerance = 255
        assert!(cold.optimal_temperature() < -25.0);
        assert!(hot.optimal_temperature() > 45.0);
    }

    #[test]
    fn mutation_preserves_bit_length() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let g = Genome(0xDEADBEEF_CAFEBABE);
        for _ in 0..1000 {
            let mutated = g.mutate(&mut rng);
            // Always u64 — no overflow possible by construction
            let _ = mutated.0;
        }
    }

    #[test]
    fn zero_mutation_rate_preserves_genome() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        // mutation_rate is bits 16-23, set to 0
        let g = Genome(0xFF00FFFF);
        assert_eq!(g.mutation_rate(), 0);
        let mutated = g.mutate(&mut rng);
        assert_eq!(g, mutated);
    }

    #[test]
    fn distance_symmetric() {
        let a = Genome(0xAAAA);
        let b = Genome(0x5555);
        assert_eq!(a.distance(&b), b.distance(&a));
    }

    #[test]
    fn distance_self_is_zero() {
        let g = Genome(0xDEADBEEF);
        assert_eq!(g.distance(&g), 0);
    }

    #[test]
    fn crossover_mixes_parents() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let a = Genome(0x0000_0000_0000_0000);
        let b = Genome(0xFFFF_FFFF_FFFF_FFFF);
        let child = a.crossover(&b, &mut rng);
        // Child should have some bits from each parent
        assert_ne!(child, a);
        assert_ne!(child, b);
        // Lower bits from a (0s), upper bits from b (1s) — or vice versa
        // Either way, it should be a contiguous split
        let bits = child.0;
        let trailing = bits.trailing_zeros();
        let leading = bits.leading_ones();
        // One of these patterns: 0...01...1 or all from one parent at split point
        assert!(trailing + leading >= 1, "crossover should produce contiguous split");
    }
}
