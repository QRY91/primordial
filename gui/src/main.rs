use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use chrono::Local;
use eframe::egui;
use egui_plot::{Line, Plot};
use rusqlite::Connection;

use primordial_core::lineage::LineageEventType;
use primordial_core::population::Population;

use primordial_common::config;
use primordial_common::db::{self, RunRow};

// ── App State ───────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("Primordial"),
        ..Default::default()
    };
    eframe::run_native(
        "Primordial",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    db_path: PathBuf,
    runs: Vec<RunRow>,
    selected_run: Option<i64>,
    run_data: Option<RunData>,

    // New run dialog
    show_new_run: bool,
    new_run_config_path: String,
    new_run_max_ticks: String,

    // Background simulation
    sim_state: Arc<Mutex<SimState>>,
}

struct RunData {
    run: RunRow,
    ticks: Vec<i64>,
    population: Vec<f64>,
    lineages: Vec<f64>,
    births: Vec<f64>,
    deaths: Vec<f64>,
    resources: Vec<f64>,
    avg_metabolism: Vec<f64>,
    avg_repro: Vec<f64>,
    avg_mutrate: Vec<f64>,
    diversity: Vec<f64>,
    season: Vec<f64>,
    migrations: Vec<f64>,
    biome_series: HashMap<String, Vec<f64>>,
}

#[derive(Default)]
struct SimState {
    running: bool,
    progress: String,
    finished: bool,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let db_path = PathBuf::from("logs/primordial.sqlite");
        let runs = load_runs(&db_path);
        Self {
            db_path,
            runs,
            selected_run: None,
            run_data: None,
            show_new_run: false,
            new_run_config_path: "sim/experiments/phase1.toml".into(),
            new_run_max_ticks: "5000".into(),
            sim_state: Arc::new(Mutex::new(SimState::default())),
        }
    }

    fn refresh_runs(&mut self) {
        self.runs = load_runs(&self.db_path);
    }

    fn load_run_data(&mut self, id: i64) {
        self.selected_run = Some(id);
        if let Some(run) = self.runs.iter().find(|r| r.id == id) {
            self.run_data = Some(load_run_data(&self.db_path, run.clone()));
        }
    }

    fn start_simulation(&mut self, ctx: egui::Context) {
        let config_path = PathBuf::from(&self.new_run_config_path);
        let max_ticks: u64 = self.new_run_max_ticks.parse().unwrap_or(5000);
        let state = self.sim_state.clone();

        {
            let mut s = state.lock().unwrap();
            s.running = true;
            s.progress = "Starting...".into();
            s.finished = false;
        }

        thread::spawn(move || {
            run_simulation(&config_path, max_ticks, &state);
            let mut s = state.lock().unwrap();
            s.finished = true;
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if background sim finished
        let sim_finished = {
            let mut state = self.sim_state.lock().unwrap();
            if state.finished {
                state.finished = false;
                state.running = false;
                true
            } else {
                false
            }
        };
        if sim_finished {
            self.refresh_runs();
            if let Some(last) = self.runs.last() {
                let id = last.id;
                self.load_run_data(id);
            }
        }

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Primordial");
                ui.separator();
                if ui.button("New Run").clicked() {
                    self.show_new_run = true;
                }
                if ui.button("Refresh").clicked() {
                    self.refresh_runs();
                    if let Some(id) = self.selected_run {
                        self.load_run_data(id);
                    }
                }
                let state = self.sim_state.lock().unwrap();
                if state.running {
                    ui.separator();
                    ui.spinner();
                    ui.label(&state.progress);
                }
            });
        });

        // Left panel: run list
        let mut clicked_run_id: Option<i64> = None;
        egui::SidePanel::left("runs_panel")
            .min_width(220.0)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading("Runs");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for run in &self.runs {
                        let label = format!(
                            "#{} s{} {}x{} {}",
                            run.id, run.seed, run.grid_size, run.grid_size, run.status,
                        );
                        let selected = self.selected_run == Some(run.id);
                        if ui.selectable_label(selected, &label).clicked() {
                            clicked_run_id = Some(run.id);
                        }
                        if let Some(t) = run.final_tick {
                            let pop = run.final_population.unwrap_or(0);
                            let time = run
                                .elapsed_seconds
                                .map(|e| format!("{e:.1}s"))
                                .unwrap_or_default();
                            ui.indent(run.id, |ui| {
                                ui.small(format!("{t} ticks | pop {pop} | {time}"));
                            });
                        }
                    }
                });
            });

        // Handle deferred run selection
        if let Some(id) = clicked_run_id {
            self.load_run_data(id);
        }

        // Central panel: charts
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(data) = &self.run_data {
                render_run_view(ui, data);
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a run from the list, or start a new one.");
                });
            }
        });

        // New run dialog
        if self.show_new_run {
            let mut open = true;
            egui::Window::new("New Run")
                .open(&mut open)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Config:");
                        ui.text_edit_singleline(&mut self.new_run_config_path);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Max ticks:");
                        ui.text_edit_singleline(&mut self.new_run_max_ticks);
                    });
                    ui.separator();
                    let running = self.sim_state.lock().unwrap().running;
                    ui.add_enabled_ui(!running, |ui| {
                        if ui.button("Start").clicked() {
                            self.start_simulation(ctx.clone());
                            self.show_new_run = false;
                        }
                    });
                });
            if !open {
                self.show_new_run = false;
            }
        }

        // Keep refreshing while sim is running
        if self.sim_state.lock().unwrap().running {
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }
    }
}

// ── Simulation runner (background thread) ───────────────────────────────

fn run_simulation(
    config_path: &Path,
    max_ticks: u64,
    state: &Arc<Mutex<SimState>>,
) {
    let toml_cfg = match config::load_config(config_path) {
        Ok(c) => c,
        Err(e) => {
            state.lock().unwrap().progress = format!("Error: {e}");
            return;
        }
    };
    let raw = fs::read_to_string(config_path).unwrap_or_default();

    let seed = toml_cfg.simulation.seed;
    let log_interval = toml_cfg.simulation.log_interval;
    let pop_config = toml_cfg.to_population_config();
    let grid_size = pop_config.grid_size;
    let is_spatial = grid_size > 1;

    let base_dir = PathBuf::from(&toml_cfg.logging.dir);
    fs::create_dir_all(&base_dir).ok();
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    let run_dir = base_dir.join(format!("{stamp}_seed{seed}"));
    fs::create_dir_all(&run_dir).ok();

    let conn = db::open_db(&base_dir);
    let config_json = serde_json::to_string(&raw).unwrap_or_default();
    let now = Local::now().to_rfc3339();
    let run_id = db::insert_run(
        &conn, &now, seed as i64, &config_json,
        max_ticks as i64, grid_size as i64,
        &run_dir.to_string_lossy(),
    );

    let phylo_file = fs::File::create(run_dir.join("phylogenetic_tree.json")).unwrap();
    let extinction_file = fs::File::create(run_dir.join("extinction_events.log")).unwrap();
    let summary_file = fs::File::create(run_dir.join("tick_summaries.ndjson")).unwrap();
    let mut phylo_w = BufWriter::new(phylo_file);
    let mut extinct_w = BufWriter::new(extinction_file);
    let mut summary_w = BufWriter::new(summary_file);

    let mut population = Population::new(pop_config, seed);
    let start = Instant::now();
    let mut last_tick = 0u64;
    let mut status = "completed";

    for tick in 0..max_ticks {
        last_tick = tick;
        let summary = population.tick(tick).expect("tick failed");

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
                writeln!(extinct_w, "tick={} lineage={} genome={:#018x}",
                    event.tick, event.lineage_id, event.genome_snapshot).ok();
            }
        }

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

            let mut s = state.lock().unwrap();
            let pct = (tick as f64 / max_ticks as f64 * 100.0) as u32;
            s.progress = format!(
                "Run #{run_id}: tick {tick}/{max_ticks} ({pct}%) pop={}",
                summary.population_size
            );
        }

        if population.is_extinct() {
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
    db::finalize_run(&conn, run_id, status, last_tick as i64, elapsed, final_pop as i64);

    let tps = (last_tick + 1) as f64 / elapsed;
    state.lock().unwrap().progress = format!(
        "Run #{run_id} done: {} ticks in {elapsed:.1}s ({tps:.0} t/s)",
        last_tick + 1
    );
}

// ── Render run data ─────────────────────────────────────────────────────

fn render_run_view(ui: &mut egui::Ui, data: &RunData) {
    let run = &data.run;

    // Header
    ui.horizontal(|ui| {
        ui.heading(format!("Run #{}", run.id));
        ui.separator();
        ui.label(format!("seed={} grid={}x{}", run.seed, run.grid_size, run.grid_size));
        if let Some(t) = run.final_tick {
            ui.separator();
            ui.label(format!("{t} ticks"));
        }
        if let Some(e) = run.elapsed_seconds {
            let tps = run.final_tick.unwrap_or(0) as f64 / e;
            ui.label(format!("({e:.1}s, {tps:.0} t/s)"));
        }
        ui.separator();
        let color = match run.status.as_str() {
            "completed" => egui::Color32::from_rgb(76, 175, 80),
            "extinct" => egui::Color32::from_rgb(244, 67, 54),
            "running" => egui::Color32::from_rgb(33, 150, 243),
            _ => egui::Color32::GRAY,
        };
        ui.colored_label(color, &run.status);
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let avail = ui.available_width();
        let chart_w = (avail / 2.0 - 12.0).max(300.0);
        let chart_h = 200.0;

        // Row 1: Population + Lineages | Birth/Death rates
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Population & Lineages").strong());
                Plot::new("pop_lineages")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.population))
                            .name("Population").color(egui::Color32::from_rgb(33, 150, 243)));
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.lineages))
                            .name("Lineages").color(egui::Color32::from_rgb(255, 152, 0)));
                    });
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Birth / Death Rates").strong());
                Plot::new("birth_death")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.births))
                            .name("Births").color(egui::Color32::from_rgb(76, 175, 80)));
                        let d: Vec<[f64; 2]> = data.ticks.iter().zip(&data.deaths)
                            .map(|(&t, &d)| [t as f64, -d]).collect();
                        plot_ui.line(Line::new(d)
                            .name("Deaths").color(egui::Color32::from_rgb(244, 67, 54)));
                    });
            });
        });

        ui.add_space(8.0);

        // Row 2: Trait evolution | Genome diversity
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Trait Evolution").strong());
                Plot::new("traits")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.avg_metabolism))
                            .name("Metabolism").color(egui::Color32::from_rgb(233, 30, 99)));
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.avg_repro))
                            .name("Repro threshold").color(egui::Color32::from_rgb(63, 81, 181)));
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.avg_mutrate))
                            .name("Mutation rate").color(egui::Color32::from_rgb(0, 150, 136)));
                    });
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Genome Diversity & Seasons").strong());
                Plot::new("diversity")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.diversity))
                            .name("Diversity").color(egui::Color32::from_rgb(156, 39, 176)));
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.season))
                            .name("Season modifier").color(egui::Color32::from_rgb(255, 152, 0)));
                    });
            });
        });

        ui.add_space(8.0);

        // Row 3: Biome populations (or resources) | Migrations
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if !data.biome_series.is_empty() {
                    ui.label(egui::RichText::new("Biome Populations").strong());
                    Plot::new("biomes")
                        .width(chart_w).height(chart_h)
                        .legend(egui_plot::Legend::default())
                        .show(ui, |plot_ui| {
                            let biome_colors: HashMap<&str, egui::Color32> = [
                                ("tropical", egui::Color32::from_rgb(255, 111, 0)),
                                ("desert", egui::Color32::from_rgb(255, 213, 79)),
                                ("temperate_forest", egui::Color32::from_rgb(76, 175, 80)),
                                ("grassland", egui::Color32::from_rgb(139, 195, 74)),
                                ("tundra", egui::Color32::from_rgb(144, 202, 249)),
                                ("ice", egui::Color32::from_rgb(224, 224, 224)),
                            ].into();
                            let mut names: Vec<&String> = data.biome_series.keys().collect();
                            names.sort();
                            for name in names {
                                let color = biome_colors.get(name.as_str()).copied()
                                    .unwrap_or(egui::Color32::GRAY);
                                plot_ui.line(Line::new(to_points(&data.ticks, &data.biome_series[name]))
                                    .name(name).color(color));
                            }
                        });
                } else {
                    ui.label(egui::RichText::new("Resources").strong());
                    Plot::new("resources")
                        .width(chart_w).height(chart_h)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(to_points(&data.ticks, &data.resources))
                                .name("Resources").color(egui::Color32::from_rgb(255, 193, 7)));
                        });
                }
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Migrations").strong());
                Plot::new("migrations")
                    .width(chart_w).height(chart_h)
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_points(&data.ticks, &data.migrations))
                            .name("Migrations").color(egui::Color32::from_rgb(121, 85, 72)));
                    });
            });
        });
    });
}

fn to_points(ticks: &[i64], values: &[f64]) -> Vec<[f64; 2]> {
    ticks.iter().zip(values).map(|(&t, &v)| [t as f64, v]).collect()
}

// ── Data loading ────────────────────────────────────────────────────────

fn load_runs(db_path: &Path) -> Vec<RunRow> {
    if !db_path.exists() {
        return vec![];
    }
    let conn = Connection::open(db_path).unwrap();
    db::list_runs(&conn)
}

fn load_run_data(db_path: &Path, run: RunRow) -> RunData {
    let conn = Connection::open(db_path).unwrap();
    let rows = db::get_tick_summaries(&conn, run.id);

    let mut data = RunData {
        run,
        ticks: vec![], population: vec![], lineages: vec![],
        births: vec![], deaths: vec![], resources: vec![],
        avg_metabolism: vec![], avg_repro: vec![], avg_mutrate: vec![],
        diversity: vec![], season: vec![], migrations: vec![],
        biome_series: HashMap::new(),
    };

    for row in &rows {
        data.ticks.push(row.tick);
        data.population.push(row.population);
        data.births.push(row.births);
        data.deaths.push(row.deaths);
        data.lineages.push(row.lineages);
        data.resources.push(row.resources);
        data.avg_metabolism.push(row.avg_metabolism);
        data.avg_repro.push(row.avg_repro_threshold);
        data.avg_mutrate.push(row.avg_mutation_rate);
        data.diversity.push(row.genome_diversity);
        data.season.push(row.season_modifier);
        data.migrations.push(row.migrations);

        if let Some(json_str) = &row.biome_json {
            if let Ok(map) = serde_json::from_str::<HashMap<String, f64>>(json_str) {
                for (name, count) in &map {
                    let series = data.biome_series.entry(name.clone()).or_default();
                    while series.len() < data.ticks.len() - 1 {
                        series.push(0.0);
                    }
                    series.push(*count);
                }
            }
        }
        let n = data.ticks.len();
        for series in data.biome_series.values_mut() {
            while series.len() < n {
                series.push(0.0);
            }
        }
    }

    data
}
