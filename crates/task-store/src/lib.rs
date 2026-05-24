//! Storage traits for the local task database.

use std::{collections::BTreeMap, path::Path};

use rusqlite::{params, Connection};
use task_core::{DeadlineType, EnergyLevel, Priority, Task, TaskId, TaskStatus, TimeBlock};
use task_sync::{ExternalLink, ExternalProvider, SyncState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    NotFound(TaskId),
    Conflict(String),
    Backend(String),
}

pub trait TaskStore {
    fn upsert_task(&mut self, task: Task) -> Result<(), StoreError>;
    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, StoreError>;
    fn list_tasks(&self) -> Result<Vec<Task>, StoreError>;
    fn list_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, StoreError>;
    fn reorder_tasks(&mut self, task_ids: &[TaskId]) -> Result<(), StoreError>;
    fn delete_task(&mut self, id: &TaskId) -> Result<(), StoreError>;
}

pub trait ExternalLinkStore {
    fn upsert_external_link(&mut self, link: ExternalLink) -> Result<(), StoreError>;
    fn get_external_link(&self, id: &str) -> Result<Option<ExternalLink>, StoreError>;
    fn get_external_link_by_external_id(
        &self,
        provider: &ExternalProvider,
        external_id: &str,
    ) -> Result<Option<ExternalLink>, StoreError>;
    fn get_external_link_by_external_path(
        &self,
        provider: &ExternalProvider,
        external_path: &str,
    ) -> Result<Option<ExternalLink>, StoreError>;
    fn list_external_links_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<ExternalLink>, StoreError>;
    fn list_external_links(&self) -> Result<Vec<ExternalLink>, StoreError>;
}

#[derive(Debug, Default)]
pub struct InMemoryTaskStore {
    tasks: BTreeMap<TaskId, Task>,
}

impl TaskStore for InMemoryTaskStore {
    fn upsert_task(&mut self, task: Task) -> Result<(), StoreError> {
        self.tasks.insert(task.id.clone(), task);
        Ok(())
    }

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, StoreError> {
        Ok(self.tasks.get(id).cloned())
    }

    fn list_tasks(&self) -> Result<Vec<Task>, StoreError> {
        let mut tasks: Vec<Task> = self.tasks.values().cloned().collect();
        tasks.sort_by_key(|task| (task.sort_order, task.created_at_ms, task.id.clone()));
        Ok(tasks)
    }

    fn list_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, StoreError> {
        let mut tasks: Vec<Task> = self
            .tasks
            .values()
            .filter(|task| task.status == status)
            .cloned()
            .collect();
        tasks.sort_by_key(|task| (task.sort_order, task.created_at_ms, task.id.clone()));
        Ok(tasks)
    }

    fn reorder_tasks(&mut self, task_ids: &[TaskId]) -> Result<(), StoreError> {
        for (index, task_id) in task_ids.iter().enumerate() {
            let task = self
                .tasks
                .get_mut(task_id)
                .ok_or_else(|| StoreError::NotFound(task_id.clone()))?;
            task.sort_order = index as i64;
        }

        Ok(())
    }

    fn delete_task(&mut self, id: &TaskId) -> Result<(), StoreError> {
        self.tasks
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| StoreError::NotFound(id.clone()))
    }
}

#[derive(Debug)]
pub struct SqliteTaskStore {
    connection: Connection,
}

impl SqliteTaskStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path).map_err(sqlite_error)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        self.connection
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS tasks (
                    id TEXT PRIMARY KEY NOT NULL,
                    title TEXT NOT NULL,
                    notes TEXT,
                    status TEXT NOT NULL,
                    priority TEXT NOT NULL,
                    due_at_ms INTEGER,
                    deadline_type TEXT NOT NULL,
                    scheduled_start_ms INTEGER,
                    scheduled_end_ms INTEGER,
                    estimate_minutes INTEGER,
                    energy_level TEXT,
                    context TEXT,
                    tags_json TEXT NOT NULL,
                    project_id TEXT,
                    created_at_ms INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL,
                    completed_at_ms INTEGER,
                    sort_order INTEGER NOT NULL DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
                CREATE INDEX IF NOT EXISTS idx_tasks_updated_at_ms ON tasks(updated_at_ms);

                CREATE TABLE IF NOT EXISTS external_links (
                    id TEXT PRIMARY KEY NOT NULL,
                    task_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    external_id TEXT,
                    external_path TEXT,
                    last_synced_at_ms INTEGER,
                    sync_hash TEXT,
                    sync_state TEXT NOT NULL,
                    FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
                );

                CREATE UNIQUE INDEX IF NOT EXISTS idx_external_links_task_provider
                    ON external_links(task_id, provider);
                CREATE UNIQUE INDEX IF NOT EXISTS idx_external_links_provider_external_id
                    ON external_links(provider, external_id)
                    WHERE external_id IS NOT NULL;
                CREATE UNIQUE INDEX IF NOT EXISTS idx_external_links_provider_external_path
                    ON external_links(provider, external_path)
                    WHERE external_path IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_external_links_sync_state
                    ON external_links(sync_state);
                ",
            )
            .map_err(sqlite_error)?;

        if !self.column_exists("tasks", "completed_at_ms")? {
            self.connection
                .execute("ALTER TABLE tasks ADD COLUMN completed_at_ms INTEGER", [])
                .map_err(sqlite_error)?;
        }

        if !self.column_exists("tasks", "sort_order")? {
            self.connection
                .execute(
                    "ALTER TABLE tasks ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
                    [],
                )
                .map_err(sqlite_error)?;
            self.connection
                .execute(
                    "UPDATE tasks SET sort_order = created_at_ms WHERE sort_order = 0",
                    [],
                )
                .map_err(sqlite_error)?;
        }

        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool, StoreError> {
        let mut statement = self
            .connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(sqlite_error)?;
        let mut rows = statement.query([]).map_err(sqlite_error)?;

        while let Some(row) = rows.next().map_err(sqlite_error)? {
            let name: String = row.get("name").map_err(sqlite_error)?;
            if name == column {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

impl ExternalLinkStore for SqliteTaskStore {
    fn upsert_external_link(&mut self, link: ExternalLink) -> Result<(), StoreError> {
        self.connection
            .execute(
                "
                INSERT INTO external_links (
                    id,
                    task_id,
                    provider,
                    external_id,
                    external_path,
                    last_synced_at_ms,
                    sync_hash,
                    sync_state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(id) DO UPDATE SET
                    task_id = excluded.task_id,
                    provider = excluded.provider,
                    external_id = excluded.external_id,
                    external_path = excluded.external_path,
                    last_synced_at_ms = excluded.last_synced_at_ms,
                    sync_hash = excluded.sync_hash,
                    sync_state = excluded.sync_state
                ",
                params![
                    link.id,
                    link.task_id,
                    external_provider_to_str(&link.provider),
                    link.external_id,
                    link.external_path,
                    link.last_synced_at_ms,
                    link.sync_hash,
                    sync_state_to_str(&link.sync_state),
                ],
            )
            .map_err(sqlite_error)?;

        Ok(())
    }

    fn get_external_link(&self, id: &str) -> Result<Option<ExternalLink>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM external_links WHERE id = ?1")
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![id]).map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => external_link_from_row(row).map(Some),
            None => Ok(None),
        }
    }

    fn get_external_link_by_external_id(
        &self,
        provider: &ExternalProvider,
        external_id: &str,
    ) -> Result<Option<ExternalLink>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM external_links WHERE provider = ?1 AND external_id = ?2")
            .map_err(sqlite_error)?;
        let mut rows = statement
            .query(params![external_provider_to_str(provider), external_id])
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => external_link_from_row(row).map(Some),
            None => Ok(None),
        }
    }

    fn get_external_link_by_external_path(
        &self,
        provider: &ExternalProvider,
        external_path: &str,
    ) -> Result<Option<ExternalLink>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM external_links WHERE provider = ?1 AND external_path = ?2")
            .map_err(sqlite_error)?;
        let mut rows = statement
            .query(params![external_provider_to_str(provider), external_path])
            .map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => external_link_from_row(row).map(Some),
            None => Ok(None),
        }
    }

    fn list_external_links_for_task(
        &self,
        task_id: &TaskId,
    ) -> Result<Vec<ExternalLink>, StoreError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT * FROM external_links WHERE task_id = ?1 ORDER BY provider ASC, id ASC",
            )
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![task_id]).map_err(sqlite_error)?;
        let mut links = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            links.push(external_link_from_row(row)?);
        }

        Ok(links)
    }

    fn list_external_links(&self) -> Result<Vec<ExternalLink>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM external_links ORDER BY provider ASC, id ASC")
            .map_err(sqlite_error)?;
        let mut rows = statement.query([]).map_err(sqlite_error)?;
        let mut links = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            links.push(external_link_from_row(row)?);
        }

        Ok(links)
    }
}

impl TaskStore for SqliteTaskStore {
    fn upsert_task(&mut self, task: Task) -> Result<(), StoreError> {
        let tags_json = serde_json::to_string(&task.tags)
            .map_err(|error| StoreError::Backend(format!("failed to encode tags: {error}")))?;
        let (scheduled_start_ms, scheduled_end_ms) = match task.scheduled {
            Some(block) => (Some(block.start_ms), Some(block.end_ms)),
            None => (None, None),
        };

        self.connection
            .execute(
                "
                INSERT INTO tasks (
                    id,
                    title,
                    notes,
                    status,
                    priority,
                    due_at_ms,
                    deadline_type,
                    scheduled_start_ms,
                    scheduled_end_ms,
                    estimate_minutes,
                    energy_level,
                    context,
                    tags_json,
                    project_id,
                    created_at_ms,
                    updated_at_ms,
                    completed_at_ms,
                    sort_order
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    notes = excluded.notes,
                    status = excluded.status,
                    priority = excluded.priority,
                    due_at_ms = excluded.due_at_ms,
                    deadline_type = excluded.deadline_type,
                    scheduled_start_ms = excluded.scheduled_start_ms,
                    scheduled_end_ms = excluded.scheduled_end_ms,
                    estimate_minutes = excluded.estimate_minutes,
                    energy_level = excluded.energy_level,
                    context = excluded.context,
                    tags_json = excluded.tags_json,
                    project_id = excluded.project_id,
                    created_at_ms = excluded.created_at_ms,
                    updated_at_ms = excluded.updated_at_ms,
                    completed_at_ms = excluded.completed_at_ms,
                    sort_order = excluded.sort_order
                ",
                params![
                    task.id,
                    task.title,
                    task.notes,
                    status_to_str(&task.status),
                    priority_to_str(&task.priority),
                    task.due_at_ms,
                    deadline_type_to_str(&task.deadline_type),
                    scheduled_start_ms,
                    scheduled_end_ms,
                    task.estimate_minutes,
                    task.energy_level.as_ref().map(energy_level_to_str),
                    task.context,
                    tags_json,
                    task.project_id,
                    task.created_at_ms,
                    task.updated_at_ms,
                    task.completed_at_ms,
                    task.sort_order,
                ],
            )
            .map_err(sqlite_error)?;

        Ok(())
    }

    fn get_task(&self, id: &TaskId) -> Result<Option<Task>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM tasks WHERE id = ?1")
            .map_err(sqlite_error)?;
        let mut rows = statement.query(params![id]).map_err(sqlite_error)?;
        match rows.next().map_err(sqlite_error)? {
            Some(row) => task_from_row(row).map(Some),
            None => Ok(None),
        }
    }

    fn list_tasks(&self) -> Result<Vec<Task>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM tasks ORDER BY sort_order ASC, created_at_ms ASC, id ASC")
            .map_err(sqlite_error)?;
        let mut rows = statement.query([]).map_err(sqlite_error)?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            tasks.push(task_from_row(row)?);
        }

        Ok(tasks)
    }

    fn list_tasks_by_status(&self, status: TaskStatus) -> Result<Vec<Task>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM tasks WHERE status = ?1 ORDER BY sort_order ASC, created_at_ms ASC, id ASC")
            .map_err(sqlite_error)?;
        let mut rows = statement
            .query(params![status_to_str(&status)])
            .map_err(sqlite_error)?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next().map_err(sqlite_error)? {
            tasks.push(task_from_row(row)?);
        }

        Ok(tasks)
    }

    fn reorder_tasks(&mut self, task_ids: &[TaskId]) -> Result<(), StoreError> {
        let transaction = self.connection.transaction().map_err(sqlite_error)?;

        for (index, task_id) in task_ids.iter().enumerate() {
            let affected = transaction
                .execute(
                    "UPDATE tasks SET sort_order = ?1 WHERE id = ?2",
                    params![index as i64, task_id],
                )
                .map_err(sqlite_error)?;

            if affected == 0 {
                return Err(StoreError::NotFound(task_id.clone()));
            }
        }

        transaction.commit().map_err(sqlite_error)
    }

    fn delete_task(&mut self, id: &TaskId) -> Result<(), StoreError> {
        let affected = self
            .connection
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])
            .map_err(sqlite_error)?;

        if affected == 0 {
            Err(StoreError::NotFound(id.clone()))
        } else {
            Ok(())
        }
    }
}

fn task_from_row(row: &rusqlite::Row<'_>) -> Result<Task, StoreError> {
    let scheduled_start_ms: Option<i64> = row.get("scheduled_start_ms").map_err(sqlite_error)?;
    let scheduled_end_ms: Option<i64> = row.get("scheduled_end_ms").map_err(sqlite_error)?;
    let tags_json: String = row.get("tags_json").map_err(sqlite_error)?;
    let energy_level: Option<String> = row.get("energy_level").map_err(sqlite_error)?;
    let status: String = row.get("status").map_err(sqlite_error)?;
    let priority: String = row.get("priority").map_err(sqlite_error)?;
    let deadline_type: String = row.get("deadline_type").map_err(sqlite_error)?;

    Ok(Task {
        id: row.get("id").map_err(sqlite_error)?,
        title: row.get("title").map_err(sqlite_error)?,
        notes: row.get("notes").map_err(sqlite_error)?,
        status: parse_status(&status)?,
        priority: parse_priority(&priority)?,
        due_at_ms: row.get("due_at_ms").map_err(sqlite_error)?,
        deadline_type: parse_deadline_type(&deadline_type)?,
        scheduled: match (scheduled_start_ms, scheduled_end_ms) {
            (Some(start_ms), Some(end_ms)) => Some(TimeBlock { start_ms, end_ms }),
            _ => None,
        },
        estimate_minutes: row.get("estimate_minutes").map_err(sqlite_error)?,
        energy_level: energy_level
            .as_deref()
            .map(parse_energy_level)
            .transpose()?,
        context: row.get("context").map_err(sqlite_error)?,
        tags: serde_json::from_str(&tags_json)
            .map_err(|error| StoreError::Backend(format!("failed to decode tags: {error}")))?,
        project_id: row.get("project_id").map_err(sqlite_error)?,
        created_at_ms: row.get("created_at_ms").map_err(sqlite_error)?,
        updated_at_ms: row.get("updated_at_ms").map_err(sqlite_error)?,
        completed_at_ms: row.get("completed_at_ms").map_err(sqlite_error)?,
        sort_order: row.get("sort_order").map_err(sqlite_error)?,
    })
}

fn external_link_from_row(row: &rusqlite::Row<'_>) -> Result<ExternalLink, StoreError> {
    let provider: String = row.get("provider").map_err(sqlite_error)?;
    let sync_state: String = row.get("sync_state").map_err(sqlite_error)?;

    Ok(ExternalLink {
        id: row.get("id").map_err(sqlite_error)?,
        task_id: row.get("task_id").map_err(sqlite_error)?,
        provider: parse_external_provider(&provider),
        external_id: row.get("external_id").map_err(sqlite_error)?,
        external_path: row.get("external_path").map_err(sqlite_error)?,
        last_synced_at_ms: row.get("last_synced_at_ms").map_err(sqlite_error)?,
        sync_hash: row.get("sync_hash").map_err(sqlite_error)?,
        sync_state: parse_sync_state(&sync_state)?,
    })
}

fn sqlite_error(error: rusqlite::Error) -> StoreError {
    StoreError::Backend(error.to_string())
}

fn external_provider_to_str(provider: &ExternalProvider) -> &str {
    match provider {
        ExternalProvider::TaskNotes => "TaskNotes",
        ExternalProvider::Calendar => "Calendar",
        ExternalProvider::Llm => "Llm",
        ExternalProvider::Other(value) => value.as_str(),
    }
}

fn parse_external_provider(value: &str) -> ExternalProvider {
    match value {
        "TaskNotes" => ExternalProvider::TaskNotes,
        "Calendar" => ExternalProvider::Calendar,
        "Llm" => ExternalProvider::Llm,
        other => ExternalProvider::Other(other.into()),
    }
}

fn sync_state_to_str(sync_state: &SyncState) -> &'static str {
    match sync_state {
        SyncState::Linked => "Linked",
        SyncState::PendingLocalCreate => "PendingLocalCreate",
        SyncState::PendingLocalUpdate => "PendingLocalUpdate",
        SyncState::PendingRemoteUpdate => "PendingRemoteUpdate",
        SyncState::Conflict => "Conflict",
        SyncState::DeletedRemote => "DeletedRemote",
    }
}

fn parse_sync_state(value: &str) -> Result<SyncState, StoreError> {
    match value {
        "Linked" => Ok(SyncState::Linked),
        "PendingLocalCreate" => Ok(SyncState::PendingLocalCreate),
        "PendingLocalUpdate" => Ok(SyncState::PendingLocalUpdate),
        "PendingRemoteUpdate" => Ok(SyncState::PendingRemoteUpdate),
        "Conflict" => Ok(SyncState::Conflict),
        "DeletedRemote" => Ok(SyncState::DeletedRemote),
        _ => Err(StoreError::Backend(format!("unknown sync state: {value}"))),
    }
}

fn status_to_str(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Inbox => "Inbox",
        TaskStatus::Planned => "Planned",
        TaskStatus::InProgress => "InProgress",
        TaskStatus::Done => "Done",
        TaskStatus::Canceled => "Canceled",
    }
}

fn parse_status(value: &str) -> Result<TaskStatus, StoreError> {
    match value {
        "Inbox" => Ok(TaskStatus::Inbox),
        "Planned" => Ok(TaskStatus::Planned),
        "InProgress" => Ok(TaskStatus::InProgress),
        "Done" => Ok(TaskStatus::Done),
        "Canceled" => Ok(TaskStatus::Canceled),
        _ => Err(StoreError::Backend(format!("unknown task status: {value}"))),
    }
}

fn priority_to_str(priority: &Priority) -> &'static str {
    match priority {
        Priority::Low => "Low",
        Priority::Normal => "Normal",
        Priority::High => "High",
        Priority::Urgent => "Urgent",
    }
}

fn parse_priority(value: &str) -> Result<Priority, StoreError> {
    match value {
        "Low" => Ok(Priority::Low),
        "Normal" => Ok(Priority::Normal),
        "High" => Ok(Priority::High),
        "Urgent" => Ok(Priority::Urgent),
        _ => Err(StoreError::Backend(format!("unknown priority: {value}"))),
    }
}

fn deadline_type_to_str(deadline_type: &DeadlineType) -> &'static str {
    match deadline_type {
        DeadlineType::None => "None",
        DeadlineType::Soft => "Soft",
        DeadlineType::Hard => "Hard",
    }
}

fn parse_deadline_type(value: &str) -> Result<DeadlineType, StoreError> {
    match value {
        "None" => Ok(DeadlineType::None),
        "Soft" => Ok(DeadlineType::Soft),
        "Hard" => Ok(DeadlineType::Hard),
        _ => Err(StoreError::Backend(format!(
            "unknown deadline type: {value}"
        ))),
    }
}

fn energy_level_to_str(energy_level: &EnergyLevel) -> &'static str {
    match energy_level {
        EnergyLevel::Low => "Low",
        EnergyLevel::Medium => "Medium",
        EnergyLevel::High => "High",
    }
}

fn parse_energy_level(value: &str) -> Result<EnergyLevel, StoreError> {
    match value {
        "Low" => Ok(EnergyLevel::Low),
        "Medium" => Ok(EnergyLevel::Medium),
        "High" => Ok(EnergyLevel::High),
        _ => Err(StoreError::Backend(format!(
            "unknown energy level: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn temp_store_path(label: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "todo-app-{label}-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ))
    }

    fn test_task(id: &str) -> Task {
        Task {
            id: id.into(),
            title: "Linked task".into(),
            notes: None,
            status: TaskStatus::Inbox,
            priority: Priority::Normal,
            due_at_ms: None,
            deadline_type: DeadlineType::None,
            scheduled: None,
            estimate_minutes: None,
            energy_level: None,
            context: None,
            tags: Vec::new(),
            project_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            completed_at_ms: None,
            sort_order: 1,
        }
    }

    fn tasknotes_link(
        id: &str,
        task_id: &str,
        external_id: &str,
        external_path: &str,
    ) -> ExternalLink {
        ExternalLink {
            id: id.into(),
            task_id: task_id.into(),
            provider: ExternalProvider::TaskNotes,
            external_id: Some(external_id.into()),
            external_path: Some(external_path.into()),
            last_synced_at_ms: Some(100),
            sync_hash: Some("hash-1".into()),
            sync_state: SyncState::Linked,
        }
    }

    #[test]
    fn sqlite_store_persists_tasks_after_reopen() {
        let path = env::temp_dir().join(format!(
            "todo-app-task-store-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ));

        let task = Task {
            id: "task-1".into(),
            title: "Write persistent task".into(),
            notes: Some("Created from the Inbox editor.".into()),
            status: TaskStatus::Inbox,
            priority: Priority::Normal,
            due_at_ms: None,
            deadline_type: DeadlineType::None,
            scheduled: None,
            estimate_minutes: None,
            energy_level: None,
            context: None,
            tags: vec!["inbox".into()],
            project_id: None,
            created_at_ms: 10,
            updated_at_ms: 10,
            completed_at_ms: None,
            sort_order: 10,
        };

        {
            let mut store = SqliteTaskStore::open(&path).expect("store should open");
            store
                .upsert_task(task.clone())
                .expect("task should be inserted");
        }

        let store = SqliteTaskStore::open(&path).expect("store should reopen");
        assert_eq!(
            store.get_task(&task.id).expect("task should load"),
            Some(task)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_filters_and_deletes_tasks() {
        let path = env::temp_dir().join(format!(
            "todo-app-task-store-filter-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ));
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let inbox = Task {
            id: "task-inbox".into(),
            title: "Inbox task".into(),
            notes: None,
            status: TaskStatus::Inbox,
            priority: Priority::Normal,
            due_at_ms: None,
            deadline_type: DeadlineType::None,
            scheduled: None,
            estimate_minutes: None,
            energy_level: None,
            context: None,
            tags: Vec::new(),
            project_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            completed_at_ms: None,
            sort_order: 1,
        };
        let done = Task {
            id: "task-done".into(),
            status: TaskStatus::Done,
            completed_at_ms: Some(2),
            sort_order: 2,
            ..inbox.clone()
        };

        store
            .upsert_task(inbox.clone())
            .expect("inbox should insert");
        store.upsert_task(done).expect("done should insert");

        assert_eq!(
            store
                .list_tasks_by_status(TaskStatus::Inbox)
                .expect("tasks should load"),
            vec![inbox.clone()]
        );

        store
            .delete_task(&inbox.id)
            .expect("task should be deleted");
        assert_eq!(store.get_task(&inbox.id).expect("task should load"), None);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_updates_existing_tasks() {
        let path = env::temp_dir().join(format!(
            "todo-app-task-store-update-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ));
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let mut task = Task {
            id: "task-update".into(),
            title: "Original title".into(),
            notes: None,
            status: TaskStatus::Inbox,
            priority: Priority::Normal,
            due_at_ms: None,
            deadline_type: DeadlineType::None,
            scheduled: None,
            estimate_minutes: None,
            energy_level: None,
            context: None,
            tags: Vec::new(),
            project_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            completed_at_ms: None,
            sort_order: 1,
        };

        store.upsert_task(task.clone()).expect("task should insert");
        task.title = "Updated title".into();
        task.notes = Some("Updated notes".into());
        task.updated_at_ms = 2;
        store.upsert_task(task.clone()).expect("task should update");

        assert_eq!(store.list_tasks().expect("tasks should load"), vec![task]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_reorders_tasks() {
        let path = env::temp_dir().join(format!(
            "todo-app-task-store-reorder-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ));
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let first = Task {
            id: "task-first".into(),
            title: "First".into(),
            notes: None,
            status: TaskStatus::Inbox,
            priority: Priority::Normal,
            due_at_ms: None,
            deadline_type: DeadlineType::None,
            scheduled: None,
            estimate_minutes: None,
            energy_level: None,
            context: None,
            tags: Vec::new(),
            project_id: None,
            created_at_ms: 1,
            updated_at_ms: 1,
            completed_at_ms: None,
            sort_order: 0,
        };
        let second = Task {
            id: "task-second".into(),
            title: "Second".into(),
            created_at_ms: 2,
            updated_at_ms: 2,
            sort_order: 1,
            ..first.clone()
        };
        let third = Task {
            id: "task-third".into(),
            title: "Third".into(),
            created_at_ms: 3,
            updated_at_ms: 3,
            sort_order: 2,
            ..first.clone()
        };

        store.upsert_task(first).expect("first should insert");
        store
            .upsert_task(second.clone())
            .expect("second should insert");
        store
            .upsert_task(third.clone())
            .expect("third should insert");
        store
            .reorder_tasks(&[
                "task-third".into(),
                "task-first".into(),
                "task-second".into(),
            ])
            .expect("tasks should reorder");

        assert_eq!(
            store
                .list_tasks_by_status(TaskStatus::Inbox)
                .expect("tasks should load")
                .into_iter()
                .map(|task| task.id)
                .collect::<Vec<_>>(),
            vec!["task-third", "task-first", "task-second"]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_persists_external_links_after_reopen() {
        let path = temp_store_path("external-link-persist");
        let task = test_task("task-linked");
        let link = tasknotes_link(
            "link-1",
            &task.id,
            "tasknotes-1",
            "Tasks/write-sync-tests.md",
        );

        {
            let mut store = SqliteTaskStore::open(&path).expect("store should open");
            store.upsert_task(task.clone()).expect("task should insert");
            store
                .upsert_external_link(link.clone())
                .expect("link should insert");
        }

        let store = SqliteTaskStore::open(&path).expect("store should reopen");
        assert_eq!(
            store
                .get_external_link_by_external_id(&ExternalProvider::TaskNotes, "tasknotes-1")
                .expect("link should load"),
            Some(link.clone())
        );
        assert_eq!(
            store
                .get_external_link_by_external_path(
                    &ExternalProvider::TaskNotes,
                    "Tasks/write-sync-tests.md"
                )
                .expect("link should load"),
            Some(link.clone())
        );
        assert_eq!(
            store
                .list_external_links_for_task(&task.id)
                .expect("links should load"),
            vec![link]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_upserts_existing_external_link_without_duplicate() {
        let path = temp_store_path("external-link-upsert");
        let task = test_task("task-linked");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        store.upsert_task(task.clone()).expect("task should insert");

        let mut link = tasknotes_link("link-1", &task.id, "tasknotes-1", "Tasks/original.md");
        store
            .upsert_external_link(link.clone())
            .expect("link should insert");

        link.external_path = Some("Archive/original.md".into());
        link.sync_hash = Some("hash-2".into());
        link.last_synced_at_ms = Some(200);
        store
            .upsert_external_link(link.clone())
            .expect("link should update");

        assert_eq!(
            store
                .list_external_links_for_task(&task.id)
                .expect("links should load"),
            vec![link]
        );
        assert_eq!(
            store
                .list_external_links()
                .expect("all links should load")
                .len(),
            1
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_rejects_duplicate_tasknotes_external_id() {
        let path = temp_store_path("external-link-duplicate-id");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let first_task = test_task("task-first");
        let second_task = test_task("task-second");
        store
            .upsert_task(first_task.clone())
            .expect("first task should insert");
        store
            .upsert_task(second_task.clone())
            .expect("second task should insert");

        let first_link = tasknotes_link("link-1", &first_task.id, "shared-id", "Tasks/one.md");
        let second_link = tasknotes_link("link-2", &second_task.id, "shared-id", "Tasks/two.md");
        store
            .upsert_external_link(first_link)
            .expect("first link should insert");

        assert!(matches!(
            store.upsert_external_link(second_link),
            Err(StoreError::Backend(_))
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_rejects_duplicate_tasknotes_external_path() {
        let path = temp_store_path("external-link-duplicate-path");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let first_task = test_task("task-first");
        let second_task = test_task("task-second");
        store
            .upsert_task(first_task.clone())
            .expect("first task should insert");
        store
            .upsert_task(second_task.clone())
            .expect("second task should insert");

        let first_link = tasknotes_link("link-1", &first_task.id, "first-id", "Tasks/shared.md");
        let second_link = tasknotes_link("link-2", &second_task.id, "second-id", "Tasks/shared.md");
        store
            .upsert_external_link(first_link)
            .expect("first link should insert");

        assert!(matches!(
            store.upsert_external_link(second_link),
            Err(StoreError::Backend(_))
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn sqlite_store_cascades_external_links_when_task_is_deleted() {
        let path = temp_store_path("external-link-cascade");
        let task = test_task("task-linked");
        let link = tasknotes_link("link-1", &task.id, "tasknotes-1", "Tasks/delete-me.md");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        store.upsert_task(task.clone()).expect("task should insert");
        store
            .upsert_external_link(link.clone())
            .expect("link should insert");

        store.delete_task(&task.id).expect("task should be deleted");

        assert_eq!(
            store
                .get_external_link(&link.id)
                .expect("link lookup should succeed"),
            None
        );

        let _ = fs::remove_file(path);
    }
}
