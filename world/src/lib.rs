pub mod biome;
pub mod climate;
pub mod planet;
pub mod star;

use rand::Rng;

pub use biome::{productivity, BiomeType};
pub use climate::{ClimateCell, ClimateGrid};
pub use planet::Planet;
pub use star::Star;

/// World configuration — star, planet, and grid parameters.
#[derive(Clone, Debug)]
pub struct WorldConfig {
    pub star_mass: f64,
    pub orbital_radius: f64,
    pub axial_tilt: f64,
    pub hydrosphere: f64,
    pub grid_size: usize,
    pub weather_volatility: f64,
    pub season_period: u64,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            star_mass: 1.0,
            orbital_radius: 1.0,
            axial_tilt: 23.5,
            hydrosphere: 0.7,
            grid_size: 1,
            weather_volatility: 0.0,
            season_period: 5000,
        }
    }
}

/// The world: star + planet + climate grid.
pub struct World {
    pub star: Star,
    pub planet: Planet,
    pub grid: ClimateGrid,
    pub season_period: u64,
    pub weather_volatility: f64,
}

impl World {
    pub fn new(config: &WorldConfig) -> Self {
        let star = Star::new(config.star_mass);
        let planet = Planet::new(config.orbital_radius, config.axial_tilt, config.hydrosphere);
        let equator_temp = planet.equilibrium_temp(&star);

        let grid = ClimateGrid::new(
            config.grid_size,
            config.grid_size,
            equator_temp,
            config.hydrosphere,
        );

        Self {
            star,
            planet,
            grid,
            season_period: config.season_period.max(1),
            weather_volatility: config.weather_volatility,
        }
    }

    /// Advance climate by one tick: seasonal shift + weather noise.
    pub fn tick(&mut self, current_tick: u64, rng: &mut impl Rng) {
        self.grid.tick(
            current_tick,
            self.season_period,
            self.planet.seasonal_amplitude(),
            self.weather_volatility,
            rng,
        );
    }

    pub fn num_cells(&self) -> usize {
        self.grid.num_cells()
    }

    pub fn cell(&self, idx: usize) -> &ClimateCell {
        &self.grid.cells[idx]
    }

    pub fn cell_productivity(&self, idx: usize) -> f64 {
        let cell = &self.grid.cells[idx];
        productivity(cell.temperature, cell.moisture)
    }

    pub fn cell_biome(&self, idx: usize) -> BiomeType {
        let cell = &self.grid.cells[idx];
        BiomeType::classify(cell.temperature, cell.moisture)
    }

    pub fn adjacency(&self, idx: usize) -> &[usize] {
        &self.grid.cells[idx].adjacency
    }

    /// Season phase for logging: sin(2pi*tick/period).
    pub fn season_phase(&self, tick: u64) -> f64 {
        let phase = 2.0 * std::f64::consts::PI * (tick as f64) / (self.season_period as f64);
        phase.sin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn world_creates_correct_grid_size() {
        let config = WorldConfig {
            grid_size: 4,
            ..Default::default()
        };
        let world = World::new(&config);
        assert_eq!(world.num_cells(), 16);
    }

    #[test]
    fn degenerate_single_cell() {
        let config = WorldConfig::default(); // grid_size=1
        let world = World::new(&config);
        assert_eq!(world.num_cells(), 1);
        assert!(world.adjacency(0).is_empty());
    }

    #[test]
    fn productivity_varies_across_grid() {
        let config = WorldConfig {
            grid_size: 6,
            ..Default::default()
        };
        let world = World::new(&config);
        let prods: Vec<f64> = (0..world.num_cells())
            .map(|c| world.cell_productivity(c))
            .collect();
        let min = prods.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = prods.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(max > min, "productivity should vary: min={min}, max={max}");
    }

    #[test]
    fn multiple_biome_types() {
        let config = WorldConfig {
            grid_size: 6,
            ..Default::default()
        };
        let world = World::new(&config);
        let biomes: std::collections::HashSet<BiomeType> = (0..world.num_cells())
            .map(|c| world.cell_biome(c))
            .collect();
        assert!(
            biomes.len() >= 3,
            "6x6 grid should have at least 3 biome types, got {:?}",
            biomes
        );
    }
}
