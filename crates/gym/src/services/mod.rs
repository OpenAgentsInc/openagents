//! Data services for TBCC live data integration

mod task_loader;
mod run_store;
mod settings_store;

pub use task_loader::*;
pub use run_store::*;
pub use settings_store::*;

use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use directories::ProjectDirs;

/// Central data service for TBCC components
pub struct TBCCDataService {
    task_loader: TaskLoader,
    run_store: Arc<RwLock<RunStore>>,
    settings_store: SettingsStore,
}

impl TBCCDataService {
    /// Create a new data service, initializing storage paths
    pub fn new() -> Self {
        let data_dir = Self::get_data_dir();

        Self {
            task_loader: TaskLoader::new(),
            run_store: Arc::new(RwLock::new(RunStore::new(data_dir.clone()))),
            settings_store: SettingsStore::new(data_dir),
        }
    }

    /// Get the application data directory
    fn get_data_dir() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "openagents", "commander") {
            let data_dir = proj_dirs.data_dir().to_path_buf();
            std::fs::create_dir_all(&data_dir).ok();
            data_dir
        } else {
            // Fallback to current directory
            PathBuf::from(".")
        }
    }

    /// Get task loader reference
    pub fn tasks(&self) -> &TaskLoader {
        &self.task_loader
    }

    /// Get run store reference
    pub fn runs(&self) -> Arc<RwLock<RunStore>> {
        self.run_store.clone()
    }

    /// Get settings store reference
    pub fn settings(&self) -> &SettingsStore {
        &self.settings_store
    }
}

impl Default for TBCCDataService {
    fn default() -> Self {
        Self::new()
    }
}
