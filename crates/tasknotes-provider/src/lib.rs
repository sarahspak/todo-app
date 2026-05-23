//! TaskNotes integration boundary.

use task_core::Task;
use task_sync::{SyncError, SyncMode, SyncPreview, TaskSyncProvider};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNotesConfig {
    pub vault_path: String,
    pub tasks_glob: String,
    pub mode: SyncMode,
}

#[derive(Debug, Clone)]
pub struct TaskNotesProvider {
    config: TaskNotesConfig,
}

impl TaskNotesProvider {
    pub fn new(config: TaskNotesConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TaskNotesConfig {
        &self.config
    }
}

impl TaskSyncProvider for TaskNotesProvider {
    fn mode(&self) -> SyncMode {
        self.config.mode.clone()
    }

    fn preview(&self) -> Result<SyncPreview, SyncError> {
        Ok(SyncPreview {
            creates: Vec::new(),
            updates: Vec::new(),
            conflicts: Vec::new(),
        })
    }

    fn import_tasks(&self) -> Result<Vec<Task>, SyncError> {
        if self.config.mode == SyncMode::Off {
            return Err(SyncError::Disabled);
        }

        Ok(Vec::new())
    }

    fn push_tasks(&self, _tasks: &[Task]) -> Result<(), SyncError> {
        match self.config.mode {
            SyncMode::TwoWay => Ok(()),
            SyncMode::Off | SyncMode::ImportOnly => Err(SyncError::Disabled),
        }
    }
}
