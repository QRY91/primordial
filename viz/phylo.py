"""Phylogenetic tree visualization for primordial.

Reads NDJSON lineage log and tick summaries, renders at-scale dashboards.
"""

import argparse
import json
import sys
import time
from collections import defaultdict
from pathlib import Path

import numpy as np


def load_events(path: Path) -> list[dict]:
    """Load all lineage events from NDJSON file."""
    events = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                events.append(json.loads(line))
    return events


def load_summaries(path: Path) -> list[dict]:
    """Load tick summaries from NDJSON file."""
    records = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                records.append(json.loads(line))
    return records


def build_tree(events: list[dict]) -> dict[int, dict]:
    """Build lineage tree from events."""
    tree: dict[int, dict] = {}

    for event in events:
        lid = event["lineage_id"]
        etype = event["event"]

        if etype == "emerged":
            tree[lid] = {
                "parent": event.get("parent_lineage_id"),
                "emerged_tick": event["tick"],
                "extinct_tick": None,
                "children": [],
                "genome": event.get("genome_snapshot", ""),
            }
            parent = event.get("parent_lineage_id")
            if parent is not None and parent in tree:
                tree[parent]["children"].append(lid)

        elif etype == "extinct":
            if lid in tree:
                tree[lid]["extinct_tick"] = event["tick"]

    return tree


def render_dashboard(tree: dict[int, dict], summaries: list[dict],
                     output: Path | None = None):
    """6-panel dashboard suited to large-scale simulations.

    Row 1: Population & diversity | Birth/death rates
    Row 2: Trait evolution        | Genome diversity + season
    Row 3: Lifespan distribution  | Lineage depth heatmap
    """
    import matplotlib.pyplot as plt
    from matplotlib.colors import LogNorm

    fig, axes = plt.subplots(3, 2, figsize=(18, 16))
    fig.suptitle("Primordial — Phylogenetic Dashboard", fontsize=14, fontweight="bold")

    # --- Extract time series from summaries ---
    ticks = [s["tick"] for s in summaries]
    pops = [s["population"] for s in summaries]
    lineage_counts = [s["lineages"] for s in summaries]
    births = [s["births"] for s in summaries]
    deaths = [s["deaths"] for s in summaries]
    resources = [s["resources"] for s in summaries]
    avg_metabolism = [s["avg_metabolism"] for s in summaries]
    avg_repro = [s.get("avg_repro_threshold", 0) for s in summaries]
    avg_mutrate = [s.get("avg_mutation_rate", 0) for s in summaries]
    diversity = [s.get("genome_diversity", 0) for s in summaries]
    season_mod = [s.get("season_modifier", 1.0) for s in summaries]

    # --- Panel 1: Population + Lineages ---
    ax1 = axes[0, 0]
    color_pop = "#2196F3"
    color_lin = "#FF9800"
    ax1.plot(ticks, pops, color=color_pop, linewidth=1, alpha=0.8, label="Population")
    ax1.set_ylabel("Population", color=color_pop)
    ax1.tick_params(axis="y", labelcolor=color_pop)
    ax1.set_ylim(bottom=0)

    ax1b = ax1.twinx()
    ax1b.plot(ticks, lineage_counts, color=color_lin, linewidth=1, alpha=0.8,
              label="Active lineages")
    ax1b.set_ylabel("Active Lineages", color=color_lin)
    ax1b.tick_params(axis="y", labelcolor=color_lin)
    ax1b.set_ylim(bottom=0)

    ax1.set_xlabel("Tick")
    ax1.set_title("Population & Diversity")
    lines1, labels1 = ax1.get_legend_handles_labels()
    lines2, labels2 = ax1b.get_legend_handles_labels()
    ax1.legend(lines1 + lines2, labels1 + labels2, loc="upper right", fontsize=8)

    # --- Panel 2: Birth/Death rates ---
    ax2 = axes[0, 1]
    window = max(1, len(ticks) // 50)
    births_arr = np.array(births, dtype=float)
    deaths_arr = np.array(deaths, dtype=float)

    if len(births_arr) > window:
        births_smooth = np.convolve(births_arr, np.ones(window) / window, mode="valid")
        deaths_smooth = np.convolve(deaths_arr, np.ones(window) / window, mode="valid")
        ticks_smooth = ticks[window - 1:]
    else:
        births_smooth = births_arr
        deaths_smooth = deaths_arr
        ticks_smooth = ticks

    ax2.fill_between(ticks_smooth, births_smooth, alpha=0.3, color="#4CAF50", label="Births")
    ax2.fill_between(ticks_smooth, -deaths_smooth, alpha=0.3, color="#F44336", label="Deaths")
    ax2.plot(ticks_smooth, births_smooth, color="#4CAF50", linewidth=0.5)
    ax2.plot(ticks_smooth, -deaths_smooth, color="#F44336", linewidth=0.5)
    ax2.axhline(0, color="gray", linewidth=0.5)
    ax2.set_xlabel("Tick")
    ax2.set_ylabel("Rate (per sample)")
    ax2.set_title("Birth / Death Rates (smoothed)")
    ax2.legend(fontsize=8)

    # --- Panel 3: Trait evolution over time ---
    ax3 = axes[1, 0]
    ax3.plot(ticks, avg_metabolism, color="#E91E63", linewidth=1, alpha=0.8,
             label="Metabolism")
    ax3.plot(ticks, avg_repro, color="#3F51B5", linewidth=1, alpha=0.8,
             label="Repro threshold")
    ax3.plot(ticks, avg_mutrate, color="#009688", linewidth=1, alpha=0.8,
             label="Mutation rate")
    ax3.set_xlabel("Tick")
    ax3.set_ylabel("Avg trait value (0-255)")
    ax3.set_title("Trait Evolution")
    ax3.legend(fontsize=8)
    ax3.set_ylim(bottom=0, top=255)

    # --- Panel 4: Genome diversity + seasonal modifier ---
    ax4 = axes[1, 1]
    ax4.plot(ticks, diversity, color="#9C27B0", linewidth=1, alpha=0.8,
             label="Genome diversity (Shannon)")
    ax4.set_ylabel("Diversity (0-1)", color="#9C27B0")
    ax4.tick_params(axis="y", labelcolor="#9C27B0")
    ax4.set_ylim(0, 1)

    ax4b = ax4.twinx()
    ax4b.plot(ticks, season_mod, color="#FF9800", linewidth=0.8, alpha=0.5,
              label="Season modifier", linestyle="--")
    ax4b.set_ylabel("Season modifier", color="#FF9800")
    ax4b.tick_params(axis="y", labelcolor="#FF9800")

    ax4.set_xlabel("Tick")
    ax4.set_title("Genome Diversity & Seasons")
    lines1, labels1 = ax4.get_legend_handles_labels()
    lines2, labels2 = ax4b.get_legend_handles_labels()
    ax4.legend(lines1 + lines2, labels1 + labels2, loc="upper right", fontsize=8)

    # --- Panel 5: Biome populations (Phase 1) or Lifespan distribution (Phase 0) ---
    ax5 = axes[2, 0]
    max_tick = max(ticks) if ticks else 1
    has_biomes = "biome_populations" in summaries[0] if summaries else False

    if has_biomes:
        # Stacked area chart of biome populations over time
        biome_colors = {
            "tropical": "#FF6F00",
            "desert": "#FFD54F",
            "temperate_forest": "#4CAF50",
            "grassland": "#8BC34A",
            "tundra": "#90CAF9",
            "ice": "#E0E0E0",
        }
        # Collect all biome names
        all_biomes = set()
        for s in summaries:
            all_biomes.update(s.get("biome_populations", {}).keys())
        biome_names = sorted(all_biomes)

        # Build time series per biome
        biome_series = {}
        for name in biome_names:
            biome_series[name] = [
                s.get("biome_populations", {}).get(name, 0) for s in summaries
            ]

        # Stacked area
        bottom = np.zeros(len(ticks))
        for name in biome_names:
            values = np.array(biome_series[name], dtype=float)
            color = biome_colors.get(name, "#999999")
            ax5.fill_between(ticks, bottom, bottom + values, alpha=0.7,
                             color=color, label=name.replace("_", " ").title())
            bottom += values
        ax5.set_xlabel("Tick")
        ax5.set_ylabel("Population")
        ax5.set_title("Population by Biome")
        ax5.legend(fontsize=7, loc="upper right")
        ax5.set_ylim(bottom=0)
    else:
        # Phase 0: lifespan distribution
        lifespans = []
        for node in tree.values():
            end = node["extinct_tick"] if node["extinct_tick"] is not None else max_tick
            span = end - node["emerged_tick"]
            if span > 0:
                lifespans.append(span)

        if lifespans:
            lifespans_arr = np.array(lifespans)
            bins = np.logspace(0, np.log10(max(lifespans_arr)), 50)
            ax5.hist(lifespans_arr, bins=bins, color="#9C27B0", alpha=0.7,
                     edgecolor="white", linewidth=0.3)
            ax5.set_xscale("log")
            ax5.set_yscale("log")
            median_life = np.median(lifespans_arr)
            ax5.axvline(median_life, color="red", linestyle="--", linewidth=1,
                        label=f"Median: {median_life:.0f} ticks")
            ax5.legend(fontsize=8)

        ax5.set_xlabel("Lifespan (ticks)")
        ax5.set_ylabel("Count")
        ax5.set_title("Lineage Lifespan Distribution")

    # --- Panel 6: Lineage activity heatmap (time bins vs generation depth) ---
    ax6 = axes[2, 1]

    # Compute generation depth for each lineage
    depths: dict[int, int] = {}
    def get_depth(lid: int) -> int:
        if lid in depths:
            return depths[lid]
        node = tree.get(lid)
        if node is None or node["parent"] is None or node["parent"] not in tree:
            depths[lid] = 0
            return 0
        d = get_depth(node["parent"]) + 1
        depths[lid] = d
        return d

    for lid in tree:
        get_depth(lid)

    if depths:
        max_depth = max(depths.values())
        n_time_bins = 100
        n_depth_bins = min(max_depth + 1, 50)
        time_edges = np.linspace(0, max_tick, n_time_bins + 1)
        depth_edges = np.linspace(0, max_depth + 1, n_depth_bins + 1)

        heatmap = np.zeros((n_depth_bins, n_time_bins))

        for lid, node in tree.items():
            start = node["emerged_tick"]
            end = node["extinct_tick"] if node["extinct_tick"] is not None else max_tick
            d = depths.get(lid, 0)

            # Find which time bins this lineage spans
            t_start_bin = np.searchsorted(time_edges, start, side="right") - 1
            t_end_bin = np.searchsorted(time_edges, end, side="right") - 1
            t_start_bin = max(0, min(t_start_bin, n_time_bins - 1))
            t_end_bin = max(0, min(t_end_bin, n_time_bins - 1))

            d_bin = np.searchsorted(depth_edges, d, side="right") - 1
            d_bin = max(0, min(d_bin, n_depth_bins - 1))

            heatmap[d_bin, t_start_bin:t_end_bin + 1] += 1

        # Log-scale color, zeros rendered as background
        cmap = plt.cm.inferno.copy()
        cmap.set_bad(color="#1a1a2e")
        heatmap_masked = np.ma.masked_where(heatmap == 0, heatmap)
        im = ax6.pcolormesh(time_edges, depth_edges, heatmap_masked,
                            cmap=cmap, norm=LogNorm(vmin=1),
                            rasterized=True)
        ax6.set_facecolor("#1a1a2e")
        fig.colorbar(im, ax=ax6, label="Active lineages", shrink=0.8)

    ax6.set_xlabel("Tick")
    ax6.set_ylabel("Generation Depth")
    ax6.set_title("Lineage Activity (depth x time)")

    plt.tight_layout()
    if output:
        plt.savefig(output, dpi=150, bbox_inches="tight")
        print(f"Saved to {output}")
    else:
        plt.show()


def render_terminal(tree: dict[int, dict], max_tick: int | None = None):
    """Print summary stats + top lineages to terminal."""
    if not tree:
        print("No lineage data.")
        return

    if max_tick is None:
        max_tick = max(
            (n.get("extinct_tick") or n.get("emerged_tick", 0)) for n in tree.values()
        )

    total = len(tree)
    extinct = sum(1 for n in tree.values() if n["extinct_tick"] is not None)
    alive = total - extinct

    # Compute lifespans
    lifespans = {}
    for lid, node in tree.items():
        end = node["extinct_tick"] if node["extinct_tick"] is not None else max_tick
        lifespans[lid] = end - node["emerged_tick"]

    # Top 20 longest-lived lineages
    top = sorted(lifespans.items(), key=lambda x: x[1], reverse=True)[:20]

    print(f"\nPhylogenetic Summary (0 -> {max_tick} ticks)")
    print("=" * 70)
    print(f"  Lineages: {total:,} total | {alive:,} surviving | {extinct:,} extinct")

    if lifespans:
        spans = list(lifespans.values())
        spans.sort()
        median = spans[len(spans) // 2]
        mean = sum(spans) / len(spans)
        p95 = spans[int(len(spans) * 0.95)]
        print(f"  Lifespan: median={median:,} | mean={mean:,.0f} | p95={p95:,} | max={spans[-1]:,}")

    # Generation depth
    depths: dict[int, int] = {}
    def get_depth(lid: int) -> int:
        if lid in depths:
            return depths[lid]
        node = tree.get(lid)
        if node is None or node["parent"] is None or node["parent"] not in tree:
            depths[lid] = 0
            return 0
        d = get_depth(node["parent"]) + 1
        depths[lid] = d
        return d

    for lid in tree:
        get_depth(lid)

    if depths:
        max_depth = max(depths.values())
        avg_depth = sum(depths.values()) / len(depths)
        print(f"  Depth:    max={max_depth} | avg={avg_depth:.1f}")

    print(f"\n  Top {len(top)} longest-lived lineages:")
    print(f"  {'ID':>8}  {'Born':>8}  {'Died':>8}  {'Span':>8}  {'Depth':>5}  Status")
    print(f"  {'-'*8}  {'-'*8}  {'-'*8}  {'-'*8}  {'-'*5}  ------")
    for lid, span in top:
        node = tree[lid]
        born = node["emerged_tick"]
        died = node["extinct_tick"]
        status = "ALIVE" if died is None else "extinct"
        died_str = str(died) if died is not None else "-"
        d = depths.get(lid, 0)
        print(f"  {lid:>8}  {born:>8}  {died_str:>8}  {span:>8,}  {d:>5}  {status}")

    # Show surviving lineages
    surviving = [(lid, lifespans[lid]) for lid, node in tree.items()
                 if node["extinct_tick"] is None]
    if surviving:
        print(f"\n  Surviving lineages ({len(surviving)}):")
        for lid, span in sorted(surviving, key=lambda x: x[1], reverse=True):
            node = tree[lid]
            d = depths.get(lid, 0)
            parent = node["parent"]
            print(f"    L{lid} (born={node['emerged_tick']}, span={span:,}, "
                  f"depth={d}, parent=L{parent})")
    print()


def follow_mode(path: Path):
    """Tail the NDJSON file and update terminal display."""
    print(f"Following {path} (Ctrl+C to stop)")
    events = []
    try:
        while True:
            new_events = load_events(path)
            if len(new_events) > len(events):
                events = new_events
                tree = build_tree(events)
                print("\033[2J\033[H", end="")
                render_terminal(tree)
            time.sleep(1.0)
    except KeyboardInterrupt:
        print("\nStopped.")


def load_from_db(db_path: Path, run_id: int | None = None):
    """Load summaries from SQLite, lineage events from NDJSON."""
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "sim"))
    from db import SimulationDB

    db = SimulationDB(db_path)
    if run_id is None:
        run_id = db.latest_run_id()
        if run_id is None:
            print("No runs in database.", file=sys.stderr)
            sys.exit(1)

    run = db.get_run(run_id)
    if run is None:
        print(f"Run #{run_id} not found.", file=sys.stderr)
        sys.exit(1)

    print(f"Loading run #{run_id}: seed={run['seed']} grid={run['grid_size']} "
          f"status={run['status']} ticks={run.get('final_tick', '?')}")

    summaries = db.get_summaries(run_id)
    db.close()

    # Load lineage events from NDJSON in the run's log directory
    log_dir = run.get("log_dir")
    tree_events = []
    if log_dir:
        phylo_path = Path(log_dir) / "phylogenetic_tree.json"
        if phylo_path.exists():
            tree_events = load_events(phylo_path)

    return tree_events, summaries


def list_runs(db_path: Path):
    """Print all runs in the database."""
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "sim"))
    from db import SimulationDB

    db = SimulationDB(db_path)
    runs = db.list_runs()
    db.close()

    if not runs:
        print("No runs found.")
        return

    print(f"{'ID':>4}  {'Status':>10}  {'Seed':>10}  {'Grid':>4}  "
          f"{'Ticks':>8}  {'Pop':>6}  {'Time':>8}  Started")
    print("-" * 80)
    for r in runs:
        elapsed = f"{r['elapsed_seconds']:.1f}s" if r['elapsed_seconds'] else "-"
        pop = str(r['final_population']) if r['final_population'] is not None else "-"
        ticks = str(r['final_tick']) if r['final_tick'] is not None else "-"
        print(f"{r['id']:>4}  {r['status']:>10}  {r['seed']:>10}  {r['grid_size']:>4}  "
              f"{ticks:>8}  {pop:>6}  {elapsed:>8}  {r['started_at']}")


def main():
    parser = argparse.ArgumentParser(description="Phylogenetic tree viewer")
    parser.add_argument("source", type=str, nargs="?", default=None,
                        help="NDJSON log file or DuckDB path")
    parser.add_argument("--db", type=Path, default=None,
                        help="DuckDB database path")
    parser.add_argument("--run", type=int, default=None,
                        help="Run ID (default: latest)")
    parser.add_argument("--list", action="store_true",
                        help="List all runs in the database")
    parser.add_argument("--summaries", "-s", type=Path, default=None,
                        help="Path to tick_summaries.ndjson")
    parser.add_argument("--follow", action="store_true", help="Live tail mode")
    parser.add_argument("--output", "-o", type=Path, default=None, help="Save plot to file")
    parser.add_argument("--terminal", "-t", action="store_true", help="Terminal summary")
    args = parser.parse_args()

    # Resolve DB path
    db_path = args.db
    if db_path is None and args.source and args.source.endswith(".sqlite"):
        db_path = Path(args.source)
    if db_path is None:
        candidate = Path("logs/primordial.sqlite")
        if candidate.exists():
            db_path = candidate

    # --list mode
    if args.list:
        if db_path is None:
            print("No database found. Pass --db path.", file=sys.stderr)
            sys.exit(1)
        list_runs(db_path)
        return

    # DB mode: load from DuckDB
    if db_path and db_path.exists() and (args.run is not None or args.source is None):
        raw_events, summaries = load_from_db(db_path, args.run)
        tree = build_tree(raw_events)
        if args.terminal:
            render_terminal(tree)
        elif not summaries:
            print("No summaries for this run.", file=sys.stderr)
            sys.exit(1)
        else:
            render_dashboard(tree, summaries, args.output)
        return

    # Legacy NDJSON mode
    log_file = Path(args.source) if args.source else None
    if log_file is None or not log_file.exists():
        print("Usage: phylo.py <log_file|--db path.sqlite> [--run N] [-o output.png]",
              file=sys.stderr)
        sys.exit(1)

    summary_path = args.summaries
    if summary_path is None:
        candidate = log_file.parent / "tick_summaries.ndjson"
        if candidate.exists():
            summary_path = candidate

    if args.follow:
        follow_mode(log_file)
    elif args.terminal:
        events = load_events(log_file)
        tree = build_tree(events)
        render_terminal(tree)
    else:
        events = load_events(log_file)
        tree = build_tree(events)
        summaries = load_summaries(summary_path) if summary_path else []
        if not summaries:
            print("No tick_summaries.ndjson found. Use --terminal or pass --summaries.",
                  file=sys.stderr)
            sys.exit(1)
        render_dashboard(tree, summaries, args.output)


if __name__ == "__main__":
    main()
