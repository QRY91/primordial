use std::collections::HashMap;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use chrono::Local;
use eframe::egui;
use egui_plot::{Line, Plot};
use rusqlite::Connection;

use primordial_core::lineage::LineageEventType;
use primordial_core::population::Population;

use primordial_common::config::{self, TomlConfig};
use primordial_common::db::{self, RunRow};

// ── Data types for live streaming ───────────────────────────────────────

#[derive(Clone)]
struct CellSnapshot {
    population: usize,
    biome: String,
    temperature: f64,
    moisture: f64,
    resources: f64,
}

#[derive(Clone)]
struct LiveFrame {
    tick: u64,
    population: usize,
    births: usize,
    deaths: usize,
    lineages: usize,
    resources: f64,
    avg_metabolism: f64,
    avg_repro: f64,
    avg_mutrate: f64,
    diversity: f64,
    season: f64,
    migrations: usize,
    biome_pops: HashMap<String, usize>,
    cells: Vec<CellSnapshot>,
    grid_size: usize,
}

enum SimMessage {
    Frame(LiveFrame),
    Done { run_id: i64, ticks: u64, elapsed: f64, tps: f64 },
    Error(String),
}

// ── Run data for display ────────────────────────────────────────────────

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
    // Spatial snapshot (latest frame)
    cells: Vec<CellSnapshot>,
    grid_size: usize,
}

impl RunData {
    fn push_frame(&mut self, f: &LiveFrame) {
        self.ticks.push(f.tick as i64);
        self.population.push(f.population as f64);
        self.lineages.push(f.lineages as f64);
        self.births.push(f.births as f64);
        self.deaths.push(f.deaths as f64);
        self.resources.push(f.resources);
        self.avg_metabolism.push(f.avg_metabolism);
        self.avg_repro.push(f.avg_repro);
        self.avg_mutrate.push(f.avg_mutrate);
        self.diversity.push(f.diversity);
        self.season.push(f.season);
        self.migrations.push(f.migrations as f64);

        for (name, &count) in &f.biome_pops {
            let series = self.biome_series.entry(name.clone()).or_default();
            while series.len() < self.ticks.len() - 1 {
                series.push(0.0);
            }
            series.push(count as f64);
        }
        let n = self.ticks.len();
        for series in self.biome_series.values_mut() {
            while series.len() < n {
                series.push(0.0);
            }
        }

        self.cells = f.cells.clone();
        self.grid_size = f.grid_size;
    }

    fn empty_for_run(run: RunRow) -> Self {
        Self {
            run, ticks: vec![], population: vec![], lineages: vec![],
            births: vec![], deaths: vec![], resources: vec![],
            avg_metabolism: vec![], avg_repro: vec![], avg_mutrate: vec![],
            diversity: vec![], season: vec![], migrations: vec![],
            biome_series: HashMap::new(), cells: vec![], grid_size: 0,
        }
    }
}

// ── App State ───────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
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

    // Run comparison
    compare_run: Option<i64>,
    compare_data: Option<RunData>,

    // New run dialog
    show_new_run: bool,
    show_config_editor: bool,
    config_text: String,
    new_run_config_path: String,
    new_run_max_ticks: String,

    // Background simulation
    sim_running: bool,
    sim_progress: String,
    sim_rx: Option<mpsc::Receiver<SimMessage>>,
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
            compare_run: None,
            compare_data: None,
            show_new_run: false,
            show_config_editor: false,
            config_text: String::new(),
            new_run_config_path: "sim/experiments/phase1.toml".into(),
            new_run_max_ticks: "5000".into(),
            sim_running: false,
            sim_progress: String::new(),
            sim_rx: None,
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

    fn load_compare_data(&mut self, id: i64) {
        self.compare_run = Some(id);
        if let Some(run) = self.runs.iter().find(|r| r.id == id) {
            self.compare_data = Some(load_run_data(&self.db_path, run.clone()));
        }
    }

    fn start_simulation(&mut self, ctx: egui::Context, config_override: Option<String>) {
        let config_path = PathBuf::from(&self.new_run_config_path);
        let max_ticks: u64 = self.new_run_max_ticks.parse().unwrap_or(5000);
        let (tx, rx) = mpsc::channel();
        self.sim_rx = Some(rx);
        self.sim_running = true;
        self.sim_progress = "Starting...".into();

        // Create a placeholder RunRow for live display
        let live_run = RunRow {
            id: 0, started_at: String::new(), seed: 0, max_ticks: max_ticks as i64,
            grid_size: 1, status: "running".into(), final_tick: None,
            elapsed_seconds: None, final_population: None, log_dir: None,
        };
        self.run_data = Some(RunData::empty_for_run(live_run));
        self.selected_run = None;

        thread::spawn(move || {
            run_simulation(&config_path, max_ticks, config_override.as_deref(), &tx);
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain live simulation frames
        let mut got_frames = false;
        if let Some(rx) = &self.sim_rx {
            loop {
                match rx.try_recv() {
                    Ok(SimMessage::Frame(frame)) => {
                        self.sim_progress = format!(
                            "tick {}/{} pop={}",
                            frame.tick, self.new_run_max_ticks.parse::<u64>().unwrap_or(0),
                            frame.population,
                        );
                        if let Some(data) = &mut self.run_data {
                            data.push_frame(&frame);
                        }
                        got_frames = true;
                    }
                    Ok(SimMessage::Done { run_id, ticks, elapsed, tps }) => {
                        self.sim_progress = format!(
                            "Done: run #{run_id}, {ticks} ticks in {elapsed:.1}s ({tps:.0} t/s)"
                        );
                        self.sim_running = false;
                        self.sim_rx = None;
                        self.refresh_runs();
                        self.load_run_data(run_id);
                        break;
                    }
                    Ok(SimMessage::Error(e)) => {
                        self.sim_progress = format!("Error: {e}");
                        self.sim_running = false;
                        self.sim_rx = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.sim_running = false;
                        self.sim_rx = None;
                        break;
                    }
                }
            }
        }
        let _ = got_frames;

        // Top bar
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Primordial");
                ui.separator();
                if ui.button("New Run").clicked() {
                    self.show_new_run = true;
                }
                if ui.button("Config Editor").clicked() {
                    // Load current config into editor
                    let path = &self.new_run_config_path;
                    self.config_text = fs::read_to_string(path).unwrap_or_default();
                    self.show_config_editor = true;
                }
                if ui.button("Refresh").clicked() {
                    self.refresh_runs();
                    if let Some(id) = self.selected_run {
                        self.load_run_data(id);
                    }
                }
                if self.sim_running {
                    ui.separator();
                    ui.spinner();
                    ui.label(&self.sim_progress);
                } else if !self.sim_progress.is_empty() {
                    ui.separator();
                    ui.label(&self.sim_progress);
                }
            });
        });

        // Left panel: run list + compare selector
        let mut clicked_run_id: Option<i64> = None;
        let mut compare_clicked: Option<i64> = None;
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
                        ui.horizontal(|ui| {
                            if ui.selectable_label(selected, &label).clicked() {
                                clicked_run_id = Some(run.id);
                            }
                            // Compare button
                            if self.selected_run.is_some() && self.selected_run != Some(run.id) {
                                let is_compare = self.compare_run == Some(run.id);
                                let cmp_label = if is_compare { "x" } else { "cmp" };
                                if ui.small_button(cmp_label).clicked() {
                                    if is_compare {
                                        compare_clicked = Some(-1); // clear
                                    } else {
                                        compare_clicked = Some(run.id);
                                    }
                                }
                            }
                        });
                        if let Some(t) = run.final_tick {
                            let pop = run.final_population.unwrap_or(0);
                            let time = run.elapsed_seconds
                                .map(|e| format!("{e:.1}s")).unwrap_or_default();
                            ui.indent(run.id, |ui| {
                                ui.small(format!("{t} ticks | pop {pop} | {time}"));
                            });
                        }
                    }
                });
            });

        // Handle deferred actions
        if let Some(id) = clicked_run_id {
            self.load_run_data(id);
            self.compare_run = None;
            self.compare_data = None;
        }
        if let Some(id) = compare_clicked {
            if id < 0 {
                self.compare_run = None;
                self.compare_data = None;
            } else {
                self.load_compare_data(id);
            }
        }

        // Central panel: charts + spatial grid
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(data) = &self.run_data {
                render_run_view(ui, data, self.compare_data.as_ref());
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
                    ui.add_enabled_ui(!self.sim_running, |ui| {
                        if ui.button("Start").clicked() {
                            self.start_simulation(ctx.clone(), None);
                            self.show_new_run = false;
                        }
                    });
                });
            if !open {
                self.show_new_run = false;
            }
        }

        // Config editor window
        if self.show_config_editor {
            let mut open = true;
            egui::Window::new("Config Editor")
                .open(&mut open)
                .default_width(500.0)
                .default_height(600.0)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label(format!("Editing: {}", self.new_run_config_path));
                    ui.separator();

                    // Validation status
                    let valid = config::parse_config(&self.config_text).is_ok();
                    if valid {
                        ui.colored_label(egui::Color32::from_rgb(76, 175, 80), "Valid TOML config");
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(244, 67, 54), "Invalid TOML");
                    }

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.config_text)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(30),
                        );
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save to file").clicked() {
                            fs::write(&self.new_run_config_path, &self.config_text).ok();
                        }
                        ui.add_enabled_ui(valid && !self.sim_running, |ui| {
                            if ui.button("Run with this config").clicked() {
                                // Save to temp, then run
                                let tmp = PathBuf::from("logs/_gui_config.toml");
                                fs::write(&tmp, &self.config_text).ok();
                                self.new_run_config_path = tmp.to_string_lossy().into();
                                self.start_simulation(ctx.clone(), Some(self.config_text.clone()));
                                self.show_config_editor = false;
                            }
                        });
                        if ui.button("Reload from file").clicked() {
                            self.config_text = fs::read_to_string(&self.new_run_config_path)
                                .unwrap_or_default();
                        }
                    });
                });
            if !open {
                self.show_config_editor = false;
            }
        }

        // Keep refreshing while sim is running
        if self.sim_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

// ── Simulation runner (background thread) ───────────────────────────────

fn run_simulation(
    config_path: &Path,
    max_ticks: u64,
    config_override: Option<&str>,
    tx: &mpsc::Sender<SimMessage>,
) {
    let raw = config_override
        .map(|s| s.to_string())
        .or_else(|| fs::read_to_string(config_path).ok());
    let raw = match raw {
        Some(s) => s,
        None => {
            tx.send(SimMessage::Error(format!("Cannot read {}", config_path.display()))).ok();
            return;
        }
    };
    let toml_cfg: TomlConfig = match config::parse_config(&raw) {
        Ok(c) => c,
        Err(e) => {
            tx.send(SimMessage::Error(e)).ok();
            return;
        }
    };

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
            // Collect biome populations
            let biome_pops: HashMap<String, usize> = if is_spatial {
                population.biome_populations().into_iter().collect()
            } else {
                HashMap::new()
            };
            let biome_json = if is_spatial {
                Some(serde_json::to_string(&biome_pops).unwrap())
            } else {
                None
            };

            // Collect per-cell spatial data
            let cell_pops = population.cell_populations();
            let cells: Vec<CellSnapshot> = (0..population.world.num_cells())
                .map(|i| CellSnapshot {
                    population: cell_pops[i],
                    biome: population.world.cell_biome(i).name().to_string(),
                    temperature: population.world.cell(i).temperature,
                    moisture: population.world.cell(i).moisture,
                    resources: population.cell_resources[i],
                })
                .collect();

            // Send live frame to GUI
            let frame = LiveFrame {
                tick: summary.tick,
                population: summary.population_size,
                births: summary.births,
                deaths: summary.deaths,
                lineages: summary.active_lineages,
                resources: summary.total_resources,
                avg_metabolism: summary.avg_metabolism,
                avg_repro: summary.avg_repro_threshold,
                avg_mutrate: summary.avg_mutation_rate,
                diversity: summary.genome_diversity,
                season: summary.season_modifier,
                migrations: summary.migrations,
                biome_pops: biome_pops.clone(),
                cells,
                grid_size,
            };
            if tx.send(SimMessage::Frame(frame)).is_err() {
                break; // GUI closed
            }

            // Write to NDJSON + DB
            let mut record = serde_json::json!({
                "tick": summary.tick,
                "population": summary.population_size,
                "births": summary.births, "deaths": summary.deaths,
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
            if let Some(ref bj) = biome_json {
                record["biome_populations"] = serde_json::from_str(bj).unwrap();
            }
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
    tx.send(SimMessage::Done { run_id, ticks: last_tick + 1, elapsed, tps }).ok();
}

// ── Render run data with optional comparison ────────────────────────────

fn render_run_view(ui: &mut egui::Ui, data: &RunData, compare: Option<&RunData>) {
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

        if let Some(cmp) = compare {
            ui.separator();
            ui.label(format!("vs Run #{}", cmp.run.id));
        }
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        let avail = ui.available_width();

        // If we have spatial data, show grid map at the top
        if data.grid_size > 1 && !data.cells.is_empty() {
            render_grid_map(ui, data, avail);
            ui.add_space(8.0);
        }

        let chart_w = (avail / 2.0 - 12.0).max(300.0);
        let chart_h = 180.0;

        // Row 1: Population + Lineages | Birth/Death rates
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Population & Lineages").strong());
                Plot::new("pop_lineages")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.population))
                            .name("Population").color(egui::Color32::from_rgb(33, 150, 243)));
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.lineages))
                            .name("Lineages").color(egui::Color32::from_rgb(255, 152, 0)));
                        if let Some(cmp) = compare {
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.population))
                                .name("Pop (cmp)").color(egui::Color32::from_rgb(33, 150, 243).gamma_multiply(0.4)));
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.lineages))
                                .name("Lin (cmp)").color(egui::Color32::from_rgb(255, 152, 0).gamma_multiply(0.4)));
                        }
                    });
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Birth / Death Rates").strong());
                Plot::new("birth_death")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.births))
                            .name("Births").color(egui::Color32::from_rgb(76, 175, 80)));
                        let d: Vec<[f64; 2]> = data.ticks.iter().zip(&data.deaths)
                            .map(|(&t, &d)| [t as f64, -d]).collect();
                        plot_ui.line(Line::new(d)
                            .name("Deaths").color(egui::Color32::from_rgb(244, 67, 54)));
                        if let Some(cmp) = compare {
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.births))
                                .name("Births (cmp)").color(egui::Color32::from_rgb(76, 175, 80).gamma_multiply(0.4)));
                            let d2: Vec<[f64; 2]> = cmp.ticks.iter().zip(&cmp.deaths)
                                .map(|(&t, &d)| [t as f64, -d]).collect();
                            plot_ui.line(Line::new(d2)
                                .name("Deaths (cmp)").color(egui::Color32::from_rgb(244, 67, 54).gamma_multiply(0.4)));
                        }
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
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.avg_metabolism))
                            .name("Metabolism").color(egui::Color32::from_rgb(233, 30, 99)));
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.avg_repro))
                            .name("Repro threshold").color(egui::Color32::from_rgb(63, 81, 181)));
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.avg_mutrate))
                            .name("Mutation rate").color(egui::Color32::from_rgb(0, 150, 136)));
                        if let Some(cmp) = compare {
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.avg_metabolism))
                                .name("Metab (cmp)").color(egui::Color32::from_rgb(233, 30, 99).gamma_multiply(0.4)));
                        }
                    });
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Genome Diversity & Seasons").strong());
                Plot::new("diversity")
                    .width(chart_w).height(chart_h)
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.diversity))
                            .name("Diversity").color(egui::Color32::from_rgb(156, 39, 176)));
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.season))
                            .name("Season").color(egui::Color32::from_rgb(255, 152, 0)));
                        if let Some(cmp) = compare {
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.diversity))
                                .name("Div (cmp)").color(egui::Color32::from_rgb(156, 39, 176).gamma_multiply(0.4)));
                        }
                    });
            });
        });

        ui.add_space(8.0);

        // Row 3: Biomes | Migrations
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if !data.biome_series.is_empty() {
                    ui.label(egui::RichText::new("Biome Populations").strong());
                    Plot::new("biomes")
                        .width(chart_w).height(chart_h)
                        .legend(egui_plot::Legend::default())
                        .show(ui, |plot_ui| {
                            let bc = biome_colors();
                            let mut names: Vec<&String> = data.biome_series.keys().collect();
                            names.sort();
                            for name in names {
                                let color = bc.get(name.as_str()).copied()
                                    .unwrap_or(egui::Color32::GRAY);
                                plot_ui.line(Line::new(to_pts(&data.ticks, &data.biome_series[name]))
                                    .name(name).color(color));
                            }
                        });
                } else {
                    ui.label(egui::RichText::new("Resources").strong());
                    Plot::new("resources")
                        .width(chart_w).height(chart_h)
                        .show(ui, |plot_ui| {
                            plot_ui.line(Line::new(to_pts(&data.ticks, &data.resources))
                                .name("Resources").color(egui::Color32::from_rgb(255, 193, 7)));
                        });
                }
            });
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Migrations").strong());
                Plot::new("migrations")
                    .width(chart_w).height(chart_h)
                    .show(ui, |plot_ui| {
                        plot_ui.line(Line::new(to_pts(&data.ticks, &data.migrations))
                            .name("Migrations").color(egui::Color32::from_rgb(121, 85, 72)));
                        if let Some(cmp) = compare {
                            plot_ui.line(Line::new(to_pts(&cmp.ticks, &cmp.migrations))
                                .name("Migr (cmp)").color(egui::Color32::from_rgb(121, 85, 72).gamma_multiply(0.4)));
                        }
                    });
            });
        });
    });
}

// ── Spatial grid map ────────────────────────────────────────────────────

fn render_grid_map(ui: &mut egui::Ui, data: &RunData, avail_width: f32) {
    let gs = data.grid_size;
    if gs == 0 { return; }

    ui.label(egui::RichText::new("Spatial Grid Map").strong());

    let max_pop = data.cells.iter().map(|c| c.population).max().unwrap_or(1).max(1);
    let cell_size = ((avail_width - 20.0) / gs as f32).min(80.0).max(20.0);
    let grid_w = cell_size * gs as f32;
    let grid_h = cell_size * gs as f32;

    let (response, painter) = ui.allocate_painter(
        egui::vec2(grid_w + 150.0, grid_h + 20.0),
        egui::Sense::hover(),
    );
    let origin = response.rect.left_top() + egui::vec2(5.0, 5.0);

    let bc = biome_color_map();

    for (idx, cell) in data.cells.iter().enumerate() {
        let x = (idx % gs) as f32;
        let y = (idx / gs) as f32;
        let rect = egui::Rect::from_min_size(
            origin + egui::vec2(x * cell_size, y * cell_size),
            egui::vec2(cell_size - 1.0, cell_size - 1.0),
        );

        // Base biome color
        let base = biome_color_lookup(&cell.biome);

        // Darken/lighten by population density
        let density = cell.population as f32 / max_pop as f32;
        let r = (base.r() as f32 * (0.3 + 0.7 * density)) as u8;
        let g = (base.g() as f32 * (0.3 + 0.7 * density)) as u8;
        let b = (base.b() as f32 * (0.3 + 0.7 * density)) as u8;
        let color = egui::Color32::from_rgb(r, g, b);

        painter.rect_filled(rect, 2.0, color);

        // Population count text
        if cell_size >= 35.0 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", cell.population),
                egui::FontId::proportional(cell_size * 0.25),
                egui::Color32::WHITE,
            );
        }

        // Tooltip on hover
        let mouse = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
        if rect.contains(mouse) {
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                response.layer_id,
                ui.auto_id_with(idx),
                |ui| {
                    ui.label(format!("Cell ({}, {})", idx % gs, idx / gs));
                    ui.label(format!("Biome: {}", cell.biome));
                    ui.label(format!("Population: {}", cell.population));
                    ui.label(format!("Temp: {:.1}C", cell.temperature));
                    ui.label(format!("Moisture: {:.2}", cell.moisture));
                    ui.label(format!("Resources: {:.0}", cell.resources));
                },
            );
        }
    }

    // Legend
    let legend_x = origin.x + grid_w + 10.0;
    let mut legend_y = origin.y;
    for (name, color) in &bc {
        let rect = egui::Rect::from_min_size(
            egui::pos2(legend_x, legend_y),
            egui::vec2(12.0, 12.0),
        );
        painter.rect_filled(rect, 1.0, *color);
        painter.text(
            egui::pos2(legend_x + 16.0, legend_y + 6.0),
            egui::Align2::LEFT_CENTER,
            *name,
            egui::FontId::proportional(11.0),
            ui.visuals().text_color(),
        );
        legend_y += 18.0;
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn to_pts(ticks: &[i64], values: &[f64]) -> Vec<[f64; 2]> {
    ticks.iter().zip(values).map(|(&t, &v)| [t as f64, v]).collect()
}

fn biome_colors() -> HashMap<&'static str, egui::Color32> {
    [
        ("tropical", egui::Color32::from_rgb(255, 111, 0)),
        ("desert", egui::Color32::from_rgb(255, 213, 79)),
        ("temperate_forest", egui::Color32::from_rgb(76, 175, 80)),
        ("grassland", egui::Color32::from_rgb(139, 195, 74)),
        ("tundra", egui::Color32::from_rgb(144, 202, 249)),
        ("ice", egui::Color32::from_rgb(200, 220, 240)),
    ].into()
}

fn biome_color_map() -> Vec<(&'static str, egui::Color32)> {
    vec![
        ("tropical", egui::Color32::from_rgb(255, 111, 0)),
        ("desert", egui::Color32::from_rgb(255, 213, 79)),
        ("temperate_forest", egui::Color32::from_rgb(76, 175, 80)),
        ("grassland", egui::Color32::from_rgb(139, 195, 74)),
        ("tundra", egui::Color32::from_rgb(144, 202, 249)),
        ("ice", egui::Color32::from_rgb(200, 220, 240)),
    ]
}

fn biome_color_lookup(name: &str) -> egui::Color32 {
    match name {
        "tropical" => egui::Color32::from_rgb(255, 111, 0),
        "desert" => egui::Color32::from_rgb(255, 213, 79),
        "temperate_forest" => egui::Color32::from_rgb(76, 175, 80),
        "grassland" => egui::Color32::from_rgb(139, 195, 74),
        "tundra" => egui::Color32::from_rgb(144, 202, 249),
        "ice" => egui::Color32::from_rgb(200, 220, 240),
        _ => egui::Color32::from_rgb(128, 128, 128),
    }
}

// ── Database loading ────────────────────────────────────────────────────

fn load_runs(db_path: &Path) -> Vec<RunRow> {
    if !db_path.exists() { return vec![]; }
    let conn = Connection::open(db_path).unwrap();
    db::list_runs(&conn)
}

fn load_run_data(db_path: &Path, run: RunRow) -> RunData {
    let conn = Connection::open(db_path).unwrap();
    let rows = db::get_tick_summaries(&conn, run.id);
    let mut data = RunData::empty_for_run(run);

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
                    while series.len() < data.ticks.len() - 1 { series.push(0.0); }
                    series.push(*count);
                }
            }
        }
        let n = data.ticks.len();
        for series in data.biome_series.values_mut() {
            while series.len() < n { series.push(0.0); }
        }
    }

    data
}
