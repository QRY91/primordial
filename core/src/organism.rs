use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::genome::Genome;
use crate::lineage::LineageTracker;

pub type OrganismId = u64;
pub type LineageId = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Organism {
    pub id: OrganismId,
    pub genome: Genome,
    pub energy: f64,
    pub age: u64,
    pub lineage_id: LineageId,
    pub parent_id: Option<OrganismId>,
    pub born_tick: u64,
    pub cell_id: usize,
}

impl Organism {
    pub fn new(
        id: OrganismId,
        genome: Genome,
        lineage_id: LineageId,
        parent_id: Option<OrganismId>,
        born_tick: u64,
        initial_energy: f64,
        cell_id: usize,
    ) -> Self {
        Self {
            id,
            genome,
            energy: initial_energy,
            age: 0,
            lineage_id,
            parent_id,
            born_tick,
            cell_id,
        }
    }

    /// Check if organism has enough energy to reproduce.
    pub fn can_reproduce(&self) -> bool {
        self.energy >= self.genome.repro_threshold() as f64
    }

    /// Produce offspring. Parent pays energy cost, child gets a fraction of remaining energy.
    /// Child genome is a mutated copy of parent's.
    pub fn reproduce(
        &mut self,
        child_id: OrganismId,
        rng: &mut impl Rng,
        tick: u64,
        reproduction_cost: f64,
        child_energy_fraction: f64,
        lineage_tracker: &mut LineageTracker,
    ) -> Organism {
        self.energy -= reproduction_cost;
        let child_energy = self.energy * child_energy_fraction;
        self.energy -= child_energy;

        let child_genome = self.genome.mutate(rng);
        let child_lineage = lineage_tracker.assign_lineage(
            &self.genome,
            &child_genome,
            self.lineage_id,
            tick,
        );
        lineage_tracker.record_birth(child_lineage);

        Organism::new(
            child_id,
            child_genome,
            child_lineage,
            Some(self.id),
            tick,
            child_energy,
            self.cell_id,
        )
    }

    /// Reproduce with crossover. Child genome = crossover(self, partner) then mutated.
    pub fn reproduce_with_crossover(
        &mut self,
        child_id: OrganismId,
        partner_genome: &Genome,
        rng: &mut impl Rng,
        tick: u64,
        reproduction_cost: f64,
        child_energy_fraction: f64,
        lineage_tracker: &mut LineageTracker,
    ) -> Organism {
        self.energy -= reproduction_cost;
        let child_energy = self.energy * child_energy_fraction;
        self.energy -= child_energy;

        let crossed = self.genome.crossover(partner_genome, rng);
        let child_genome = crossed.mutate(rng);
        let child_lineage = lineage_tracker.assign_lineage(
            &self.genome,
            &child_genome,
            self.lineage_id,
            tick,
        );
        lineage_tracker.record_birth(child_lineage);

        Organism::new(
            child_id,
            child_genome,
            child_lineage,
            Some(self.id),
            tick,
            child_energy,
            self.cell_id,
        )
    }

    pub fn is_alive(&self) -> bool {
        self.energy > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn make_organism(metabolism: u8, repro_threshold: u8, energy: f64) -> Organism {
        let genome_bits = metabolism as u64 | ((repro_threshold as u64) << 8);
        Organism::new(1, Genome(genome_bits), 0, None, 0, energy, 0)
    }

    #[test]
    fn can_reproduce_respects_threshold() {
        let org = make_organism(100, 50, 49.0);
        assert!(!org.can_reproduce());

        let org = make_organism(100, 50, 50.0);
        assert!(org.can_reproduce());
    }

    #[test]
    fn energy_never_negative_after_reproduce() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut tracker = LineageTracker::new(8);
        tracker.record_birth(0);

        let mut parent = make_organism(100, 50, 100.0);
        let child = parent.reproduce(2, &mut rng, 1, 30.0, 0.4, &mut tracker);

        assert!(parent.energy >= 0.0);
        assert!(child.energy >= 0.0);
        // Parent had 100, paid 30 cost = 70, then gave 40% of 70 = 28 to child
        assert!((parent.energy - 42.0).abs() < 0.01);
        assert!((child.energy - 28.0).abs() < 0.01);
    }
}
