# ROADMAP.md
## Development Phases

---

### Philosophy of the Roadmap

Build the simplest honest version first. Resist premature complexity. The environment should earn its features by creating selection pressure that demands them. If adding climate doesn't change what survives, the climate model is decorative.

Each phase should be runnable and observable before the next begins. Emergence is the test, not feature completeness.

---

## Phase 0 — Skeleton
*Get the loop running. Prove the architecture.*

**Goal:** One biome. Organisms that tick, reproduce, and die. Selection from resource scarcity. Lineage tracked.

### Deliverables

- [ ] `sim/main.py` — tick loop, config loading, logging scaffold
- [ ] `sim/config.py` — TOML config parsing
- [ ] `core/src/organism.rs` — genome struct (64-bit), tick, reproduce, die
- [ ] `core/src/genome.rs` — mutation (bit-flip, rate configurable), crossover stub
- [ ] `core/src/population.rs` — container, parallel tick via rayon
- [ ] `core/src/selection.rs` — resource budget, cull below threshold
- [ ] `viz/phylo.py` — basic lineage tree, terminal or matplotlib render
- [ ] `logs/` — phylogenetic_tree.json, extinction_events.log writing

### Definition of Done
Run for 100,000 ticks. Watch at least one lineage go extinct and at least one diverge. The tree has branches.

### Notes
- Genome is a 64-bit integer. Traits decoded as bit fields. Start with: metabolism_rate (8 bits), reproduction_threshold (8 bits), mutation_rate (8 bits), mobility (8 bits), remainder reserved.
- Resources are a global pool, consumed per tick by metabolic rate, replenished at fixed rate. Scarcity is the only selection pressure at this stage. That's enough.
- No spatial model yet. All organisms share one abstract environment. Add space when the flat model's limitations become observable.

---

## Phase 1 — World
*Give the organisms somewhere to be.*

**Goal:** Spatial biomes derived from a planet's physical parameters. Climate as a driver of resource gradients. Migration between biomes.

### Deliverables

- [ ] `world/src/universe.rs` — constants, tick orchestration
- [ ] `world/src/star.rs` — mass, luminosity, habitable zone calculation
- [ ] `world/src/planet.rs` — orbital radius, axial tilt, hydrosphere fraction, gravity
- [ ] `world/src/climate.rs` — temperature × moisture grid, seasonal modulation, stochastic weather events
- [ ] `world/src/biome.rs` — derived from climate grid, resource gradients, adjacency graph for migration
- [ ] `core/src/organism.rs` — extend with biome affinity traits, migration logic
- [ ] `sim/main.py` — wire world into tick loop, pass biome state to populations

### Definition of Done
Three distinct biomes with different resource profiles. Organisms specialize under selection pressure — genomes dominant in harsh biomes differ measurably from temperate ones. A climate event shifts resource gradients and kills off a specialist lineage.

### Notes
- Biomes are emergent from climate math, not hardcoded. Temperature and moisture are the axes. Biome type is a label applied to a region of that space for observability, not a constraint.
- Two tick rates: weather (fast, stochastic) and geological (slow, deterministic). Don't conflate them.
- Migration is costly — mobility trait pays an energy penalty. This creates island dynamics organically.

---

## Phase 2 — Pressure
*Make time do damage. Make recovery interesting.*

**Goal:** Geological timescale events. Mass extinction as a feature. Adaptive radiation after extinction.

### Deliverables

- [ ] `world/src/star.rs` — stellar evolution curve, occasional flare events
- [ ] `world/src/planet.rs` — volcanic event scheduler, impact events (low probability, high consequence)
- [ ] `world/src/climate.rs` — extinction-scale climate shifts (volcanic winter, rapid warming)
- [ ] `core/src/selection.rs` — extinction threshold detection, event logging with full population snapshot
- [ ] `viz/climate_replay.py` — replay climate history against phylogenetic tree
- [ ] `viz/dashboard.py` — live dashboard: population diversity index, dominant genomes, climate state, event markers

### Definition of Done
A volcanic winter event reduces diversity by >80%. At least two lineages survive. Recovery produces measurable adaptive radiation — more lineage branching in the 10,000 ticks post-extinction than in the 10,000 ticks prior. The Cambrian moment is visible in the tree.

### Notes
- Extinction events should feel catastrophic but not arbitrary. They should emerge from physics (volcanic forcing → climate shift → resource collapse) not from a random event scheduler.
- The diversity index (Shannon entropy over genome distribution) is a useful single-number health metric. Log it every tick.
- At this phase, the simulation becomes genuinely interesting to watch. Invest in viz.

---

## Phase 3 — Depth
*Emergent complexity. Multicellularity. Rudimentary signal processing.*

**Goal:** Organisms that cooperate when cooperation is more fit than competition. Proto-neural trait. Ecological relationships (predation, symbiosis) as emergent outcomes.

### Deliverables

- [ ] `core/src/organism.rs` — colony detection: when N organisms share genome proximity and spatial adjacency, emergent colony entity forms
- [ ] `core/src/genome.rs` — neural_depth trait (4 bits): organisms with nonzero value have a signal-processing delay before acting, enabling anticipatory behavior
- [ ] `core/src/selection.rs` — trophic level detection: high-mobility, high-metabolism organisms that co-locate with low-mobility organisms select for predation behavior
- [ ] `sim/experiments/` — curated experiment configs that target specific emergence phenomena

### Definition of Done
At least one run produces a stable multicellular colony that persists longer than any unicellular lineage in the same environment. At least one run produces a stable predator/prey oscillation (Lotka-Volterra emerging, not specified).

### Notes
- Don't design multicellularity. Design the conditions under which it becomes more fit than the alternative. If it doesn't emerge, the fitness landscape needs adjustment, not the code.
- The neural_depth trait is the seed for Phase 4. Don't over-engineer it here. One bit that says "this organism delays action based on recent history" is enough.

---

## Phase 4 — Genome Language
*Performance art. The language is discovered, not designed.*

**Goal:** Replace the 64-bit integer genome with a minimal bytecode. The instruction set is derived from what Phase 0–3's evolution actually needed to express. The compiler is the simulation's own history.

### Framing

This phase is not planned in detail here intentionally. The genome language should emerge from a retrospective analysis of dominant genomes across simulation runs:

- What trait combinations were repeatedly selected for?
- What transitions (mutations) were most consequential?
- What expressions were never stable?

The answers to those questions define the instruction set. We're not designing a language — we're reading one out of the simulation's output.

### Likely Deliverables (subject to discovery)

- [ ] `genome_lang/` — new top-level module
- [ ] `genome_lang/src/bytecode.rs` — instruction set (expect 8–16 opcodes)
- [ ] `genome_lang/src/interpreter.rs` — organism tick as bytecode execution
- [ ] `genome_lang/src/mutator.rs` — mutation as bytecode surgery (insert, delete, substitute instruction)
- [ ] `genome_lang/src/compiler.rs` — compile 64-bit legacy genome to bytecode for continuity
- [ ] Analysis notebook: dominant genome patterns across 10+ long runs

### Definition of Done
A simulation run using the genome language produces emergent outcomes qualitatively similar to Phase 2 runs, with measurably more expressive genome diversity. The instruction set has fewer than 20 opcodes. Each opcode exists because the simulation needed it.

---

## Ongoing

- **Checkpointing** — save/restore simulation state at any tick. Essential for long runs.
- **Experiment framework** — parameter sweeps, reproducible seeds, comparison across runs
- **Performance profiling** — identify actual bottlenecks before rewriting anything
- **Documentation** — every non-obvious design decision gets a comment explaining *why*, not *what*

---

## What's Not On The Roadmap

- **Spatial 3D environments** — 2D biome grid is sufficient. 3D adds complexity without selection pressure.
- **Graphical organism rendering** — the tree is the product, not individual organism animation.
- **Networking/multiplayer** — the universe is already parallel enough.
- **AI-directed evolution** — selection pressure must come from the environment, not a trained model. That would collapse the point.
