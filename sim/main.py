"""Main simulation loop for primordial Phase 0/1."""

import argparse
import dataclasses
import json
import logging
import sys
import time
from datetime import datetime
from pathlib import Path

logger = logging.getLogger("primordial")


def main():
    parser = argparse.ArgumentParser(description="primordial evolutionary simulation")
    parser.add_argument("--config", type=Path, required=True, help="Path to TOML config")
    parser.add_argument("--max-ticks", type=int, default=None, help="Override max ticks")
    parser.add_argument("--no-db", action="store_true", help="Skip SQLite storage")
    args = parser.parse_args()

    config_path = args.config if args.config.is_absolute() else Path.cwd() / args.config

    sys.path.insert(0, str(Path(__file__).parent))
    from config import load_config, config_to_population_config
    from db import SimulationDB

    config = load_config(config_path)
    if args.max_ticks is not None:
        config.max_ticks = args.max_ticks

    logging.basicConfig(level=logging.INFO, format="%(message)s")

    # Per-run subdirectory: logs/YYYYMMDD_HHMMSS_seed{seed}/
    base_log_dir = Path(config.log_dir)
    base_log_dir.mkdir(parents=True, exist_ok=True)
    run_stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    run_dir = base_log_dir / f"{run_stamp}_seed{config.seed}"
    run_dir.mkdir(parents=True, exist_ok=True)

    try:
        import primordial_core as core  # noqa: F401
    except ImportError:
        print(
            "ERROR: primordial_core not built. Run: maturin develop --release",
            file=sys.stderr,
        )
        sys.exit(1)

    pop_config = config_to_population_config(config)
    population = core.PyPopulation(pop_config, config.seed)

    # Per-run log files
    phylo_path = run_dir / "phylogenetic_tree.json"
    extinction_path = run_dir / "extinction_events.log"
    summary_path = run_dir / "tick_summaries.ndjson"

    # SQLite (shared across runs, in base log dir — same DB as Rust CLI)
    db = None
    run_id = None
    if not args.no_db:
        db_path = base_log_dir / "primordial.sqlite"
        db = SimulationDB(db_path)
        config_dict = dataclasses.asdict(config)
        run_id = db.start_run(
            config.seed, config_dict, config.max_ticks, config.grid_size,
            log_dir=str(run_dir.resolve()),
        )
        logger.info("[run #%d] %s", run_id, run_dir)

    is_spatial = config.grid_size > 1
    if is_spatial:
        logger.info(
            "[world] grid=%dx%d star=%.1fM orbital=%.1fAU tilt=%.1f hydro=%.1f",
            config.grid_size, config.grid_size, config.star_mass,
            config.orbital_radius, config.axial_tilt, config.hydrosphere,
        )

    start_time = time.time()
    last_tick = 0
    final_status = "completed"

    with (
        open(phylo_path, "w") as phylo_log,
        open(extinction_path, "w") as extinction_log,
        open(summary_path, "w") as summary_log,
    ):
        for tick in range(config.max_ticks):
            last_tick = tick
            summary = population.tick(tick)

            for event in population.drain_lineage_events():
                record = {
                    "event": event.event_type,
                    "tick": event.tick,
                    "lineage_id": event.lineage_id,
                    "parent_lineage_id": event.parent_lineage_id,
                    "genome_snapshot": format(event.genome_snapshot, "064b"),
                    "population_count": event.population_count,
                }
                phylo_log.write(json.dumps(record) + "\n")

                if event.event_type == "extinct":
                    extinction_log.write(
                        f"tick={event.tick} lineage={event.lineage_id} "
                        f"genome={event.genome_snapshot:#018x}\n"
                    )

            if tick % config.log_interval == 0:
                summary_record = {
                    "tick": summary.tick,
                    "population": summary.population_size,
                    "births": summary.births,
                    "deaths": summary.deaths,
                    "lineages": summary.active_lineages,
                    "resources": round(summary.total_resources, 2),
                    "avg_energy": round(summary.avg_energy, 2),
                    "avg_metabolism": round(summary.avg_metabolism, 2),
                    "avg_repro_threshold": round(summary.avg_repro_threshold, 2),
                    "avg_mutation_rate": round(summary.avg_mutation_rate, 2),
                    "genome_diversity": round(summary.genome_diversity, 4),
                    "season_modifier": round(summary.season_modifier, 3),
                    "num_cells": summary.num_cells,
                    "migrations": summary.migrations,
                }
                if is_spatial:
                    biome_pops = population.biome_populations()
                    summary_record["biome_populations"] = {
                        name: count for name, count in biome_pops
                    }
                summary_log.write(json.dumps(summary_record) + "\n")
                if db:
                    db.insert_summary(summary_record)

                if is_spatial:
                    biome_str = " ".join(
                        f"{name}={count}" for name, count in population.biome_populations()
                    )
                    logger.info(
                        "[tick %d] pop=%d lineages=%d resources=%.0f "
                        "births=%d deaths=%d migr=%d | %s",
                        tick,
                        summary.population_size,
                        summary.active_lineages,
                        summary.total_resources,
                        summary.births,
                        summary.deaths,
                        summary.migrations,
                        biome_str,
                    )
                else:
                    logger.info(
                        "[tick %d] pop=%d lineages=%d resources=%.0f births=%d deaths=%d",
                        tick,
                        summary.population_size,
                        summary.active_lineages,
                        summary.total_resources,
                        summary.births,
                        summary.deaths,
                    )

            if population.is_extinct():
                logger.warning("[tick %d] TOTAL EXTINCTION", tick)
                final_status = "extinct"
                break

            if tick % 1000 == 0:
                phylo_log.flush()
                extinction_log.flush()
                summary_log.flush()
                if db:
                    db.flush()

    elapsed = time.time() - start_time
    final_pop = population.organism_count()
    logger.info(
        "[done] %d ticks in %.1fs (%.0f ticks/s)",
        last_tick + 1,
        elapsed,
        (last_tick + 1) / elapsed if elapsed > 0 else 0,
    )

    if db:
        db.finalize_run(last_tick, elapsed, final_pop, final_status)
        logger.info("[db] run #%d finalized (%s)", run_id, final_status)
        db.close()


if __name__ == "__main__":
    main()
