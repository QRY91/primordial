use std::path::Path;

use rusqlite::{params, Connection};

pub const SCHEMA: &str = "
    PRAGMA journal_mode=WAL;
    PRAGMA synchronous=NORMAL;
    CREATE TABLE IF NOT EXISTS runs (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        started_at TEXT NOT NULL,
        seed INTEGER NOT NULL,
        config TEXT NOT NULL,
        max_ticks INTEGER NOT NULL,
        grid_size INTEGER NOT NULL DEFAULT 1,
        log_dir TEXT,
        status TEXT NOT NULL DEFAULT 'running',
        final_tick INTEGER,
        elapsed_seconds REAL,
        final_population INTEGER
    );
    CREATE TABLE IF NOT EXISTS tick_summaries (
        run_id INTEGER NOT NULL REFERENCES runs(id),
        tick INTEGER NOT NULL,
        population INTEGER NOT NULL,
        births INTEGER NOT NULL,
        deaths INTEGER NOT NULL,
        lineages INTEGER NOT NULL,
        resources REAL NOT NULL,
        avg_energy REAL,
        avg_metabolism REAL,
        avg_repro_threshold REAL,
        avg_mutation_rate REAL,
        genome_diversity REAL,
        season_modifier REAL,
        num_cells INTEGER,
        migrations INTEGER,
        biome_populations TEXT,
        PRIMARY KEY (run_id, tick)
    );
";

pub fn open_db(base_dir: &Path) -> Connection {
    let db_path = base_dir.join("primordial.sqlite");
    let conn = Connection::open(&db_path).expect("failed to open sqlite");
    conn.execute_batch(SCHEMA).expect("failed to create schema");
    conn
}

#[derive(Clone, Debug)]
pub struct RunRow {
    pub id: i64,
    pub started_at: String,
    pub seed: i64,
    pub max_ticks: i64,
    pub grid_size: i64,
    pub status: String,
    pub final_tick: Option<i64>,
    pub elapsed_seconds: Option<f64>,
    pub final_population: Option<i64>,
    pub log_dir: Option<String>,
}

pub fn list_runs(conn: &Connection) -> Vec<RunRow> {
    let mut stmt = conn
        .prepare(
            "SELECT id, started_at, seed, max_ticks, grid_size, status, \
             final_tick, elapsed_seconds, final_population, log_dir \
             FROM runs ORDER BY id",
        )
        .unwrap();

    stmt.query_map([], |row| {
        Ok(RunRow {
            id: row.get(0)?,
            started_at: row.get(1)?,
            seed: row.get(2)?,
            max_ticks: row.get(3)?,
            grid_size: row.get(4)?,
            status: row.get(5)?,
            final_tick: row.get(6)?,
            elapsed_seconds: row.get(7)?,
            final_population: row.get(8)?,
            log_dir: row.get(9)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn get_run(conn: &Connection, run_id: i64) -> Option<RunRow> {
    conn.query_row(
        "SELECT id, started_at, seed, max_ticks, grid_size, status, \
         final_tick, elapsed_seconds, final_population, log_dir \
         FROM runs WHERE id=?1",
        params![run_id],
        |row| {
            Ok(RunRow {
                id: row.get(0)?,
                started_at: row.get(1)?,
                seed: row.get(2)?,
                max_ticks: row.get(3)?,
                grid_size: row.get(4)?,
                status: row.get(5)?,
                final_tick: row.get(6)?,
                elapsed_seconds: row.get(7)?,
                final_population: row.get(8)?,
                log_dir: row.get(9)?,
            })
        },
    )
    .ok()
}

pub fn latest_run_id(conn: &Connection) -> Option<i64> {
    conn.query_row("SELECT max(id) FROM runs", [], |row| row.get(0))
        .ok()
        .flatten()
}

pub fn insert_run(
    conn: &Connection,
    started_at: &str,
    seed: i64,
    config_json: &str,
    max_ticks: i64,
    grid_size: i64,
    log_dir: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO runs (started_at, seed, config, max_ticks, grid_size, log_dir) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![started_at, seed, config_json, max_ticks, grid_size, log_dir],
    )
    .expect("failed to insert run");
    conn.last_insert_rowid()
}

pub fn finalize_run(
    conn: &Connection,
    run_id: i64,
    status: &str,
    final_tick: i64,
    elapsed: f64,
    final_pop: i64,
) {
    conn.execute(
        "UPDATE runs SET status=?1, final_tick=?2, elapsed_seconds=?3, final_population=?4 \
         WHERE id=?5",
        params![status, final_tick, elapsed, final_pop, run_id],
    )
    .ok();
}

pub fn insert_tick_summary(
    conn: &Connection,
    run_id: i64,
    tick: i64,
    population: i64,
    births: i64,
    deaths: i64,
    lineages: i64,
    resources: f64,
    avg_energy: f64,
    avg_metabolism: f64,
    avg_repro_threshold: f64,
    avg_mutation_rate: f64,
    genome_diversity: f64,
    season_modifier: f64,
    num_cells: i64,
    migrations: i64,
    biome_json: Option<&str>,
) {
    conn.execute(
        "INSERT INTO tick_summaries VALUES \
         (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            run_id, tick, population, births, deaths, lineages, resources,
            avg_energy, avg_metabolism, avg_repro_threshold, avg_mutation_rate,
            genome_diversity, season_modifier, num_cells, migrations, biome_json,
        ],
    )
    .ok();
}

#[derive(Debug)]
pub struct TickRow {
    pub tick: i64,
    pub population: f64,
    pub births: f64,
    pub deaths: f64,
    pub lineages: f64,
    pub resources: f64,
    pub avg_metabolism: f64,
    pub avg_repro_threshold: f64,
    pub avg_mutation_rate: f64,
    pub genome_diversity: f64,
    pub season_modifier: f64,
    pub migrations: f64,
    pub biome_json: Option<String>,
}

pub fn get_tick_summaries(conn: &Connection, run_id: i64) -> Vec<TickRow> {
    let mut stmt = conn
        .prepare(
            "SELECT tick, population, births, deaths, lineages, resources, \
             avg_metabolism, avg_repro_threshold, avg_mutation_rate, \
             genome_diversity, season_modifier, migrations, biome_populations \
             FROM tick_summaries WHERE run_id=?1 ORDER BY tick",
        )
        .unwrap();

    stmt.query_map(params![run_id], |row| {
        Ok(TickRow {
            tick: row.get(0)?,
            population: row.get(1)?,
            births: row.get(2)?,
            deaths: row.get(3)?,
            lineages: row.get(4)?,
            resources: row.get(5)?,
            avg_metabolism: row.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
            avg_repro_threshold: row.get::<_, Option<f64>>(7)?.unwrap_or(0.0),
            avg_mutation_rate: row.get::<_, Option<f64>>(8)?.unwrap_or(0.0),
            genome_diversity: row.get::<_, Option<f64>>(9)?.unwrap_or(0.0),
            season_modifier: row.get::<_, Option<f64>>(10)?.unwrap_or(1.0),
            migrations: row.get::<_, Option<f64>>(11)?.unwrap_or(0.0),
            biome_json: row.get(12)?,
        })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}
