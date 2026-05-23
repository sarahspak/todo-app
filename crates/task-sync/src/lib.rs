//! Provider-neutral sync contracts.

use task_core::{Task, TaskId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncMode {
    Off,
    ImportOnly,
    TwoWay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalProvider {
    TaskNotes,
    Calendar,
    Llm,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLink {
    pub id: String,
    pub task_id: TaskId,
    pub provider: ExternalProvider,
    pub external_id: Option<String>,
    pub external_path: Option<String>,
    pub last_synced_at_ms: Option<i64>,
    pub sync_hash: Option<String>,
    pub sync_state: SyncState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    Linked,
    PendingLocalCreate,
    PendingLocalUpdate,
    PendingRemoteUpdate,
    Conflict,
    DeletedRemote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Disabled,
    Conflict(String),
    Provider(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPreview {
    pub creates: Vec<Task>,
    pub updates: Vec<Task>,
    pub conflicts: Vec<TaskId>,
}

pub trait TaskSyncProvider {
    fn mode(&self) -> SyncMode;
    fn preview(&self) -> Result<SyncPreview, SyncError>;
    fn import_tasks(&self) -> Result<Vec<Task>, SyncError>;
    fn push_tasks(&self, tasks: &[Task]) -> Result<(), SyncError>;
}
