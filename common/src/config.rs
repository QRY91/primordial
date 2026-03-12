use std::path::Path;

use serde::Deserialize;

use primordial_core::population::PopulationConfig;

/// Parse a TOML config file into a TomlConfig.
pub fn load_config(path: &Path) -> Result<TomlConfig, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&raw).map_err(|e| format!("bad config: {e}"))
}

/// Parse TOML string directly.
pub fn parse_config(s: &str) -> Result<TomlConfig, String> {
    toml::from_str(s).map_err(|e| format!("bad config: {e}"))
}

#[derive(Deserialize, Default)]
pub struct TomlConfig {
    #[serde(default)]
    pub simulation: SimSection,
    #[serde(default)]
    pub population: PopSection,
    #[serde(default)]
    pub resources: ResourceSection,
    #[serde(default)]
    pub energy: EnergySection,
    #[serde(default)]
    pub reproduction: ReproSection,
    #[serde(default)]
    pub lifespan: LifespanSection,
    #[serde(default)]
    pub lineage: LineageSection,
    #[serde(default)]
    pub genome: GenomeSection,
    #[serde(default)]
    pub world: Option<WorldSection>,
    #[serde(default)]
    pub logging: LogSection,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct SimSection {
    pub seed: u64,
    pub max_ticks: u64,
    pub log_interval: u64,
}
impl Default for SimSection {
    fn default() -> Self {
        Self { seed: 42, max_ticks: 100_000, log_interval: 100 }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct PopSection {
    pub initial_size: usize,
    pub max_size: usize,
    pub initial_energy: f64,
}
impl Default for PopSection {
    fn default() -> Self {
        Self { initial_size: 500, max_size: 50_000, initial_energy: 100.0 }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ResourceSection {
    pub initial: f64,
    pub replenishment_rate: f64,
    pub max_capacity: f64,
    pub volatility: f64,
    pub season_period: u64,
}
impl Default for ResourceSection {
    fn default() -> Self {
        Self {
            initial: 10_000.0, replenishment_rate: 5_000.0, max_capacity: 50_000.0,
            volatility: 0.0, season_period: 5000,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct EnergySection {
    pub metabolism_scale: f64,
    pub base_survival_cost: f64,
    pub survival_cost_scale: f64,
}
impl Default for EnergySection {
    fn default() -> Self {
        Self { metabolism_scale: 0.1, base_survival_cost: 1.0, survival_cost_scale: 0.05 }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct ReproSection {
    pub energy_cost: f64,
    pub child_energy_fraction: f64,
}
impl Default for ReproSection {
    fn default() -> Self {
        Self { energy_cost: 30.0, child_energy_fraction: 0.4 }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct LifespanSection {
    pub max_age: u64,
}
impl Default for LifespanSection {
    fn default() -> Self { Self { max_age: 100 } }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct LineageSection {
    pub divergence_threshold: u32,
    pub snapshot_interval: u64,
}
impl Default for LineageSection {
    fn default() -> Self { Self { divergence_threshold: 8, snapshot_interval: 1000 } }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct GenomeSection {
    pub seed: u64,
}
impl Default for GenomeSection {
    fn default() -> Self { Self { seed: 0xDEADBEEF } }
}

#[derive(Deserialize, Clone)]
#[serde(default)]
pub struct WorldSection {
    pub grid_size: usize,
    pub star_mass: f64,
    pub orbital_radius: f64,
    pub axial_tilt: f64,
    pub hydrosphere: f64,
    pub weather_volatility: f64,
    pub season_period: Option<u64>,
    pub migration_rate: f64,
    pub migration_cost: f64,
    pub mismatch_scale: f64,
}
impl Default for WorldSection {
    fn default() -> Self {
        Self {
            grid_size: 1, star_mass: 1.0, orbital_radius: 1.0, axial_tilt: 23.5,
            hydrosphere: 0.7, weather_volatility: 0.0, season_period: None,
            migration_rate: 0.0, migration_cost: 0.0, mismatch_scale: 0.0,
        }
    }
}

#[derive(Deserialize)]
#[serde(default)]
pub struct LogSection {
    pub dir: String,
}
impl Default for LogSection {
    fn default() -> Self { Self { dir: "logs".to_string() } }
}

impl TomlConfig {
    pub fn to_population_config(&self) -> PopulationConfig {
        let w = self.world.as_ref().cloned().unwrap_or_default();
        let season_period = w.season_period.unwrap_or(self.resources.season_period);
        PopulationConfig {
            max_population: self.population.max_size,
            initial_population: self.population.initial_size,
            initial_energy: self.population.initial_energy,
            metabolism_scale: self.energy.metabolism_scale,
            base_survival_cost: self.energy.base_survival_cost,
            survival_cost_scale: self.energy.survival_cost_scale,
            reproduction_cost: self.reproduction.energy_cost,
            child_energy_fraction: self.reproduction.child_energy_fraction,
            divergence_threshold: self.lineage.divergence_threshold,
            resource_replenishment: self.resources.replenishment_rate,
            resource_max_capacity: self.resources.max_capacity,
            initial_resources: self.resources.initial,
            snapshot_interval: self.lineage.snapshot_interval,
            max_age: self.lifespan.max_age,
            resource_volatility: self.resources.volatility,
            season_period,
            star_mass: w.star_mass,
            orbital_radius: w.orbital_radius,
            axial_tilt: w.axial_tilt,
            hydrosphere: w.hydrosphere,
            grid_size: w.grid_size,
            weather_volatility: w.weather_volatility,
            migration_rate: w.migration_rate,
            migration_cost: w.migration_cost,
            mismatch_scale: w.mismatch_scale,
        }
    }
}
