use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use costline_core::{Config, Row};
use serde::Serialize;

pub struct AppState {
    pub config_path: PathBuf,
    pub state_path: PathBuf,
    pub inner: Mutex<Runtime>,
    pub check_lock: Arc<tokio::sync::Mutex<()>>,
}

pub struct Runtime {
    pub config: Config,
    pub watching: bool,
    pub rows: Vec<Row>,
    pub last_check: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub watching: bool,
    pub interval_secs: u64,
    pub assets_count: usize,
    pub rows: Vec<Row>,
    pub last_check: Option<i64>,
    pub last_error: Option<String>,
    pub config_path: String,
}

impl AppState {
    pub fn new(config_path: PathBuf, state_path: PathBuf, config: Config) -> Self {
        let watching = true;
        Self {
            config_path,
            state_path,
            inner: Mutex::new(Runtime {
                config,
                watching,
                rows: Vec::new(),
                last_check: None,
                last_error: None,
            }),
            check_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let runtime = self.lock();
        Snapshot {
            watching: runtime.watching,
            interval_secs: runtime.config.interval_secs,
            assets_count: runtime.config.assets.len(),
            rows: runtime.rows.clone(),
            last_check: runtime.last_check,
            last_error: runtime.last_error.clone(),
            config_path: self.config_path.display().to_string(),
        }
    }

    pub fn lock(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}
