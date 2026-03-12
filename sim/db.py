"""SQLite storage for primordial simulation runs.

Shared database with the Rust CLI (logs/primordial.sqlite).
Lineage events stay in NDJSON files — different data shape, different storage.
The runs table records the log directory so viz tools can find the event files.
"""

import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path


class SimulationDB:
    """Append-only store for simulation runs and tick summaries."""

    def __init__(self, path: Path):
        self.path = path
        self.conn = sqlite3.connect(str(path))
        self.conn.row_factory = sqlite3.Row
        self._ensure_schema()
        self._run_id: int | None = None
        self._summary_buf: list[tuple] = []

    def _ensure_schema(self):
        self.conn.executescript("""
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
        """)

    @property
    def run_id(self) -> int:
        if self._run_id is None:
            raise RuntimeError("no active run — call start_run() first")
        return self._run_id

    def start_run(self, seed: int, config: dict, max_ticks: int,
                  grid_size: int, log_dir: str | None = None) -> int:
        now = datetime.now(timezone.utc).isoformat()
        cur = self.conn.execute(
            "INSERT INTO runs (started_at, seed, config, max_ticks, grid_size, log_dir) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            [now, seed, json.dumps(config), max_ticks, grid_size, log_dir],
        )
        self.conn.commit()
        self._run_id = cur.lastrowid
        return self._run_id

    def insert_summary(self, record: dict):
        biome_json = (json.dumps(record["biome_populations"])
                      if "biome_populations" in record else None)
        self._summary_buf.append((
            self.run_id,
            record["tick"],
            record["population"],
            record["births"],
            record["deaths"],
            record["lineages"],
            record["resources"],
            record.get("avg_energy"),
            record.get("avg_metabolism"),
            record.get("avg_repro_threshold"),
            record.get("avg_mutation_rate"),
            record.get("genome_diversity"),
            record.get("season_modifier"),
            record.get("num_cells"),
            record.get("migrations"),
            biome_json,
        ))

    def flush(self):
        if self._summary_buf:
            self.conn.executemany(
                "INSERT INTO tick_summaries VALUES "
                "(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                self._summary_buf,
            )
            self.conn.commit()
            self._summary_buf.clear()

    def finalize_run(self, final_tick: int, elapsed: float, final_pop: int,
                     status: str = "completed"):
        self.flush()
        self.conn.execute(
            "UPDATE runs SET status=?, final_tick=?, elapsed_seconds=?, final_population=? "
            "WHERE id=?",
            [status, final_tick, elapsed, final_pop, self.run_id],
        )
        self.conn.commit()

    def close(self):
        self.flush()
        self.conn.close()

    # --- Query methods ---

    def list_runs(self) -> list[dict]:
        cur = self.conn.execute(
            "SELECT id, started_at, seed, max_ticks, grid_size, status, "
            "final_tick, elapsed_seconds, final_population, log_dir "
            "FROM runs ORDER BY id"
        )
        return [dict(row) for row in cur.fetchall()]

    def get_summaries(self, run_id: int) -> list[dict]:
        cur = self.conn.execute(
            "SELECT * FROM tick_summaries WHERE run_id=? ORDER BY tick", [run_id]
        )
        rows = []
        for row in cur.fetchall():
            d = dict(row)
            if d.get("biome_populations"):
                d["biome_populations"] = json.loads(d["biome_populations"])
            rows.append(d)
        return rows

    def get_run(self, run_id: int) -> dict | None:
        cur = self.conn.execute("SELECT * FROM runs WHERE id=?", [run_id])
        row = cur.fetchone()
        if row:
            d = dict(row)
            if isinstance(d["config"], str):
                d["config"] = json.loads(d["config"])
            return d
        return None

    def latest_run_id(self) -> int | None:
        cur = self.conn.execute("SELECT max(id) FROM runs")
        row = cur.fetchone()
        return row[0] if row and row[0] is not None else None
