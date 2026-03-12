use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use chrono::Local;
use clap::{Parser, Subcommand};
use primordial_core::lineage::LineageEventType;
use primordial_core::population::Population;

use primordial_common::config;
use primordial_common::db;

// ── CLI ──────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "primordial", about = "Evolutionary simulation")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a simulation
    Run {
        /// Path to TOML config
        #[arg(short, long)]
        config: PathBuf,
        /// Override max ticks
        #[arg(long)]
        max_ticks: Option<u64>,
    },
    /// List all runs
    Ls,
    /// Show details of a run
    Show {
        /// Run ID (default: latest)
        id: Option<i64>,
    },
    /// Generate dashboard PNG
    Dashboard {
        /// Run ID (default: latest)
        id: Option<i64>,
        /// Output path (default: <run_dir>/dashboard.png)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

// ── Run command ──────────────────────────────────────────────────────────

fn cmd_run(config_path: &Path, max_ticks_override: Option<u64>) {
    let toml_cfg = config::load_config(config_path)
        .unwrap_or_else(|e| panic!("{e}"));
    let raw = fs::read_to_string(config_path).unwrap();

    let max_ticks = max_ticks_override.unwrap_or(toml_cfg.simulation.max_ticks);
    let seed = toml_cfg.simulation.seed;
    let log_interval = toml_cfg.simulation.log_interval;
    let pop_config = toml_cfg.to_population_config();
    let grid_size = pop_config.grid_size;
    let is_spatial = grid_size > 1;

    // Per-run directory
    let base_dir = PathBuf::from(&toml_cfg.logging.dir);
    fs::create_dir_all(&base_dir).ok();
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    let run_dir = base_dir.join(format!("{stamp}_seed{seed}"));
    fs::create_dir_all(&run_dir).expect("failed to create run dir");

    // DB
    let conn = db::open_db(&base_dir);
    let config_json = serde_json::to_string(&raw).unwrap_or_default();
    let now = Local::now().to_rfc3339();
    let run_id = db::insert_run(
        &conn, &now, seed as i64, &config_json,
        max_ticks as i64, grid_size as i64,
        &run_dir.to_string_lossy(),
    );

    eprintln!("[run #{run_id}] {}", run_dir.display());
    if is_spatial {
        eprintln!(
            "[world] grid={g}x{g} star={m:.1}M orbital={r:.1}AU",
            g = grid_size, m = pop_config.star_mass, r = pop_config.orbital_radius,
        );
    }

    // Log files
    let phylo_file = fs::File::create(run_dir.join("phylogenetic_tree.json")).unwrap();
    let extinction_file = fs::File::create(run_dir.join("extinction_events.log")).unwrap();
    let summary_file = fs::File::create(run_dir.join("tick_summaries.ndjson")).unwrap();
    let mut phylo_w = BufWriter::new(phylo_file);
    let mut extinct_w = BufWriter::new(extinction_file);
    let mut summary_w = BufWriter::new(summary_file);

    // Simulation
    let mut population = Population::new(pop_config, seed);
    let start = Instant::now();
    let mut last_tick = 0u64;
    let mut status = "completed";

    for tick in 0..max_ticks {
        last_tick = tick;
        let summary = population.tick(tick).expect("tick failed");

        // Lineage events → NDJSON
        for event in population.drain_lineage_events() {
            let etype = match event.event_type {
                LineageEventType::Emerged => "emerged",
                LineageEventType::Extinct => "extinct",
                LineageEventType::Snapshot => "snapshot",
            };
            let record = serde_json::json!({
                "event": etype,
                "tick": event.tick,
                "lineage_id": event.lineage_id,
                "parent_lineage_id": event.parent_lineage_id,
                "genome_snapshot": format!("{:064b}", event.genome_snapshot),
                "population_count": event.population_count,
            });
            serde_json::to_writer(&mut phylo_w, &record).ok();
            phylo_w.write_all(b"\n").ok();

            if matches!(event.event_type, LineageEventType::Extinct) {
                writeln!(
                    extinct_w, "tick={} lineage={} genome={:#018x}",
                    event.tick, event.lineage_id, event.genome_snapshot
                ).ok();
            }
        }

        // Periodic summary
        if tick % log_interval == 0 {
            let mut record = serde_json::json!({
                "tick": summary.tick,
                "population": summary.population_size,
                "births": summary.births,
                "deaths": summary.deaths,
                "lineages": summary.active_lineages,
                "resources": (summary.total_resources * 100.0).round() / 100.0,
                "avg_energy": (summary.avg_energy * 100.0).round() / 100.0,
                "avg_metabolism": (summary.avg_metabolism * 100.0).round() / 100.0,
                "avg_repro_threshold": (summary.avg_repro_threshold * 100.0).round() / 100.0,
                "avg_mutation_rate": (summary.avg_mutation_rate * 100.0).round() / 100.0,
                "genome_diversity": (summary.genome_diversity * 10000.0).round() / 10000.0,
                "season_modifier": (summary.season_modifier * 1000.0).round() / 1000.0,
                "num_cells": summary.num_cells,
                "migrations": summary.migrations,
            });

            let biome_json = if is_spatial {
                let biome_pops = population.biome_populations();
                let map: HashMap<String, usize> = biome_pops.into_iter().collect();
                let j = serde_json::to_value(&map).unwrap();
                record["biome_populations"] = j.clone();
                Some(serde_json::to_string(&j).unwrap())
            } else {
                None
            };

            serde_json::to_writer(&mut summary_w, &record).ok();
            summary_w.write_all(b"\n").ok();

            db::insert_tick_summary(
                &conn, run_id,
                summary.tick as i64, summary.population_size as i64,
                summary.births as i64, summary.deaths as i64,
                summary.active_lineages as i64, summary.total_resources,
                summary.avg_energy, summary.avg_metabolism,
                summary.avg_repro_threshold, summary.avg_mutation_rate,
                summary.genome_diversity, summary.season_modifier,
                summary.num_cells as i64, summary.migrations as i64,
                biome_json.as_deref(),
            );

            if is_spatial {
                let biome_str: String = population
                    .biome_populations()
                    .iter()
                    .map(|(n, c)| format!("{n}={c}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                eprintln!(
                    "[tick {tick}] pop={} lin={} res={:.0} b={} d={} m={} | {biome_str}",
                    summary.population_size, summary.active_lineages,
                    summary.total_resources, summary.births,
                    summary.deaths, summary.migrations,
                );
            } else {
                eprintln!(
                    "[tick {tick}] pop={} lin={} res={:.0} b={} d={}",
                    summary.population_size, summary.active_lineages,
                    summary.total_resources, summary.births, summary.deaths,
                );
            }
        }

        if population.is_extinct() {
            eprintln!("[tick {tick}] TOTAL EXTINCTION");
            status = "extinct";
            break;
        }

        if tick % 1000 == 0 {
            phylo_w.flush().ok();
            extinct_w.flush().ok();
            summary_w.flush().ok();
        }
    }

    phylo_w.flush().ok();
    extinct_w.flush().ok();
    summary_w.flush().ok();

    let elapsed = start.elapsed().as_secs_f64();
    let final_pop = population.organism_count();
    eprintln!(
        "[done] {} ticks in {elapsed:.1}s ({:.0} ticks/s)",
        last_tick + 1, (last_tick + 1) as f64 / elapsed,
    );

    db::finalize_run(&conn, run_id, status, last_tick as i64, elapsed, final_pop as i64);
}

// ── Ls command ───────────────────────────────────────────────────────────

fn cmd_ls() {
    let conn = db::open_db(Path::new("logs"));
    let rows = db::list_runs(&conn);

    if rows.is_empty() {
        println!("No runs yet. Use: primordial run -c <config.toml>");
        return;
    }

    println!(
        "{:>4}  {:>10}  {:>6}  {:>4}  {:>8}  {:>6}  {:>8}  {}",
        "ID", "Status", "Seed", "Grid", "Ticks", "Pop", "Time", "Started"
    );
    println!("{}", "-".repeat(76));
    for r in &rows {
        let t = r.final_tick.map_or("-".into(), |t| t.to_string());
        let p = r.final_population.map_or("-".into(), |p| p.to_string());
        let e = r.elapsed_seconds.map_or("-".into(), |e| format!("{e:.1}s"));
        let short_time = if r.started_at.len() > 19 { &r.started_at[..19] } else { &r.started_at };
        println!(
            "{:>4}  {:>10}  {:>6}  {:>4}  {:>8}  {:>6}  {:>8}  {short_time}",
            r.id, r.status, r.seed, r.grid_size, t, p, e
        );
    }
}

// ── Show command ─────────────────────────────────────────────────────────

fn cmd_show(id: Option<i64>) {
    let conn = db::open_db(Path::new("logs"));
    let run_id = id.unwrap_or_else(|| db::latest_run_id(&conn).unwrap_or(0));

    let run = match db::get_run(&conn, run_id) {
        Some(r) => r,
        None => {
            eprintln!("Run #{run_id} not found.");
            return;
        }
    };

    println!("Run #{}", run.id);
    println!("  Status:     {}", run.status);
    println!("  Seed:       {}", run.seed);
    println!("  Grid:       {}x{}", run.grid_size, run.grid_size);
    println!("  Max ticks:  {}", run.max_ticks);
    if let Some(t) = run.final_tick {
        println!("  Final tick: {t}");
    }
    if let Some(p) = run.final_population {
        println!("  Final pop:  {p}");
    }
    if let Some(e) = run.elapsed_seconds {
        let tps = run.final_tick.unwrap_or(0) as f64 / e;
        println!("  Time:       {e:.1}s ({tps:.0} ticks/s)");
    }
    println!("  Started:    {}", run.started_at);
    if let Some(d) = &run.log_dir {
        println!("  Log dir:    {d}");
    }
}

// ── Dashboard command ────────────────────────────────────────────────────

fn cmd_dashboard(id: Option<i64>, output: Option<PathBuf>) {
    let conn = db::open_db(Path::new("logs"));
    let run_id = id.unwrap_or_else(|| db::latest_run_id(&conn).unwrap_or(0));

    let run = match db::get_run(&conn, run_id) {
        Some(r) => r,
        None => {
            eprintln!("Run #{run_id} not found.");
            return;
        }
    };

    let log_dir = match &run.log_dir {
        Some(d) => PathBuf::from(d),
        None => {
            eprintln!("Run #{run_id} has no log dir.");
            return;
        }
    };

    let phylo = log_dir.join("phylogenetic_tree.json");
    let summaries = log_dir.join("tick_summaries.ndjson");
    let out = output.unwrap_or_else(|| log_dir.join("dashboard.png"));

    if !phylo.exists() || !summaries.exists() {
        eprintln!("Log files missing in {}", log_dir.display());
        return;
    }

    eprintln!("Generating dashboard for run #{run_id}...");
    let python = if Path::new(".venv/bin/python").exists() {
        ".venv/bin/python"
    } else {
        "python"
    };
    let status = Command::new(python)
        .args([
            "viz/phylo.py",
            phylo.to_str().unwrap(),
            "-s", summaries.to_str().unwrap(),
            "-o", out.to_str().unwrap(),
        ])
        .status();

    match status {
        Ok(s) if s.success() => println!("{}", out.display()),
        Ok(s) => eprintln!("viz exited with {s}"),
        Err(e) => eprintln!("failed to run viz: {e}"),
    }
}

// ── Main ─────────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Run { config, max_ticks } => cmd_run(&config, max_ticks),
        Cmd::Ls => cmd_ls(),
        Cmd::Show { id } => cmd_show(id),
        Cmd::Dashboard { id, output } => cmd_dashboard(id, output),
    }
}
