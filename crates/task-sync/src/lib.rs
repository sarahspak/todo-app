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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTaskIdentity {
    pub provider: ExternalProvider,
    pub external_id: Option<String>,
    pub external_path: Option<String>,
    pub todo_app_id: Option<TaskId>,
    pub title: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityMatch {
    Linked(TaskId),
    PossibleTitleMatch(TaskId),
    NewRemote,
    Conflict(String),
}

pub fn match_remote_identity(
    remote: &RemoteTaskIdentity,
    links: &[ExternalLink],
    local_tasks: &[Task],
) -> IdentityMatch {
    if let Some(todo_app_id) = &remote.todo_app_id {
        let matching_tasks = local_tasks
            .iter()
            .filter(|task| &task.id == todo_app_id)
            .count();
        if matching_tasks == 1 {
            return IdentityMatch::Linked(todo_app_id.clone());
        }
        if matching_tasks > 1 {
            return IdentityMatch::Conflict(format!(
                "multiple local tasks use todo_app_id {todo_app_id}"
            ));
        }
    }

    if let Some(external_id) = &remote.external_id {
        let matches: Vec<&ExternalLink> = links
            .iter()
            .filter(|link| {
                link.provider == remote.provider && link.external_id.as_ref() == Some(external_id)
            })
            .collect();
        match matches.as_slice() {
            [link] => return IdentityMatch::Linked(link.task_id.clone()),
            [] => {}
            _ => {
                return IdentityMatch::Conflict(format!(
                    "multiple links use external_id {external_id}"
                ))
            }
        }
    }

    if let Some(external_path) = &remote.external_path {
        let matches: Vec<&ExternalLink> = links
            .iter()
            .filter(|link| {
                link.provider == remote.provider
                    && link.external_path.as_ref() == Some(external_path)
            })
            .collect();
        match matches.as_slice() {
            [link] => return IdentityMatch::Linked(link.task_id.clone()),
            [] => {}
            _ => {
                return IdentityMatch::Conflict(format!(
                    "multiple links use external_path {external_path}"
                ))
            }
        }
    }

    let title_matches: Vec<&Task> = local_tasks
        .iter()
        .filter(|task| task.title == remote.title)
        .collect();
    match title_matches.as_slice() {
        [task] => IdentityMatch::PossibleTitleMatch(task.id.clone()),
        [] => IdentityMatch::NewRemote,
        _ => IdentityMatch::Conflict(format!("multiple local tasks have title {}", remote.title)),
    }
}

pub trait TaskSyncProvider {
    fn mode(&self) -> SyncMode;
    fn preview(&self) -> Result<SyncPreview, SyncError>;
    fn import_tasks(&self) -> Result<Vec<Task>, SyncError>;
    fn push_tasks(&self, tasks: &[Task]) -> Result<(), SyncError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use task_core::{DeadlineType, Priority, TaskStatus};

    fn task(id: &str, title: &str) -> Task {
        Task {
            id: id.into(),
            title: title.into(),
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

    fn link(task_id: &str, external_id: Option<&str>, external_path: Option<&str>) -> ExternalLink {
        ExternalLink {
            id: format!("link-{task_id}"),
            task_id: task_id.into(),
            provider: ExternalProvider::TaskNotes,
            external_id: external_id.map(str::to_string),
            external_path: external_path.map(str::to_string),
            last_synced_at_ms: Some(1),
            sync_hash: Some("hash".into()),
            sync_state: SyncState::Linked,
        }
    }

    fn remote(
        external_id: Option<&str>,
        external_path: Option<&str>,
        todo_app_id: Option<&str>,
        title: &str,
    ) -> RemoteTaskIdentity {
        RemoteTaskIdentity {
            provider: ExternalProvider::TaskNotes,
            external_id: external_id.map(str::to_string),
            external_path: external_path.map(str::to_string),
            todo_app_id: todo_app_id.map(str::to_string),
            title: title.into(),
            content_hash: "hash".into(),
        }
    }

    #[test]
    fn app_id_matches_existing_local_task_first() {
        let tasks = vec![task("task-1", "Write sync tests")];
        let links = vec![link(
            "task-2",
            Some("different-remote-id"),
            Some("Tasks/different.md"),
        )];

        assert_eq!(
            match_remote_identity(
                &remote(
                    Some("remote-1"),
                    Some("Tasks/task.md"),
                    Some("task-1"),
                    "Other"
                ),
                &links,
                &tasks,
            ),
            IdentityMatch::Linked("task-1".into())
        );
    }

    #[test]
    fn external_id_matches_existing_link_when_path_changes() {
        let tasks = vec![task("task-1", "Write sync tests")];
        let links = vec![link(
            "task-1",
            Some("tasknotes-1"),
            Some("Tasks/original.md"),
        )];

        assert_eq!(
            match_remote_identity(
                &remote(
                    Some("tasknotes-1"),
                    Some("Archive/original.md"),
                    None,
                    "Write sync tests"
                ),
                &links,
                &tasks,
            ),
            IdentityMatch::Linked("task-1".into())
        );
    }

    #[test]
    fn external_path_matches_when_stable_id_is_missing() {
        let tasks = vec![task("task-1", "Write sync tests")];
        let links = vec![link("task-1", None, Some("Tasks/original.md"))];

        assert_eq!(
            match_remote_identity(
                &remote(None, Some("Tasks/original.md"), None, "Renamed task"),
                &links,
                &tasks,
            ),
            IdentityMatch::Linked("task-1".into())
        );
    }

    #[test]
    fn title_only_match_is_possible_but_not_linked() {
        let tasks = vec![task("task-1", "Write sync tests")];
        let links = Vec::new();

        assert_eq!(
            match_remote_identity(
                &remote(None, None, None, "Write sync tests"),
                &links,
                &tasks,
            ),
            IdentityMatch::PossibleTitleMatch("task-1".into())
        );
    }

    #[test]
    fn duplicate_external_id_is_a_conflict() {
        let tasks = vec![task("task-1", "One"), task("task-2", "Two")];
        let links = vec![
            link("task-1", Some("shared-id"), Some("Tasks/one.md")),
            link("task-2", Some("shared-id"), Some("Tasks/two.md")),
        ];

        assert!(matches!(
            match_remote_identity(
                &remote(Some("shared-id"), Some("Tasks/three.md"), None, "Three"),
                &links,
                &tasks,
            ),
            IdentityMatch::Conflict(_)
        ));
    }
}
