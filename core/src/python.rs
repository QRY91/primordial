use pyo3::prelude::*;

use crate::lineage::{LineageEvent, LineageEventType};
use crate::population::{Population, PopulationConfig, TickSummary};

#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct PyPopulationConfig {
    #[pyo3(get, set)]
    pub max_population: usize,
    #[pyo3(get, set)]
    pub initial_population: usize,
    #[pyo3(get, set)]
    pub initial_energy: f64,
    #[pyo3(get, set)]
    pub metabolism_scale: f64,
    #[pyo3(get, set)]
    pub base_survival_cost: f64,
    #[pyo3(get, set)]
    pub survival_cost_scale: f64,
    #[pyo3(get, set)]
    pub reproduction_cost: f64,
    #[pyo3(get, set)]
    pub child_energy_fraction: f64,
    #[pyo3(get, set)]
    pub divergence_threshold: u32,
    #[pyo3(get, set)]
    pub resource_replenishment: f64,
    #[pyo3(get, set)]
    pub resource_max_capacity: f64,
    #[pyo3(get, set)]
    pub initial_resources: f64,
    #[pyo3(get, set)]
    pub snapshot_interval: u64,
    #[pyo3(get, set)]
    pub max_age: u64,
    #[pyo3(get, set)]
    pub resource_volatility: f64,
    #[pyo3(get, set)]
    pub season_period: u64,
    #[pyo3(get, set)]
    pub star_mass: f64,
    #[pyo3(get, set)]
    pub orbital_radius: f64,
    #[pyo3(get, set)]
    pub axial_tilt: f64,
    #[pyo3(get, set)]
    pub hydrosphere: f64,
    #[pyo3(get, set)]
    pub grid_size: usize,
    #[pyo3(get, set)]
    pub weather_volatility: f64,
    #[pyo3(get, set)]
    pub migration_rate: f64,
    #[pyo3(get, set)]
    pub migration_cost: f64,
    #[pyo3(get, set)]
    pub mismatch_scale: f64,
}

#[pymethods]
impl PyPopulationConfig {
    #[new]
    #[pyo3(signature = (
        max_population=50000,
        initial_population=500,
        initial_energy=100.0,
        metabolism_scale=0.1,
        base_survival_cost=1.0,
        survival_cost_scale=0.05,
        reproduction_cost=30.0,
        child_energy_fraction=0.4,
        divergence_threshold=8,
        resource_replenishment=5000.0,
        resource_max_capacity=50000.0,
        initial_resources=10000.0,
        snapshot_interval=1000,
        max_age=100,
        resource_volatility=0.0,
        season_period=5000,
        star_mass=1.0,
        orbital_radius=1.0,
        axial_tilt=23.5,
        hydrosphere=0.7,
        grid_size=1,
        weather_volatility=0.0,
        migration_rate=0.0,
        migration_cost=0.0,
        mismatch_scale=0.0,
    ))]
    fn new(
        max_population: usize,
        initial_population: usize,
        initial_energy: f64,
        metabolism_scale: f64,
        base_survival_cost: f64,
        survival_cost_scale: f64,
        reproduction_cost: f64,
        child_energy_fraction: f64,
        divergence_threshold: u32,
        resource_replenishment: f64,
        resource_max_capacity: f64,
        initial_resources: f64,
        snapshot_interval: u64,
        max_age: u64,
        resource_volatility: f64,
        season_period: u64,
        star_mass: f64,
        orbital_radius: f64,
        axial_tilt: f64,
        hydrosphere: f64,
        grid_size: usize,
        weather_volatility: f64,
        migration_rate: f64,
        migration_cost: f64,
        mismatch_scale: f64,
    ) -> Self {
        Self {
            max_population,
            initial_population,
            initial_energy,
            metabolism_scale,
            base_survival_cost,
            survival_cost_scale,
            reproduction_cost,
            child_energy_fraction,
            divergence_threshold,
            resource_replenishment,
            resource_max_capacity,
            initial_resources,
            snapshot_interval,
            max_age,
            resource_volatility,
            season_period,
            star_mass,
            orbital_radius,
            axial_tilt,
            hydrosphere,
            grid_size,
            weather_volatility,
            migration_rate,
            migration_cost,
            mismatch_scale,
        }
    }
}

impl From<PyPopulationConfig> for PopulationConfig {
    fn from(py: PyPopulationConfig) -> Self {
        PopulationConfig {
            max_population: py.max_population,
            initial_population: py.initial_population,
            initial_energy: py.initial_energy,
            metabolism_scale: py.metabolism_scale,
            base_survival_cost: py.base_survival_cost,
            survival_cost_scale: py.survival_cost_scale,
            reproduction_cost: py.reproduction_cost,
            child_energy_fraction: py.child_energy_fraction,
            divergence_threshold: py.divergence_threshold,
            resource_replenishment: py.resource_replenishment,
            resource_max_capacity: py.resource_max_capacity,
            initial_resources: py.initial_resources,
            snapshot_interval: py.snapshot_interval,
            max_age: py.max_age,
            resource_volatility: py.resource_volatility,
            season_period: py.season_period,
            star_mass: py.star_mass,
            orbital_radius: py.orbital_radius,
            axial_tilt: py.axial_tilt,
            hydrosphere: py.hydrosphere,
            grid_size: py.grid_size,
            weather_volatility: py.weather_volatility,
            migration_rate: py.migration_rate,
            migration_cost: py.migration_cost,
            mismatch_scale: py.mismatch_scale,
        }
    }
}

#[pyclass]
pub struct PyPopulation {
    inner: Population,
}

#[pymethods]
impl PyPopulation {
    #[new]
    fn new(config: PyPopulationConfig, seed: u64) -> PyResult<Self> {
        Ok(Self {
            inner: Population::new(config.into(), seed),
        })
    }

    fn tick(&mut self, current_tick: u64) -> PyResult<PyTickSummary> {
        let summary = self
            .inner
            .tick(current_tick)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        Ok(PyTickSummary::from(summary))
    }

    fn drain_lineage_events(&mut self) -> Vec<PyLineageEvent> {
        self.inner
            .drain_lineage_events()
            .into_iter()
            .map(PyLineageEvent::from)
            .collect()
    }

    fn organism_count(&self) -> usize {
        self.inner.organism_count()
    }

    fn is_extinct(&self) -> bool {
        self.inner.is_extinct()
    }

    fn biome_populations(&self) -> Vec<(String, usize)> {
        self.inner.biome_populations()
    }

    fn cell_populations(&self) -> Vec<usize> {
        self.inner.cell_populations()
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct PyTickSummary {
    #[pyo3(get)]
    pub tick: u64,
    #[pyo3(get)]
    pub population_size: usize,
    #[pyo3(get)]
    pub births: usize,
    #[pyo3(get)]
    pub deaths: usize,
    #[pyo3(get)]
    pub active_lineages: usize,
    #[pyo3(get)]
    pub total_resources: f64,
    #[pyo3(get)]
    pub total_consumption: f64,
    #[pyo3(get)]
    pub avg_energy: f64,
    #[pyo3(get)]
    pub avg_metabolism: f64,
    #[pyo3(get)]
    pub avg_repro_threshold: f64,
    #[pyo3(get)]
    pub avg_mutation_rate: f64,
    #[pyo3(get)]
    pub genome_diversity: f64,
    #[pyo3(get)]
    pub season_modifier: f64,
    #[pyo3(get)]
    pub num_cells: usize,
    #[pyo3(get)]
    pub migrations: usize,
}

impl From<TickSummary> for PyTickSummary {
    fn from(s: TickSummary) -> Self {
        Self {
            tick: s.tick,
            population_size: s.population_size,
            births: s.births,
            deaths: s.deaths,
            active_lineages: s.active_lineages,
            total_resources: s.total_resources,
            total_consumption: s.total_consumption,
            avg_energy: s.avg_energy,
            avg_metabolism: s.avg_metabolism,
            avg_repro_threshold: s.avg_repro_threshold,
            avg_mutation_rate: s.avg_mutation_rate,
            genome_diversity: s.genome_diversity,
            season_modifier: s.season_modifier,
            num_cells: s.num_cells,
            migrations: s.migrations,
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct PyLineageEvent {
    #[pyo3(get)]
    pub event_type: String,
    #[pyo3(get)]
    pub tick: u64,
    #[pyo3(get)]
    pub lineage_id: u64,
    #[pyo3(get)]
    pub parent_lineage_id: Option<u64>,
    #[pyo3(get)]
    pub genome_snapshot: u64,
    #[pyo3(get)]
    pub population_count: u32,
}

impl From<LineageEvent> for PyLineageEvent {
    fn from(e: LineageEvent) -> Self {
        let event_type = match e.event_type {
            LineageEventType::Emerged => "emerged".to_string(),
            LineageEventType::Extinct => "extinct".to_string(),
            LineageEventType::Snapshot => "snapshot".to_string(),
        };
        Self {
            event_type,
            tick: e.tick,
            lineage_id: e.lineage_id,
            parent_lineage_id: e.parent_lineage_id,
            genome_snapshot: e.genome_snapshot,
            population_count: e.population_count,
        }
    }
}

#[pymodule]
fn primordial_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyPopulation>()?;
    m.add_class::<PyPopulationConfig>()?;
    m.add_class::<PyTickSummary>()?;
    m.add_class::<PyLineageEvent>()?;
    Ok(())
}
