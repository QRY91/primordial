# primordial

> The universe called `main()`. This is the wrapper.

An evolutionary simulation. Physical constants seed a star system. A star determines a habitable zone. A planet develops climate. Climate carves biomes. Biomes host organisms. Organisms compete, reproduce, mutate, and die. The phylogenetic tree writes itself.

Nobody hardcodes the outcomes.

---

## Quick Start

### Prerequisites
- Rust (stable, via [rustup](https://rustup.rs))
- Python 3.11+ (optional — for matplotlib dashboard and PyO3 bindings)

### Build

```bash
git clone https://github.com/QRY91/primordial
cd primordial

# Build CLI + GUI
cargo build --release
```

### Run a Simulation

```bash
# Via CLI
./target/release/primordial run -c sim/experiments/phase1.toml --max-ticks 5000

# List past runs
./target/release/primordial ls

# Show details of a run
./target/release/primordial show 1

# Generate matplotlib dashboard PNG
./target/release/primordial dashboard 1
```

### GUI

```bash
./target/release/primordial-gui
```

The GUI provides:
- **Run list** — browse all past simulations from shared SQLite database
- **Spatial grid map** — 6×6 biome-colored cells with live population density heatmap
- **6 interactive charts** — population, birth/death rates, trait evolution, genome diversity, biome populations, migrations (zoom/pan)
- **Live updating** — charts and grid map update in real-time during simulation
- **Run comparison** — overlay two runs' curves to compare outcomes
- **Config editor** — edit TOML config in-GUI with live validation, run directly

### Python Path (optional)

For the matplotlib dashboard or to run simulations via Python:

```bash
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
pip install maturin
maturin develop --release
.venv/bin/python sim/main.py --config sim/experiments/phase1.toml --max-ticks 5000
```

---

## Architecture

```
primordial/
├── core/           Rust — simulation engine
│   └── src/
│       ├── genome.rs       64-bit packed genome with 6 trait fields
│       ├── organism.rs     energy, age, reproduction, cell placement
│       ├── population.rs   tick loop: replenish → consume → cull → migrate → reproduce
│       ├── selection.rs    resource pools, scarcity math
│       ├── lineage.rs      divergence tracking, NDJSON event logging
│       └── python.rs       PyO3 bindings (feature-gated)
├── world/          Rust — environment model
│   └── src/
│       ├── star.rs         mass → luminosity, habitable zone
│       ├── planet.rs       orbital mechanics, equilibrium temperature
│       ├── climate.rs      latitude/longitude grid, seasonal cycles, weather
│       └── biome.rs        classification (tropical → ice), productivity
├── common/         Rust — shared config parsing + SQLite helpers
├── cli/            Rust — terminal binary (run/ls/show/dashboard)
├── gui/            Rust — egui desktop app with live charts + grid map
├── sim/            Python — orchestration, config, SQLite storage
├── viz/            Python — matplotlib 6-panel dashboard
└── logs/           Per-run subdirectories + shared primordial.sqlite
```

### Workspace Crates

| Crate | Purpose |
|-------|---------|
| `primordial-core` | Simulation engine — genome, organism, population, lineage, selection |
| `primordial-world` | Star → planet → climate → biome pipeline |
| `primordial-common` | TOML config parsing, SQLite schema + query helpers |
| `primordial` (cli) | Terminal binary with 4 subcommands |
| `primordial-gui` | egui desktop app with interactive charts |

---

## How It Works

### Genome

Each organism carries a 64-bit genome with 6 evolvable trait fields:

| Bits | Trait | Controls |
|------|-------|----------|
| 0–7 | Metabolism | Energy extraction rate (higher = more food but higher cost) |
| 8–15 | Repro threshold | Energy needed before reproduction triggers |
| 16–23 | Mutation rate | Per-bit flip probability during reproduction |
| 24–31 | Mobility | Migration probability + energy cost |
| 32–39 | Heat tolerance | Optimal temperature (-30°C to +50°C) |
| 40–47 | Moisture preference | Optimal moisture level (0.0 to 1.0) |

Mutation flips individual bits. Crossover swaps random byte-aligned segments between parents sharing a cell.

### World Model

A star's mass determines luminosity. Orbital radius and luminosity set equilibrium temperature. Axial tilt drives seasonal amplitude. A grid of cells gets latitude-based temperature gradients and longitude-wrapped moisture patterns. Each cell is classified into one of 6 biomes (tropical, desert, temperate forest, grassland, tundra, ice) with biome-specific resource productivity.

### Selection

Resources replenish per-cell based on biome productivity. Organisms consume resources proportional to metabolism. When resources are scarce, high-metabolism organisms pay more than they gain. Organisms that can't cover survival costs die. Those with enough energy reproduce — children inherit mutated genomes and stay in the parent's cell. Migration moves organisms between adjacent cells based on mobility trait.

Biome mismatch penalizes organisms whose heat tolerance / moisture preference doesn't match their cell's conditions, creating selective pressure for local adaptation and niche specialization.

### Lineage Tracking

A new lineage is assigned when a child's genome diverges from its parent by more than a configurable Hamming distance threshold. This produces a meaningful phylogenetic tree that tracks actual genetic divergence, not just reproduction events.

---

## Configuration

Simulations are defined in TOML. See [`sim/experiments/phase1.toml`](sim/experiments/phase1.toml) for a full example.

```toml
[simulation]
seed = 42
max_ticks = 100000
log_interval = 100

[population]
initial_size = 200
max_size = 50000

[world]
grid_size = 6
star_mass = 1.0
orbital_radius = 1.0
axial_tilt = 23.5
hydrosphere = 0.7
weather_volatility = 0.2
migration_rate = 0.08
migration_cost = 3.0
mismatch_scale = 1.5

[resources]
initial = 80000
replenishment_rate = 8000
max_capacity = 200000
```

---

## Storage

All interfaces (CLI, GUI, Python) share a single SQLite database at `logs/primordial.sqlite`:
- **runs** table — metadata, config, timing, final state
- **tick_summaries** table — population, births, deaths, diversity, biome populations per logged tick

Lineage events are stored as NDJSON files in per-run subdirectories (`logs/YYYYMMDD_HHMMSS_seedN/`) — different data shape, different storage.

---

## Philosophy

See [`NORTHSTAR.md`](NORTHSTAR.md).

The short version: this is a meditation with a compiler. We're not building a game or a biology textbook. We're building something that makes the question *"at what point does a record become a perceiver?"* feel like it has skin in the game.
