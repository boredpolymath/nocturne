//! Local SQLite Persistence Layer (WAL Mode).
//!
//! Stores state configuration, curfew profiles, and incident audit logs strictly within
//! standard local application data directories with zero network egress:
//! - macOS: `~/Library/Application Support/local.nocturne.mk4.daemon`
//! - Windows: `%LOCALAPPDATA%\local.nocturne.mk4.daemon`
//! - Linux: `$XDG_DATA_HOME/nocturne` (or `~/.local/share/nocturne`)

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Application identifier used for macOS and Windows local data directories.
pub const APP_IDENTIFIER: &str = "local.nocturne.mk4.daemon";
/// Linux XDG data subdirectory name.
pub const XDG_SUBDIR: &str = "nocturne";
/// Database filename within the application data directory.
pub const DB_FILENAME: &str = "nocturne_telemetry.db";
/// Rolling retention cap for the `incident_audit_log` table to prevent unbounded disk growth.
pub const MAX_AUDIT_LOG_ROWS: i64 = 10_000;

/// Strongly typed representation of user profile settings stored in `profile_settings`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub vigil_start: String,
    pub vigil_end: String,
    pub curfew_engaged: bool,
    pub intensity_detent: u8,
    pub cognitive_lull_window_sec: f32,
    pub channel_acoustics: bool,
    pub channel_gamma: bool,
    pub channel_thermal: bool,
    pub peak_cap_db: f32,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            vigil_start: "00:00".to_string(),
            vigil_end: "05:00".to_string(),
            curfew_engaged: true,
            intensity_detent: 1, // POS 1 // SUBTLE
            cognitive_lull_window_sec: 7.5,
            channel_acoustics: true,
            channel_gamma: true,
            channel_thermal: false,
            peak_cap_db: 12.0,
        }
    }
}

impl ProfileConfig {
    /// Returns the human-readable intensity detent label shown in the Séance Panel.
    pub fn intensity_detent_label(&self) -> &'static str {
        match self.intensity_detent {
            1 => "POS 1 // SUBTLE",
            2 => "POS 2 // UNSETTLE",
            3 => "POS 3 // ACUTE",
            _ => "POS 1 // SUBTLE",
        }
    }

    /// Validates profile invariants against hardware & operational guardrails.
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=3).contains(&self.intensity_detent) {
            return Err(format!(
                "Invalid intensity_detent {}: must be 1 (SUBTLE), 2 (UNSETTLE), or 3 (ACUTE)",
                self.intensity_detent
            ));
        }
        if !(4.0..=15.0).contains(&self.cognitive_lull_window_sec) {
            return Err(format!(
                "Invalid cognitive_lull_window_sec {}: must be within [4.0, 15.0]",
                self.cognitive_lull_window_sec
            ));
        }
        if !(6.0..=18.0).contains(&self.peak_cap_db) {
            return Err(format!(
                "Invalid peak_cap_db {}: must be within [6.0, 18.0] dB",
                self.peak_cap_db
            ));
        }
        Ok(())
    }
}

/// Input payload for inserting a new incident into `incident_audit_log`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewIncidentRecord {
    pub timestamp_utc: String,
    pub trigger_category: String,
    pub intensity_detent: u8,
    pub milligauss_reading: f32,
    pub lull_duration_ms: i64,
    pub cpu_temp_c: f32,
    pub gamma_delta_pct: f32,
    pub suppressed_flag: bool,
}

/// Stored row retrieved from `incident_audit_log`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncidentAuditRecord {
    pub id: i64,
    pub timestamp_utc: String,
    pub trigger_category: String,
    pub intensity_detent: u8,
    pub milligauss_reading: f32,
    pub lull_duration_ms: i64,
    pub cpu_temp_c: f32,
    pub gamma_delta_pct: f32,
    pub suppressed_flag: bool,
}

/// Resolves the OS-standard local application data directory strictly without network calls.
pub fn resolve_app_data_dir() -> Result<PathBuf, String> {
    let base_dir = if cfg!(target_os = "macos") {
        let home = env::var_os("HOME")
            .ok_or_else(|| "Missing HOME environment variable on macOS".to_string())?;
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER)
    } else if cfg!(target_os = "windows") {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            PathBuf::from(local_app_data).join(APP_IDENTIFIER)
        } else if let Some(user_profile) = env::var_os("USERPROFILE") {
            PathBuf::from(user_profile)
                .join("AppData")
                .join("Local")
                .join(APP_IDENTIFIER)
        } else {
            return Err("Missing LOCALAPPDATA and USERPROFILE on Windows".to_string());
        }
    } else if let Some(xdg_data_home) = env::var_os("XDG_DATA_HOME") {
        let xdg_path = PathBuf::from(xdg_data_home);
        if !xdg_path.is_absolute() {
            return Err("XDG_DATA_HOME must be an absolute path".to_string());
        }
        xdg_path.join(XDG_SUBDIR)
    } else {
        let home = env::var_os("HOME")
            .ok_or_else(|| "Missing HOME and XDG_DATA_HOME environment variables".to_string())?;
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join(XDG_SUBDIR)
    };

    validate_no_parent_traversal(&base_dir)?;
    Ok(base_dir)
}

fn validate_no_parent_traversal(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "Application data path must be absolute: {}",
            path.display()
        ));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(format!(
                "Parent directory traversal ('..') is forbidden in data path: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn ensure_secure_directory(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create app data directory {}: {e}", dir.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o700);
        fs::set_permissions(dir, perms).map_err(|e| {
            format!(
                "Failed to set 0700 permissions on directory {}: {e}",
                dir.display()
            )
        })?;
    }

    Ok(())
}

fn tighten_db_file_permissions(db_path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for suffix in ["", "-wal", "-shm"] {
            let candidate = if suffix.is_empty() {
                db_path.to_path_buf()
            } else {
                let mut os = db_path.as_os_str().to_os_string();
                os.push(suffix);
                PathBuf::from(os)
            };
            if candidate.exists() {
                let perms = fs::Permissions::from_mode(0o600);
                fs::set_permissions(&candidate, perms).map_err(|e| {
                    format!(
                        "Failed to set 0600 permissions on database file {}: {e}",
                        candidate.display()
                    )
                })?;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = db_path;
    }
    Ok(())
}

fn unix_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Thread-safe embedded SQLite persistence store running in Write-Ahead Logging (WAL) mode.
pub struct LocalPersistenceStore {
    db_path: PathBuf,
    conn: Mutex<Connection>,
}

impl LocalPersistenceStore {
    /// Resolves the OS-standard local application data directory and initializes the WAL database.
    pub fn init_wal_database() -> Result<Self, String> {
        let app_dir = resolve_app_data_dir()?;
        ensure_secure_directory(&app_dir)?;
        let db_path = app_dir.join(DB_FILENAME);
        let store = Self::init_at_path(&db_path)?;
        if store.get_recent_incidents(1)?.is_empty() {
            let _ = store.record_incident(&NewIncidentRecord {
                timestamp_utc: "02:39:12".to_string(),
                trigger_category: "REM_POD_CARRIER_SYNC".to_string(),
                intensity_detent: 1,
                milligauss_reading: 2.87,
                lull_duration_ms: 0,
                cpu_temp_c: 18.2,
                gamma_delta_pct: 0.0,
                suppressed_flag: false,
            });
            let _ = store.record_incident(&NewIncidentRecord {
                timestamp_utc: "02:40:04".to_string(),
                trigger_category: "EVP CADENCE DETECTED: 7.2s LULL".to_string(),
                intensity_detent: 1,
                milligauss_reading: 2.87,
                lull_duration_ms: 7200,
                cpu_temp_c: 18.2,
                gamma_delta_pct: 0.0,
                suppressed_flag: true,
            });
            let _ = store.record_incident(&NewIncidentRecord {
                timestamp_utc: "02:41:02".to_string(),
                trigger_category: "AMBIENT_ACOUSTIC_FLOOR".to_string(),
                intensity_detent: 1,
                milligauss_reading: 1.50,
                lull_duration_ms: 0,
                cpu_temp_c: 18.2,
                gamma_delta_pct: 0.0,
                suppressed_flag: false,
            });
        }
        Ok(store)
    }

    /// Initializes the SQLite database in WAL mode at an explicit filesystem path.
    pub fn init_at_path(db_path: &Path) -> Result<Self, String> {
        validate_no_parent_traversal(db_path)?;
        if let Some(parent) = db_path.parent() {
            ensure_secure_directory(parent)?;
        }

        let mut conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open SQLite database {}: {e}", db_path.display()))?;

        // Configure WAL mode, normal synchronous commits, and foreign keys
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))
            .map_err(|e| format!("Failed to enable SQLite WAL journal_mode: {e}"))?;

        if !journal_mode.eq_ignore_ascii_case("wal") {
            return Err(format!(
                "Unexpected SQLite journal_mode '{journal_mode}' (expected 'wal')"
            ));
        }

        conn.execute_batch(
            "PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(|e| format!("Failed to configure SQLite PRAGMA settings: {e}"))?;

        // Create schema for `profile_settings` and `incident_audit_log`
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profile_settings (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL,
                updated_at_ms INTEGER NOT NULL
            ) STRICT;

            CREATE TABLE IF NOT EXISTS incident_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_utc TEXT NOT NULL,
                trigger_category TEXT NOT NULL,
                intensity_detent INTEGER NOT NULL CHECK (intensity_detent IN (1, 2, 3)),
                milligauss_reading REAL NOT NULL,
                lull_duration_ms INTEGER NOT NULL,
                cpu_temp_c REAL NOT NULL,
                gamma_delta_pct REAL NOT NULL CHECK (gamma_delta_pct >= 0.0 AND gamma_delta_pct <= 8.0),
                suppressed_flag INTEGER NOT NULL CHECK (suppressed_flag IN (0, 1))
            ) STRICT;

            CREATE INDEX IF NOT EXISTS idx_incident_audit_timestamp
                ON incident_audit_log(timestamp_utc DESC);",
        )
        .map_err(|e| format!("Failed to initialize SQLite schema: {e}"))?;

        // Seed initial default profile settings atomically without overwriting existing user values
        Self::seed_defaults(&mut conn)?;

        tighten_db_file_permissions(db_path)?;

        Ok(Self {
            db_path: db_path.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    fn seed_defaults(conn: &mut Connection) -> Result<(), String> {
        let defaults = ProfileConfig::default();
        let now_ms = unix_epoch_ms();
        let pairs = [
            ("vigil_start", defaults.vigil_start),
            ("vigil_end", defaults.vigil_end),
            ("curfew_engaged", defaults.curfew_engaged.to_string()),
            ("intensity_detent", defaults.intensity_detent.to_string()),
            (
                "cognitive_lull_window_sec",
                format!("{:.1}", defaults.cognitive_lull_window_sec),
            ),
            ("channel_acoustics", defaults.channel_acoustics.to_string()),
            ("channel_gamma", defaults.channel_gamma.to_string()),
            ("channel_thermal", defaults.channel_thermal.to_string()),
            ("peak_cap_db", format!("{:.1}", defaults.peak_cap_db)),
        ];

        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to begin seed transaction: {e}"))?;

        for (key, val) in pairs {
            tx.execute(
                "INSERT INTO profile_settings (key, value, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO NOTHING;",
                params![key, val, now_ms],
            )
            .map_err(|e| format!("Failed to seed profile key '{key}': {e}"))?;
        }

        tx.commit()
            .map_err(|e| format!("Failed to commit seed transaction: {e}"))?;
        Ok(())
    }

    /// Returns the absolute filesystem path of the SQLite database file.
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Queries the active SQLite journal mode (guaranteed to be `"wal"` for disk databases).
    pub fn verify_wal_mode(&self) -> Result<String, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;
        conn.query_row("PRAGMA journal_mode;", [], |row| row.get(0))
            .map_err(|e| format!("Failed to query PRAGMA journal_mode: {e}"))
    }

    /// Atomically inserts or updates a single key-value entry in `profile_settings`.
    pub fn set_profile_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;
        let now_ms = unix_epoch_ms();

        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to start profile upsert transaction: {e}"))?;

        tx.execute(
            "INSERT INTO profile_settings (key, value, updated_at_ms)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                 value = excluded.value,
                 updated_at_ms = excluded.updated_at_ms;",
            params![key, value, now_ms],
        )
        .map_err(|e| format!("Failed to upsert profile key '{key}': {e}"))?;

        tx.commit()
            .map_err(|e| format!("Failed to commit profile upsert transaction: {e}"))?;
        Ok(())
    }

    /// Retrieves a single key's value from `profile_settings`.
    pub fn get_profile_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;
        conn.query_row(
            "SELECT value FROM profile_settings WHERE key = ?1;",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read profile key '{key}': {e}"))
    }

    /// Atomically saves a validated [`ProfileConfig`] across `profile_settings`.
    pub fn save_profile_config(&self, config: &ProfileConfig) -> Result<(), String> {
        config.validate()?;

        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;
        let now_ms = unix_epoch_ms();

        let pairs = [
            ("vigil_start", config.vigil_start.clone()),
            ("vigil_end", config.vigil_end.clone()),
            ("curfew_engaged", config.curfew_engaged.to_string()),
            ("intensity_detent", config.intensity_detent.to_string()),
            (
                "cognitive_lull_window_sec",
                format!("{:.1}", config.cognitive_lull_window_sec),
            ),
            ("channel_acoustics", config.channel_acoustics.to_string()),
            ("channel_gamma", config.channel_gamma.to_string()),
            ("channel_thermal", config.channel_thermal.to_string()),
            ("peak_cap_db", format!("{:.1}", config.peak_cap_db)),
        ];

        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to start profile batch transaction: {e}"))?;

        for (key, val) in pairs {
            tx.execute(
                "INSERT INTO profile_settings (key, value, updated_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     updated_at_ms = excluded.updated_at_ms;",
                params![key, val, now_ms],
            )
            .map_err(|e| format!("Failed to upsert profile key '{key}': {e}"))?;
        }

        tx.commit()
            .map_err(|e| format!("Failed to commit profile batch transaction: {e}"))?;
        Ok(())
    }

    /// Loads the complete [`ProfileConfig`] from `profile_settings`, falling back to calibrated defaults.
    pub fn load_profile_config(&self) -> Result<ProfileConfig, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;

        let mut stmt = conn
            .prepare("SELECT key, value FROM profile_settings;")
            .map_err(|e| format!("Failed to prepare profile_settings query: {e}"))?;

        let mut config = ProfileConfig::default();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Failed to query profile_settings: {e}"))?;

        for row in rows {
            let (key, val) = row.map_err(|e| format!("Failed to decode profile row: {e}"))?;
            match key.as_str() {
                "vigil_start" => config.vigil_start = val,
                "vigil_end" => config.vigil_end = val,
                "curfew_engaged" => {
                    if let Ok(parsed) = val.parse::<bool>() {
                        config.curfew_engaged = parsed;
                    }
                }
                "intensity_detent" => {
                    if let Ok(parsed) = val.parse::<u8>() {
                        config.intensity_detent = parsed;
                    }
                }
                "cognitive_lull_window_sec" => {
                    if let Ok(parsed) = val.parse::<f32>() {
                        config.cognitive_lull_window_sec = parsed;
                    }
                }
                "channel_acoustics" => {
                    if let Ok(parsed) = val.parse::<bool>() {
                        config.channel_acoustics = parsed;
                    }
                }
                "channel_gamma" => {
                    if let Ok(parsed) = val.parse::<bool>() {
                        config.channel_gamma = parsed;
                    }
                }
                "channel_thermal" => {
                    if let Ok(parsed) = val.parse::<bool>() {
                        config.channel_thermal = parsed;
                    }
                }
                "peak_cap_db" => {
                    if let Ok(parsed) = val.parse::<f32>() {
                        config.peak_cap_db = parsed;
                    }
                }
                _ => {}
            }
        }

        Ok(config)
    }

    /// Records a telemetry or manifestation incident in `incident_audit_log` and enforces rolling retention.
    pub fn record_incident(&self, entry: &NewIncidentRecord) -> Result<i64, String> {
        if !(1..=3).contains(&entry.intensity_detent) {
            return Err(format!(
                "Invalid incident intensity_detent {}: must be 1, 2, or 3",
                entry.intensity_detent
            ));
        }
        if !(0.0..=8.0).contains(&entry.gamma_delta_pct) {
            return Err(format!(
                "Invalid gamma_delta_pct {}: must be clamped within [0.0, 8.0]%",
                entry.gamma_delta_pct
            ));
        }

        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;

        let tx = conn
            .transaction()
            .map_err(|e| format!("Failed to begin incident log transaction: {e}"))?;

        tx.execute(
            "INSERT INTO incident_audit_log (
                timestamp_utc,
                trigger_category,
                intensity_detent,
                milligauss_reading,
                lull_duration_ms,
                cpu_temp_c,
                gamma_delta_pct,
                suppressed_flag
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
            params![
                entry.timestamp_utc,
                entry.trigger_category,
                i64::from(entry.intensity_detent),
                f64::from(entry.milligauss_reading),
                entry.lull_duration_ms,
                f64::from(entry.cpu_temp_c),
                f64::from(entry.gamma_delta_pct),
                i64::from(entry.suppressed_flag),
            ],
        )
        .map_err(|e| format!("Failed to insert incident_audit_log entry: {e}"))?;

        let inserted_id = tx.last_insert_rowid();

        // Enforce rolling retention cap (MAX_AUDIT_LOG_ROWS)
        tx.execute(
            "DELETE FROM incident_audit_log
             WHERE id NOT IN (
                 SELECT id FROM incident_audit_log
                 ORDER BY id DESC
                 LIMIT ?1
             );",
            params![MAX_AUDIT_LOG_ROWS],
        )
        .map_err(|e| format!("Failed to prune old incident_audit_log entries: {e}"))?;

        tx.commit()
            .map_err(|e| format!("Failed to commit incident log transaction: {e}"))?;

        Ok(inserted_id)
    }

    /// Returns the most recent incident records ordered newest-first.
    pub fn get_recent_incidents(&self, limit: usize) -> Result<Vec<IncidentAuditRecord>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "SQLite connection mutex poisoned".to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT
                    id,
                    timestamp_utc,
                    trigger_category,
                    intensity_detent,
                    milligauss_reading,
                    lull_duration_ms,
                    cpu_temp_c,
                    gamma_delta_pct,
                    suppressed_flag
                 FROM incident_audit_log
                 ORDER BY id DESC
                 LIMIT ?1;",
            )
            .map_err(|e| format!("Failed to prepare get_recent_incidents query: {e}"))?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                Ok(IncidentAuditRecord {
                    id: row.get(0)?,
                    timestamp_utc: row.get(1)?,
                    trigger_category: row.get(2)?,
                    intensity_detent: row.get::<_, i64>(3)? as u8,
                    milligauss_reading: row.get::<_, f64>(4)? as f32,
                    lull_duration_ms: row.get(5)?,
                    cpu_temp_c: row.get::<_, f64>(6)? as f32,
                    gamma_delta_pct: row.get::<_, f64>(7)? as f32,
                    suppressed_flag: row.get::<_, i64>(8)? != 0,
                })
            })
            .map_err(|e| format!("Failed to execute get_recent_incidents query: {e}"))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| format!("Failed to decode incident row: {e}"))?);
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_db_path(test_name: &str) -> PathBuf {
        let unique = format!(
            "nocturne_test_{}_{}_{}",
            test_name,
            std::process::id(),
            unix_epoch_ms()
        );
        env::temp_dir().join(unique).join(DB_FILENAME)
    }

    #[test]
    fn test_resolve_app_data_dir_is_absolute_and_safe() {
        let path = resolve_app_data_dir().expect("should resolve OS app data dir");
        assert!(path.is_absolute());
        assert!(
            path.to_string_lossy().contains(APP_IDENTIFIER)
                || path.to_string_lossy().contains(XDG_SUBDIR)
        );
    }

    #[test]
    fn test_wal_initialization_and_profile_upsert() {
        let db_path = temp_test_db_path("wal_profile");
        let store =
            LocalPersistenceStore::init_at_path(&db_path).expect("should initialize WAL database");

        assert_eq!(
            store
                .verify_wal_mode()
                .expect("should query journal_mode")
                .to_lowercase(),
            "wal"
        );

        // Verify seeded defaults match screenshot calibration
        let initial = store
            .load_profile_config()
            .expect("should load seeded config");
        assert_eq!(initial, ProfileConfig::default());
        assert_eq!(initial.intensity_detent_label(), "POS 1 // SUBTLE");

        // Update profile via atomic `INSERT ... ON CONFLICT DO UPDATE`
        let mut updated = initial;
        updated.vigil_start = "01:00".to_string();
        updated.vigil_end = "04:30".to_string();
        updated.intensity_detent = 2;
        updated.cognitive_lull_window_sec = 9.0;
        updated.channel_thermal = true;

        store
            .save_profile_config(&updated)
            .expect("should atomically save updated profile");

        let reloaded = store
            .load_profile_config()
            .expect("should reload updated profile");
        assert_eq!(reloaded, updated);
        assert_eq!(reloaded.intensity_detent_label(), "POS 2 // UNSETTLE");

        if let Some(parent) = db_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    #[test]
    fn test_incident_audit_log_insert_and_query() {
        let db_path = temp_test_db_path("audit_log");
        let store =
            LocalPersistenceStore::init_at_path(&db_path).expect("should initialize WAL database");

        let id1 = store
            .record_incident(&NewIncidentRecord {
                timestamp_utc: "2026-10-19T02:40:04Z".to_string(),
                trigger_category: "EVP_CADENCE_LULL".to_string(),
                intensity_detent: 1,
                milligauss_reading: 2.87,
                lull_duration_ms: 7200,
                cpu_temp_c: 18.2,
                gamma_delta_pct: 7.8,
                suppressed_flag: true,
            })
            .expect("should insert incident 1");

        assert!(id1 > 0);

        let logs = store
            .get_recent_incidents(10)
            .expect("should query recent incidents");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].trigger_category, "EVP_CADENCE_LULL");
        assert_eq!(logs[0].lull_duration_ms, 7200);
        assert!(logs[0].suppressed_flag);

        // Out-of-bounds gamma modulation (>8.0%) must be rejected
        let invalid_res = store.record_incident(&NewIncidentRecord {
            timestamp_utc: "2026-10-19T02:41:02Z".to_string(),
            trigger_category: "GAMMA_SPIKE".to_string(),
            intensity_detent: 1,
            milligauss_reading: 2.87,
            lull_duration_ms: 7500,
            cpu_temp_c: 18.2,
            gamma_delta_pct: 9.5,
            suppressed_flag: false,
        });
        assert!(invalid_res.is_err());

        if let Some(parent) = db_path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
