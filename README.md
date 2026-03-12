# primordial

> The universe called `main()`. This is the wrapper.

A compressed evolutionary simulation. Physical constants seed a star system. A star determines a habitable zone. A planet develops climate. Climate carves biomes. Biomes host organisms. Organisms compete, reproduce, mutate, and die. The phylogenetic tree writes itself.

Nobody hardcodes the outcomes.

---

## What This Is

`primordial` is a multi-scale evolutionary simulation focused on **emergent complexity over directed design**. The goal is not biological accuracy — it's to watch selection logic operate: extinction, recovery, divergence, the occasional Cambrian moment when cleared fitness landscape suddenly fills.

The phylogenetic tree is the primary artifact. Living organisms at any given tick are less interesting than the history of what survived, what didn't, and why.

---

## Architecture Overview

```
primordial/
├── core/               # Rust — hot path (organism ticks, selection, population)
│   ├── src/
│   │   ├── organism.rs
│   │   ├── genome.rs
│   │   ├── population.rs
│   │   ├── selection.rs
│   │   └── lib.rs
├── world/              # Rust — environment (climate, biome, physics)
│   ├── src/
│   │   ├── universe.rs
│   │   ├── star.rs
│   │   ├── planet.rs
│   │   ├── climate.rs
│   │   └── biome.rs
├── sim/                # Python — orchestration, config, experiment management
│   ├── main.py
│   ├── config.py
│   └── experiments/
├── viz/                # Python — phylogenetic rendering, analysis, dashboards
│   ├── phylo.py
│   ├── climate_replay.py
│   └── dashboard.py
├── logs/               # Simulation output
│   ├── phylogenetic_tree.json
│   ├── extinction_events.log
│   ├── climate_history.csv
│   └── dominant_genomes.json
├── NORTHSTAR.md        # Vision and philosophy — read before contributing
├── ROADMAP.md          # Phased development plan
└── CLAUDE.md           # Instructions for Claude Code
```

---

## Quick Start

### Prerequisites
- Python 3.11+
- Rust (stable, via rustup)
- [PyO3](https://pyo3.rs) for the Rust/Python bridge

### Setup

```bash
# Clone
git clone https://github.com/yourname/primordial
cd primordial

# Python environment
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# Build Rust core
cd core && cargo build --release && cd ..
cd world && cargo build --release && cd ..

# Run
python sim/main.py --config sim/experiments/default.toml
```

### First Run Output

```
[tick 0]      universe initialized. seed=42. constants locked.
[tick 1]      star ignited. luminosity=1.0 sol. habitable zone: 0.95–1.37 AU
[tick 12]     planet settled at 1.02 AU. hydrosphere forming.
[tick 340]    first biomes stable. 3 zones active.
[tick 1,200]  first organisms seeded. genome_length=64bit.
[tick 8,400]  population: 12,400. lineages: 7.
[tick 23,100] extinction event. volcanic. diversity: -84%.
[tick 23,800] recovery underway. 3 lineages survived.
              (this is where it gets interesting)
```

---

## Configuration

Simulations are defined in TOML:

```toml
[universe]
seed = 42
time_compression = 1000  # 1 tick = N years

[star]
mass = 1.0               # solar masses
spectral_class = "G"

[planet]
orbital_radius = 1.02    # AU
axial_tilt = 23.5        # degrees
hydrosphere = 0.71       # surface fraction

[simulation]
max_ticks = 1_000_000
checkpoint_interval = 10_000
extinction_threshold = 0.05  # population fraction triggers event logging
```

---

## Output and Observation

The primary output is the **phylogenetic tree** — a JSON log of every lineage: when it emerged, from what ancestor, how long it persisted, and what killed it.

```json
{
  "lineage_id": "L-0042",
  "emerged_tick": 8400,
  "parent": "L-0007",
  "genome_snapshot": "01101001...",
  "dominant_biome": "temperate_shallow",
  "extinction_tick": 23100,
  "cause": "volcanic_winter",
  "descendants_surviving": 2
}
```

A live dashboard (`viz/dashboard.py`) renders the tree as it grows and prunes.

---

## Philosophy

See `NORTHSTAR.md`.

The short version: this is a meditation with a compiler. We're not building a game or a biology textbook. We're building something that makes the question *"at what point does a record become a perceiver?"* feel like it has skin in the game.
