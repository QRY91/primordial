"""TOML config parsing for primordial simulations."""

import tomllib
from dataclasses import dataclass
from pathlib import Path


@dataclass
class SimulationConfig:
    """All simulation parameters. Loaded from TOML, never hardcoded."""

    # Simulation control
    seed: int = 42
    max_ticks: int = 100_000
    time_compression: int = 1000
    log_interval: int = 100
    checkpoint_interval: int = 10_000

    # Population
    initial_population: int = 500
    max_population: int = 50_000
    initial_energy: float = 100.0

    # Resources
    initial_resources: float = 10_000.0
    resource_replenishment: float = 5_000.0
    resource_max_capacity: float = 50_000.0

    # Energy model
    metabolism_scale: float = 0.1
    base_survival_cost: float = 1.0
    survival_cost_scale: float = 0.05

    # Reproduction
    reproduction_cost: float = 30.0
    child_energy_fraction: float = 0.4

    # Lifespan
    max_age: int = 100

    # Lineage
    divergence_threshold: int = 8
    snapshot_interval: int = 1000

    # Genome
    genome_seed: int = 0xDEADBEEF

    # Seasons (Phase 0 compat)
    resource_volatility: float = 0.0
    season_period: int = 5000

    # World (Phase 1)
    star_mass: float = 1.0
    orbital_radius: float = 1.0
    axial_tilt: float = 23.5
    hydrosphere: float = 0.7
    grid_size: int = 1
    weather_volatility: float = 0.0

    # Migration (Phase 1)
    migration_rate: float = 0.0
    migration_cost: float = 0.0
    mismatch_scale: float = 0.0

    # Logging
    log_dir: str = "logs"
    extinction_threshold: float = 0.05


def load_config(path: Path) -> SimulationConfig:
    """Load config from TOML file. Missing fields use defaults."""
    with open(path, "rb") as f:
        raw = tomllib.load(f)

    config = SimulationConfig()

    sim = raw.get("simulation", {})
    config.seed = sim.get("seed", config.seed)
    config.max_ticks = sim.get("max_ticks", config.max_ticks)
    config.time_compression = sim.get("time_compression", config.time_compression)
    config.log_interval = sim.get("log_interval", config.log_interval)
    config.checkpoint_interval = sim.get("checkpoint_interval", config.checkpoint_interval)

    pop = raw.get("population", {})
    config.initial_population = pop.get("initial_size", config.initial_population)
    config.max_population = pop.get("max_size", config.max_population)
    config.initial_energy = pop.get("initial_energy", config.initial_energy)

    res = raw.get("resources", {})
    config.initial_resources = res.get("initial", config.initial_resources)
    config.resource_replenishment = res.get("replenishment_rate", config.resource_replenishment)
    config.resource_max_capacity = res.get("max_capacity", config.resource_max_capacity)
    config.resource_volatility = res.get("volatility", config.resource_volatility)
    config.season_period = res.get("season_period", config.season_period)

    energy = raw.get("energy", {})
    config.metabolism_scale = energy.get("metabolism_scale", config.metabolism_scale)
    config.base_survival_cost = energy.get("base_survival_cost", config.base_survival_cost)
    config.survival_cost_scale = energy.get("survival_cost_scale", config.survival_cost_scale)

    repro = raw.get("reproduction", {})
    config.reproduction_cost = repro.get("energy_cost", config.reproduction_cost)
    config.child_energy_fraction = repro.get("child_energy_fraction", config.child_energy_fraction)

    life = raw.get("lifespan", {})
    config.max_age = life.get("max_age", config.max_age)

    lin = raw.get("lineage", {})
    config.divergence_threshold = lin.get("divergence_threshold", config.divergence_threshold)
    config.snapshot_interval = lin.get("snapshot_interval", config.snapshot_interval)

    gen = raw.get("genome", {})
    config.genome_seed = gen.get("seed", config.genome_seed)

    # Phase 1: World
    world = raw.get("world", {})
    config.star_mass = world.get("star_mass", config.star_mass)
    config.orbital_radius = world.get("orbital_radius", config.orbital_radius)
    config.axial_tilt = world.get("axial_tilt", config.axial_tilt)
    config.hydrosphere = world.get("hydrosphere", config.hydrosphere)
    config.grid_size = world.get("grid_size", config.grid_size)
    config.weather_volatility = world.get("weather_volatility", config.weather_volatility)
    config.season_period = world.get("season_period", config.season_period)
    config.migration_rate = world.get("migration_rate", config.migration_rate)
    config.migration_cost = world.get("migration_cost", config.migration_cost)
    config.mismatch_scale = world.get("mismatch_scale", config.mismatch_scale)

    log = raw.get("logging", {})
    config.log_dir = log.get("dir", config.log_dir)
    config.extinction_threshold = log.get("extinction_threshold", config.extinction_threshold)

    return config


def config_to_population_config(config: SimulationConfig):
    """Convert SimulationConfig to primordial_core.PyPopulationConfig."""
    import primordial_core as core

    return core.PyPopulationConfig(
        max_population=config.max_population,
        initial_population=config.initial_population,
        initial_energy=config.initial_energy,
        metabolism_scale=config.metabolism_scale,
        base_survival_cost=config.base_survival_cost,
        survival_cost_scale=config.survival_cost_scale,
        reproduction_cost=config.reproduction_cost,
        child_energy_fraction=config.child_energy_fraction,
        divergence_threshold=config.divergence_threshold,
        resource_replenishment=config.resource_replenishment,
        resource_max_capacity=config.resource_max_capacity,
        initial_resources=config.initial_resources,
        snapshot_interval=config.snapshot_interval,
        max_age=config.max_age,
        resource_volatility=config.resource_volatility,
        season_period=config.season_period,
        star_mass=config.star_mass,
        orbital_radius=config.orbital_radius,
        axial_tilt=config.axial_tilt,
        hydrosphere=config.hydrosphere,
        grid_size=config.grid_size,
        weather_volatility=config.weather_volatility,
        migration_rate=config.migration_rate,
        migration_cost=config.migration_cost,
        mismatch_scale=config.mismatch_scale,
    )
