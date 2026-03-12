# CLAUDE.md
## Instructions for Claude Code

Read this before touching anything.

---

## What This Project Is

An evolutionary simulation. Physical constants seed a universe. A universe seeds a star. A star determines habitability. A planet develops climate. Climate carves biomes. Biomes host organisms. Organisms compete, reproduce, mutate, and die. Nobody hardcodes the outcomes.

Read `NORTHSTAR.md` for the philosophy. Read `ROADMAP.md` for the plan. This file is about how to work.

---

## Current Phase

**Check `ROADMAP.md`** for which phase is active. Only work within the current phase's scope unless explicitly told otherwise. Do not implement Phase 2 features while Phase 0 is incomplete. Premature complexity is the primary failure mode of this project.

---

## Architecture

```
primordial/
├── core/       Rust  — organism tick, genome, population, selection (HOT PATH)
├── world/      Rust  — universe, star, planet, climate, biome
├── sim/        Python — orchestration, config, experiment management
├── viz/        Python — phylogenetic rendering, dashboards, analysis
└── logs/       Output — phylogenetic_tree.json, extinction_events.log, etc.
```

The Rust/Python boundary is managed via **PyO3**. Python calls into Rust for compute-intensive operations. Python owns configuration, experiment management, and visualization.

---

## Principles — Read These Carefully

### 1. Emergence over specification
If something interesting needs to be hardcoded, that is a design failure, not an implementation task. Biomes should emerge from climate math. Extinction should emerge from physics. Predation should emerge from trait interactions under resource scarcity. If you find yourself writing `if organism.type == "predator"`, stop and reconsider the model.

### 2. The fitness function is the environment
Do not add selection pressure by writing selection logic. Add it by changing environmental parameters and letting the organism tick loop + resource scarcity do the work. Selection code should only enforce resource limits and cull organisms that fall below survival threshold.

### 3. The phylogenetic tree is the product
Every architectural decision should be evaluated against: *does this make the tree more interesting or more legible?* Not: does this make individual organisms more sophisticated.

### 4. Measure before optimizing
Do not rewrite Python in Rust because it feels slow. Profile first. The hot path is the organism tick loop and population-level selection. Everything else is probably fast enough. Use `cargo flamegraph` for Rust, `py-spy` for Python.

### 5. Ticks are not wall time
A tick represents configurable compressed time (default: 1 tick = 1000 years). Climate operates on geological ticks (slower). Weather operates on fast ticks (more frequent). These are separate loop rates within a single tick orchestration. Do not conflate them.

---

## Rust Conventions

### Organism genome
The genome is a `u64`. Traits are decoded as bit fields:

```rust
pub struct Genome(pub u64);

impl Genome {
    pub fn metabolism_rate(&self) -> u8  { (self.0 & 0xFF) as u8 }
    pub fn repro_threshold(&self) -> u8  { ((self.0 >> 8) & 0xFF) as u8 }
    pub fn mutation_rate(&self) -> u8    { ((self.0 >> 16) & 0xFF) as u8 }
    pub fn mobility(&self) -> u8         { ((self.0 >> 24) & 0xFF) as u8 }
    // bits 32–63: reserved for Phase 2+ traits
}
```

Do not change the genome struct layout without updating this documentation and the genome language compiler (Phase 4).

### Parallelism
Use `rayon` for population-level parallelism. The organism tick loop is embarrassingly parallel — organisms in a population do not communicate during a tick, only during reproduction and selection which happen sequentially after.

```rust
use rayon::prelude::*;
population.par_iter_mut().for_each(|org| org.tick(&biome_snapshot));
```

Do not use `std::thread` directly for population parallelism. Use `rayon`.

### Error handling
Use `thiserror` for library errors. No `unwrap()` in library code. `unwrap()` is acceptable in `main.rs` and test code only.

### Logging
Use the `log` crate with `env_logger`. Do not use `println!` in library code.

```rust
log::info!("tick {}: population {} -> {}", tick, prev_count, curr_count);
```

---

## Python Conventions

### Configuration
All simulation parameters live in TOML files under `sim/experiments/`. Never hardcode a parameter value in Python code. If you find yourself writing `max_ticks = 1_000_000` in a `.py` file, move it to config.

### Logging output
The simulation produces structured log files in `logs/`. Append-only. Never truncate a log mid-run. The phylogenetic tree JSON is newline-delimited records (NDJSON), not a single JSON array — this allows streaming and avoids loading the full tree into memory.

```python
# Correct — NDJSON
with open("logs/phylogenetic_tree.json", "a") as f:
    f.write(json.dumps(lineage_record) + "\n")
```

### Calling Rust from Python
The PyO3 bridge exposes the Rust core as a Python module `primordial_core`. Import it as:

```python
import primordial_core as core
population = core.Population(config.initial_size, config.genome_seed)
population.tick(biome_snapshot)
```

If the Rust module isn't built, fail loudly with a helpful message. Do not fall back to a Python implementation silently — the Python fallback will be too slow and results will not be comparable.

---

## What Not To Do

- **Do not add features not in the current roadmap phase.** If you think something should be added, add it to ROADMAP.md under a future phase and note the rationale.
- **Do not render individual organisms.** The tree is the product.
- **Do not add an AI/ML model to guide evolution.** Selection pressure comes from the environment only.
- **Do not use `localStorage`, browser APIs, or anything web-facing.** This is a CLI simulation.
- **Do not write synchronous file I/O in the Rust hot path.** Log async or buffer and flush.
- **Do not hardcode biome types.** Biomes are emergent labels over a climate grid.

---

## Testing

Each Rust module has unit tests in `#[cfg(test)]` blocks. Run with `cargo test`.

Key invariants to test:
- Genome mutation preserves bit length (always `u64`)
- Organism energy never goes negative (die before that)
- Population size never exceeds configured maximum (selection enforces this)
- Phylogenetic log entries are monotonically increasing by tick

Python tests live in `tests/` and use `pytest`. Key invariants:
- Config parsing fails loudly on missing required fields
- Log output is valid NDJSON
- Simulation produces nonzero extinction events on 100k tick runs with default config

---

## Running

```bash
# Full run
python sim/main.py --config sim/experiments/default.toml

# Short test run
python sim/main.py --config sim/experiments/default.toml --max-ticks 10000

# Resume from checkpoint
python sim/main.py --resume logs/checkpoints/tick_50000.pkl

# Watch the tree live
python viz/phylo.py --follow logs/phylogenetic_tree.json
```

---

## When You're Unsure

Ask. The codebase has opinions and they're documented here and in NORTHSTAR.md. If something feels like it should be hardcoded, it probably shouldn't be. If something feels like it needs more complexity, check whether the environment model needs adjustment first.

The universe started simple. So should the code.
