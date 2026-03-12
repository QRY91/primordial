use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::Serialize;

use primordial_world::{World, WorldConfig};

use crate::error::Result;
use crate::genome::Genome;
use crate::lineage::{LineageEvent, LineageTracker};
use crate::organism::{Organism, OrganismId};

#[derive(Clone, Debug)]
pub struct PopulationConfig {
    pub max_population: usize,
    pub initial_population: usize,
    pub initial_energy: f64,
    pub metabolism_scale: f64,
    pub base_survival_cost: f64,
    pub survival_cost_scale: f64,
    pub reproduction_cost: f64,
    pub child_energy_fraction: f64,
    pub divergence_threshold: u32,
    pub resource_replenishment: f64,
    pub resource_max_capacity: f64,
    pub initial_resources: f64,
    pub snapshot_interval: u64,
    pub max_age: u64,
    // Phase 0 compat
    pub resource_volatility: f64,
    pub season_period: u64,
    // Phase 1: world
    pub star_mass: f64,
    pub orbital_radius: f64,
    pub axial_tilt: f64,
    pub hydrosphere: f64,
    pub grid_size: usize,
    pub weather_volatility: f64,
    // Phase 1: migration & biome
    pub migration_rate: f64,
    pub migration_cost: f64,
    pub mismatch_scale: f64,
}

#[derive(Debug, Serialize)]
pub struct TickSummary {
    pub tick: u64,
    pub population_size: usize,
    pub births: usize,
    pub deaths: usize,
    pub active_lineages: usize,
    pub total_resources: f64,
    pub total_consumption: f64,
    pub avg_energy: f64,
    pub avg_metabolism: f64,
    pub avg_repro_threshold: f64,
    pub avg_mutation_rate: f64,
    pub genome_diversity: f64,
    pub season_modifier: f64,
    pub num_cells: usize,
    pub migrations: usize,
}

pub struct Population {
    pub organisms: Vec<Organism>,
    pub cell_resources: Vec<f64>,
    pub world: World,
    pub lineage_tracker: LineageTracker,
    next_organism_id: OrganismId,
    config: PopulationConfig,
    rng: ChaCha8Rng,
}

impl Population {
    pub fn new(config: PopulationConfig, seed: u64) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut lineage_tracker = LineageTracker::new(config.divergence_threshold);

        let world_config = WorldConfig {
            star_mass: config.star_mass,
            orbital_radius: config.orbital_radius,
            axial_tilt: config.axial_tilt,
            hydrosphere: config.hydrosphere,
            grid_size: config.grid_size,
            weather_volatility: config.weather_volatility,
            season_period: config.season_period,
        };
        let world = World::new(&world_config);
        let num_cells = world.num_cells();

        // Distribute initial resources across cells by productivity
        let productivities: Vec<f64> = (0..num_cells)
            .map(|c| world.cell_productivity(c))
            .collect();
        let total_prod: f64 = productivities.iter().sum();
        let cell_resources: Vec<f64> = if total_prod > 0.0 {
            productivities
                .iter()
                .map(|p| config.initial_resources * p / total_prod)
                .collect()
        } else {
            vec![config.initial_resources / num_cells as f64; num_cells]
        };

        // Distribute organisms evenly across cells
        let organisms: Vec<Organism> = (0..config.initial_population as u64)
            .map(|id| {
                let cell_id = id as usize % num_cells;
                let genome = Genome::random(&mut rng);
                let lineage = lineage_tracker.create_lineage(None, &genome, 0);
                lineage_tracker.record_birth(lineage);
                Organism::new(
                    id,
                    genome,
                    lineage,
                    None,
                    0,
                    config.initial_energy,
                    cell_id,
                )
            })
            .collect();

        Self {
            next_organism_id: config.initial_population as u64,
            organisms,
            cell_resources,
            world,
            lineage_tracker,
            config,
            rng,
        }
    }

    /// Execute one full tick of the simulation.
    pub fn tick(&mut self, current_tick: u64) -> Result<TickSummary> {
        let num_cells = self.world.num_cells();

        // 1. World tick: update climate (season + weather)
        self.world.tick(current_tick, &mut self.rng);
        let season_mod = self.world.season_phase(current_tick);

        // 2. Per-cell resource replenishment
        let productivities: Vec<f64> = (0..num_cells)
            .map(|c| self.world.cell_productivity(c))
            .collect();
        let total_prod: f64 = productivities.iter().sum();

        if total_prod > 0.0 {
            // In Phase 0 compat mode (grid_size=1), also apply resource_volatility
            let volatility_mod = if num_cells == 1 {
                let phase = 2.0 * std::f64::consts::PI * (current_tick as f64)
                    / (self.config.season_period.max(1) as f64);
                1.0 + self.config.resource_volatility * phase.sin()
            } else {
                1.0 // Phase 1: seasonality comes through temperature -> productivity
            };

            for c in 0..num_cells {
                let frac = productivities[c] / total_prod;
                let replenish =
                    self.config.resource_replenishment * frac * volatility_mod.max(0.0);
                let capacity = self.config.resource_max_capacity * frac;
                self.cell_resources[c] = (self.cell_resources[c] + replenish).min(capacity);
            }
        }

        // 3. Compute per-cell desired consumption
        let mut cell_desired = vec![0.0f64; num_cells];
        let metabolism_scale = self.config.metabolism_scale;
        for org in &self.organisms {
            cell_desired[org.cell_id] += org.genome.metabolism_rate() as f64 * metabolism_scale;
        }

        // 4. Per-cell scarcity factor
        let cell_scarcity: Vec<f64> = (0..num_cells)
            .map(|c| {
                if cell_desired[c] > 0.0 {
                    (self.cell_resources[c] / cell_desired[c]).min(1.0)
                } else {
                    1.0
                }
            })
            .collect();

        // 5. Consume from each cell
        let mut total_consumed = 0.0;
        for c in 0..num_cells {
            let consumed = cell_desired[c].min(self.cell_resources[c]);
            self.cell_resources[c] -= consumed;
            total_consumed += consumed;
        }

        // 6. Apply energy changes + biome mismatch penalty (parallel)
        let base_cost = self.config.base_survival_cost;
        let cost_scale = self.config.survival_cost_scale;
        let mismatch_scale = self.config.mismatch_scale;
        let world_cells = &self.world.grid.cells;

        self.organisms.par_iter_mut().for_each(|org| {
            let desired = org.genome.metabolism_rate() as f64 * metabolism_scale;
            let actual_intake = desired * cell_scarcity[org.cell_id];
            let survival_cost = base_cost + org.genome.metabolism_rate() as f64 * cost_scale;

            // Biome mismatch penalty (only meaningful with grid_size > 1)
            let mismatch = if mismatch_scale > 0.0 {
                let cell = &world_cells[org.cell_id];
                let temp_diff = (cell.temperature - org.genome.optimal_temperature()) / 30.0;
                let moist_diff = cell.moisture - org.genome.optimal_moisture();
                (temp_diff.powi(2) + moist_diff.powi(2)) * mismatch_scale
            } else {
                0.0
            };

            org.energy += actual_intake - survival_cost - mismatch;
            org.age += 1;
        });

        // 7. Cull dead (energy <= 0 or age > max_age)
        let mut dead_count = 0usize;
        let max_age = self.config.max_age;
        let mut i = 0;
        while i < self.organisms.len() {
            if !self.organisms[i].is_alive() || self.organisms[i].age > max_age {
                let dead = self.organisms.swap_remove(i);
                self.lineage_tracker
                    .record_death(dead.lineage_id, &dead.genome, current_tick);
                dead_count += 1;
            } else {
                i += 1;
            }
        }

        // 8. Migration (sequential — uses rng)
        let mut migration_count = 0usize;
        let migration_rate = self.config.migration_rate;
        let migration_cost = self.config.migration_cost;
        if num_cells > 1 {
            for i in 0..self.organisms.len() {
                let mobility = self.organisms[i].genome.mobility() as f64 / 255.0;
                if self.rng.gen::<f64>() < mobility * migration_rate {
                    let adj = self.world.adjacency(self.organisms[i].cell_id);
                    if !adj.is_empty() {
                        let dest = adj[self.rng.gen_range(0..adj.len())];
                        let cost = migration_cost * (1.0 - mobility);
                        self.organisms[i].energy -= cost;
                        self.organisms[i].cell_id = dest;
                        migration_count += 1;
                    }
                }
            }
        }

        // 9. Reproduction with crossover — partners must share a cell
        let mut offspring = Vec::new();
        let repro_cost = self.config.reproduction_cost;
        let child_frac = self.config.child_energy_fraction;
        let pop_len = self.organisms.len();

        if pop_len >= 2 {
            // Build per-cell organism indices
            let mut cell_orgs: Vec<Vec<usize>> = vec![vec![]; num_cells];
            for (idx, org) in self.organisms.iter().enumerate() {
                cell_orgs[org.cell_id].push(idx);
            }

            for idx in 0..pop_len {
                if self.organisms[idx].can_reproduce()
                    && pop_len + offspring.len() < self.config.max_population
                {
                    let cell = self.organisms[idx].cell_id;
                    let local = &cell_orgs[cell];

                    // Need at least 2 organisms in the cell for crossover
                    if local.len() >= 2 {
                        // Pick a random partner from the same cell (not self)
                        let mut attempts = 0;
                        loop {
                            let pick = self.rng.gen_range(0..local.len());
                            let partner_idx = local[pick];
                            if partner_idx != idx {
                                let partner_genome = self.organisms[partner_idx].genome;
                                let child_id = self.next_organism_id;
                                self.next_organism_id += 1;
                                let child = self.organisms[idx].reproduce_with_crossover(
                                    child_id,
                                    &partner_genome,
                                    &mut self.rng,
                                    current_tick,
                                    repro_cost,
                                    child_frac,
                                    &mut self.lineage_tracker,
                                );
                                offspring.push(child);
                                break;
                            }
                            attempts += 1;
                            if attempts > 5 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        let birth_count = offspring.len();
        self.organisms.extend(offspring);

        // 10. Population cap (global)
        if self.organisms.len() > self.config.max_population {
            self.organisms
                .sort_unstable_by(|a, b| a.energy.partial_cmp(&b.energy).unwrap());
            let excess = self.organisms.len() - self.config.max_population;
            for org in self.organisms.drain(..excess) {
                self.lineage_tracker
                    .record_death(org.lineage_id, &org.genome, current_tick);
                dead_count += 1;
            }
        }

        // 11. Periodic lineage snapshot
        if self.config.snapshot_interval > 0 && current_tick % self.config.snapshot_interval == 0 {
            self.lineage_tracker.snapshot(current_tick);
        }

        // 12. Build summary
        let pop_size = self.organisms.len();
        let (energy_sum, metabolism_sum, repro_sum, mutation_sum) =
            self.organisms
                .iter()
                .fold((0.0f64, 0.0f64, 0.0f64, 0.0f64), |(e, m, r, mu), org| {
                    (
                        e + org.energy,
                        m + org.genome.metabolism_rate() as f64,
                        r + org.genome.repro_threshold() as f64,
                        mu + org.genome.mutation_rate() as f64,
                    )
                });

        let genome_diversity = Self::compute_diversity(&self.organisms);
        let total_resources: f64 = self.cell_resources.iter().sum();

        Ok(TickSummary {
            tick: current_tick,
            population_size: pop_size,
            births: birth_count,
            deaths: dead_count,
            active_lineages: self.lineage_tracker.active_lineage_count(),
            total_resources,
            total_consumption: total_consumed,
            avg_energy: if pop_size > 0 {
                energy_sum / pop_size as f64
            } else {
                0.0
            },
            avg_metabolism: if pop_size > 0 {
                metabolism_sum / pop_size as f64
            } else {
                0.0
            },
            avg_repro_threshold: if pop_size > 0 {
                repro_sum / pop_size as f64
            } else {
                0.0
            },
            avg_mutation_rate: if pop_size > 0 {
                mutation_sum / pop_size as f64
            } else {
                0.0
            },
            genome_diversity,
            season_modifier: season_mod,
            num_cells,
            migrations: migration_count,
        })
    }

    /// Shannon entropy across bit positions.
    fn compute_diversity(organisms: &[Organism]) -> f64 {
        let n = organisms.len();
        if n < 2 {
            return 0.0;
        }
        let n_f = n as f64;
        let mut total_entropy = 0.0;
        for bit in 0..64 {
            let ones = organisms
                .iter()
                .filter(|o| (o.genome.0 >> bit) & 1 == 1)
                .count() as f64;
            let p = ones / n_f;
            if p > 0.0 && p < 1.0 {
                total_entropy += -p * p.log2() - (1.0 - p) * (1.0 - p).log2();
            }
        }
        total_entropy / 64.0
    }

    /// Drain pending lineage events for logging.
    pub fn drain_lineage_events(&mut self) -> Vec<LineageEvent> {
        self.lineage_tracker.drain_events()
    }

    pub fn organism_count(&self) -> usize {
        self.organisms.len()
    }

    pub fn is_extinct(&self) -> bool {
        self.organisms.is_empty()
    }

    /// Per-cell population counts (for logging/viz).
    pub fn cell_populations(&self) -> Vec<usize> {
        let num_cells = self.world.num_cells();
        let mut counts = vec![0usize; num_cells];
        for org in &self.organisms {
            counts[org.cell_id] += 1;
        }
        counts
    }

    /// Per-biome population counts as (biome_name, count) pairs.
    pub fn biome_populations(&self) -> Vec<(String, usize)> {
        let cell_pops = self.cell_populations();
        let mut biome_counts = std::collections::HashMap::new();
        for (cell_id, &count) in cell_pops.iter().enumerate() {
            let biome = self.world.cell_biome(cell_id);
            *biome_counts.entry(biome.name().to_string()).or_insert(0usize) += count;
        }
        let mut result: Vec<_> = biome_counts.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PopulationConfig {
        PopulationConfig {
            max_population: 1000,
            initial_population: 50,
            initial_energy: 100.0,
            metabolism_scale: 0.1,
            base_survival_cost: 1.0,
            survival_cost_scale: 0.05,
            reproduction_cost: 30.0,
            child_energy_fraction: 0.4,
            divergence_threshold: 8,
            resource_replenishment: 500.0,
            resource_max_capacity: 5000.0,
            initial_resources: 1000.0,
            snapshot_interval: 100,
            max_age: 100,
            resource_volatility: 0.0,
            season_period: 1000,
            // Phase 1 defaults (single cell = Phase 0 behavior)
            star_mass: 1.0,
            orbital_radius: 1.0,
            axial_tilt: 23.5,
            hydrosphere: 0.7,
            grid_size: 1,
            weather_volatility: 0.0,
            migration_rate: 0.0,
            migration_cost: 0.0,
            mismatch_scale: 0.0,
        }
    }

    #[test]
    fn population_runs_without_panic() {
        let mut pop = Population::new(test_config(), 42);
        for tick in 0..1000 {
            let summary = pop.tick(tick).unwrap();
            assert!(summary.population_size <= 1000);
        }
    }

    #[test]
    fn population_size_bounded() {
        let mut config = test_config();
        config.max_population = 200;
        config.resource_replenishment = 50000.0;
        let mut pop = Population::new(config, 42);
        for tick in 0..500 {
            let summary = pop.tick(tick).unwrap();
            assert!(summary.population_size <= 200);
        }
    }

    #[test]
    fn extinction_under_starvation() {
        let mut config = test_config();
        config.resource_replenishment = 0.0;
        config.initial_resources = 0.0;
        let mut pop = Population::new(config, 42);
        for tick in 0..1000 {
            pop.tick(tick).unwrap();
            if pop.is_extinct() {
                break;
            }
        }
        assert!(
            pop.is_extinct(),
            "population should go extinct with no resources"
        );
    }

    #[test]
    fn lineage_events_produced() {
        let mut pop = Population::new(test_config(), 42);
        for tick in 0..200 {
            pop.tick(tick).unwrap();
        }
        let events = pop.drain_lineage_events();
        assert!(
            !events.is_empty(),
            "should have lineage events after 200 ticks"
        );
    }

    #[test]
    fn genome_diversity_range() {
        let pop = Population::new(test_config(), 42);
        let div = Population::compute_diversity(&pop.organisms);
        assert!(div >= 0.0 && div <= 1.0, "diversity should be in [0, 1]");
        assert!(div > 0.5, "random initial genomes should be diverse");
    }

    #[test]
    fn seasonal_resources_create_variation() {
        let mut config = test_config();
        config.resource_volatility = 0.8;
        config.season_period = 200;
        config.resource_replenishment = 5000.0;
        config.initial_resources = 5000.0;
        let mut pop = Population::new(config, 42);

        let mut resource_levels = Vec::new();
        for tick in 0..400 {
            let summary = pop.tick(tick).unwrap();
            resource_levels.push(summary.total_resources);
        }
        let min_r = resource_levels.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_r = resource_levels
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(max_r > min_r, "seasonal resources should vary over time");
    }

    #[test]
    fn multi_cell_runs() {
        let mut config = test_config();
        config.grid_size = 4;
        config.migration_rate = 0.1;
        config.migration_cost = 2.0;
        config.mismatch_scale = 1.0;
        config.resource_replenishment = 5000.0;
        config.initial_resources = 5000.0;
        let mut pop = Population::new(config, 42);
        assert_eq!(pop.world.num_cells(), 16);

        for tick in 0..500 {
            let summary = pop.tick(tick).unwrap();
            assert_eq!(summary.num_cells, 16);
        }
        // Should still have organisms
        assert!(!pop.is_extinct(), "population should survive 500 ticks");
    }

    #[test]
    fn migration_moves_organisms() {
        let mut config = test_config();
        config.grid_size = 4;
        config.migration_rate = 1.0; // very high
        config.migration_cost = 0.0; // free
        config.mismatch_scale = 0.0;
        config.resource_replenishment = 50000.0;
        config.initial_resources = 50000.0;
        config.initial_population = 100;
        let mut pop = Population::new(config, 42);

        let mut total_migrations = 0;
        for tick in 0..100 {
            let summary = pop.tick(tick).unwrap();
            total_migrations += summary.migrations;
        }
        assert!(
            total_migrations > 0,
            "should have migrations with rate=1.0"
        );
    }

    #[test]
    fn biome_populations_sum_to_total() {
        let mut config = test_config();
        config.grid_size = 4;
        config.initial_population = 100;
        config.resource_replenishment = 5000.0;
        config.initial_resources = 5000.0;
        let pop = Population::new(config, 42);
        let biome_pops = pop.biome_populations();
        let sum: usize = biome_pops.iter().map(|(_, c)| c).sum();
        assert_eq!(sum, pop.organism_count());
    }
}
