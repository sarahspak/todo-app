//! TaskNotes integration boundary.

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use task_core::{DeadlineType, Priority, Task, TaskStatus};
use task_store::{ExternalLinkStore, StoreError, TaskStore};
use task_sync::{
    match_remote_identity, ExternalLink, ExternalProvider, IdentityMatch, RemoteTaskIdentity,
    SyncError, SyncMode, SyncPreview, SyncState, TaskSyncProvider,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNotesDocument {
    pub external_id: Option<String>,
    pub todo_app_id: Option<String>,
    pub title: String,
    pub body: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNotesScannedDocument {
    pub relative_path: String,
    pub document: TaskNotesDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskNotesImportSummary {
    pub created: usize,
    pub updated: usize,
    pub linked: usize,
    pub conflicts: Vec<String>,
}

impl TaskNotesProvider {
    pub fn new(config: TaskNotesConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &TaskNotesConfig {
        &self.config
    }
}

pub fn scan_tasknotes_documents(
    vault_path: impl AsRef<Path>,
) -> Result<Vec<TaskNotesScannedDocument>, SyncError> {
    let vault_path = vault_path.as_ref();
    let mut documents = Vec::new();
    scan_markdown_files(vault_path, vault_path, &mut documents)?;
    documents.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(documents)
}

pub fn import_tasknotes_documents<S>(
    store: &mut S,
    documents: &[TaskNotesScannedDocument],
    now_ms: i64,
) -> Result<TaskNotesImportSummary, StoreError>
where
    S: TaskStore + ExternalLinkStore,
{
    let mut summary = TaskNotesImportSummary {
        created: 0,
        updated: 0,
        linked: 0,
        conflicts: Vec::new(),
    };

    for scanned in documents {
        let local_tasks = store.list_tasks()?;
        let links = store.list_external_links()?;
        let identity = scanned
            .document
            .remote_identity(scanned.relative_path.clone());

        match match_remote_identity(&identity, &links, &local_tasks) {
            IdentityMatch::Linked(task_id) => {
                let Some(mut task) = store.get_task(&task_id)? else {
                    summary.conflicts.push(format!(
                        "link points to missing local task {task_id} for {}",
                        scanned.relative_path
                    ));
                    continue;
                };
                apply_document_to_task(&mut task, &scanned.document, now_ms);
                store.upsert_task(task.clone())?;
                store.upsert_external_link(link_for_document(&task.id, scanned, now_ms))?;
                summary.updated += 1;
            }
            IdentityMatch::NewRemote => {
                let task = task_from_document(scanned, now_ms);
                store.upsert_task(task.clone())?;
                store.upsert_external_link(link_for_document(&task.id, scanned, now_ms))?;
                summary.created += 1;
            }
            IdentityMatch::PossibleTitleMatch(_) => {
                summary.conflicts.push(format!(
                    "possible title-only match for {}; refusing automatic link",
                    scanned.relative_path
                ));
            }
            IdentityMatch::Conflict(message) => summary
                .conflicts
                .push(format!("{}: {message}", scanned.relative_path)),
        }
    }

    Ok(summary)
}

impl TaskNotesDocument {
    pub fn remote_identity(&self, external_path: impl Into<String>) -> RemoteTaskIdentity {
        RemoteTaskIdentity {
            provider: ExternalProvider::TaskNotes,
            external_id: self.external_id.clone(),
            external_path: Some(external_path.into()),
            todo_app_id: self.todo_app_id.clone(),
            title: self.title.clone(),
            content_hash: self.content_hash.clone(),
        }
    }
}

pub fn parse_tasknotes_document(
    relative_path: impl AsRef<Path>,
    contents: &str,
) -> TaskNotesDocument {
    let (frontmatter, body) = split_frontmatter(contents);
    let external_id =
        frontmatter.and_then(|fields| first_field(fields, &["id", "task_id", "taskId"]));
    let todo_app_id =
        frontmatter.and_then(|fields| first_field(fields, &["todo_app_id", "todoAppId"]));
    let title = frontmatter
        .and_then(|fields| first_field(fields, &["title", "name"]))
        .or_else(|| first_markdown_heading(body))
        .unwrap_or_else(|| title_from_path(relative_path.as_ref()));

    TaskNotesDocument {
        external_id,
        todo_app_id,
        title,
        body: body.trim().to_string(),
        content_hash: stable_content_hash(contents),
    }
}

fn split_frontmatter(contents: &str) -> (Option<&str>, &str) {
    let Some(rest) = contents.strip_prefix("---\n") else {
        return (None, contents);
    };
    let Some(end_index) = rest.find("\n---\n") else {
        return (None, contents);
    };
    let frontmatter = &rest[..end_index];
    let body = &rest[end_index + "\n---\n".len()..];
    (Some(frontmatter), body)
}

fn first_field(frontmatter: &str, keys: &[&str]) -> Option<String> {
    frontmatter.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        if keys.iter().any(|candidate| key.trim() == *candidate) {
            clean_scalar(value)
        } else {
            None
        }
    })
}

fn clean_scalar(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() || trimmed.starts_with('[') || trimmed.starts_with('{') {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn first_markdown_heading(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let heading = line.strip_prefix("# ")?;
        let heading = heading.trim();
        if heading.is_empty() {
            None
        } else {
            Some(heading.to_string())
        }
    })
}

fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled task")
        .to_string()
}

fn stable_content_hash(contents: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in contents.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn scan_markdown_files(
    vault_path: &Path,
    directory: &Path,
    documents: &mut Vec<TaskNotesScannedDocument>,
) -> Result<(), SyncError> {
    let entries = fs::read_dir(directory)
        .map_err(|error| SyncError::Provider(format!("failed to read vault directory: {error}")))?;

    for entry in entries {
        let entry = entry
            .map_err(|error| SyncError::Provider(format!("failed to read vault entry: {error}")))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            SyncError::Provider(format!("failed to inspect vault entry type: {error}"))
        })?;

        if file_type.is_dir() {
            if entry.file_name() == ".obsidian" {
                continue;
            }
            scan_markdown_files(vault_path, &path, documents)?;
            continue;
        }

        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            let contents = fs::read_to_string(&path).map_err(|error| {
                SyncError::Provider(format!("failed to read markdown file {:?}: {error}", path))
            })?;
            let relative_path = relative_markdown_path(vault_path, &path);
            let document = parse_tasknotes_document(&relative_path, &contents);
            documents.push(TaskNotesScannedDocument {
                relative_path,
                document,
            });
        }
    }

    Ok(())
}

fn relative_markdown_path(vault_path: &Path, path: &Path) -> String {
    path.strip_prefix(vault_path)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn task_from_document(scanned: &TaskNotesScannedDocument, now_ms: i64) -> Task {
    let id = scanned
        .document
        .todo_app_id
        .clone()
        .unwrap_or_else(|| generated_task_id(scanned));
    let mut task = Task {
        id,
        title: String::new(),
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
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        completed_at_ms: None,
        sort_order: now_ms,
    };
    apply_document_to_task(&mut task, &scanned.document, now_ms);
    task
}

fn apply_document_to_task(task: &mut Task, document: &TaskNotesDocument, now_ms: i64) {
    task.title = document.title.clone();
    task.notes = if document.body.is_empty() {
        None
    } else {
        Some(document.body.clone())
    };
    task.updated_at_ms = now_ms;
}

fn link_for_document(
    task_id: &str,
    scanned: &TaskNotesScannedDocument,
    now_ms: i64,
) -> ExternalLink {
    ExternalLink {
        id: format!("tasknotes:{task_id}"),
        task_id: task_id.to_string(),
        provider: ExternalProvider::TaskNotes,
        external_id: scanned.document.external_id.clone(),
        external_path: Some(scanned.relative_path.clone()),
        last_synced_at_ms: Some(now_ms),
        sync_hash: Some(scanned.document.content_hash.clone()),
        sync_state: SyncState::Linked,
    }
}

fn generated_task_id(scanned: &TaskNotesScannedDocument) -> String {
    if let Some(external_id) = &scanned.document.external_id {
        format!("tasknotes-id-{external_id}")
    } else {
        format!(
            "tasknotes-path-{}",
            stable_content_hash(&scanned.relative_path)
        )
    }
}

impl TaskSyncProvider for TaskNotesProvider {
    fn mode(&self) -> SyncMode {
        self.config.mode.clone()
    }

    fn preview(&self) -> Result<SyncPreview, SyncError> {
        if self.config.mode == SyncMode::Off {
            return Err(SyncError::Disabled);
        }

        Ok(SyncPreview {
            creates: self.import_tasks()?,
            updates: Vec::new(),
            conflicts: Vec::new(),
        })
    }

    fn import_tasks(&self) -> Result<Vec<Task>, SyncError> {
        if self.config.mode == SyncMode::Off {
            return Err(SyncError::Disabled);
        }

        let now_ms = now_ms()?;
        scan_tasknotes_documents(&self.config.vault_path).map(|documents| {
            documents
                .iter()
                .map(|document| task_from_document(document, now_ms))
                .collect()
        })
    }

    fn push_tasks(&self, _tasks: &[Task]) -> Result<(), SyncError> {
        match self.config.mode {
            SyncMode::TwoWay => Ok(()),
            SyncMode::Off | SyncMode::ImportOnly => Err(SyncError::Disabled),
        }
    }
}

fn now_ms() -> Result<i64, SyncError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|error| {
        SyncError::Provider(format!("system clock is before Unix epoch: {error}"))
    })?;
    Ok(duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    use task_core::{DeadlineType, Priority, TaskStatus};
    use task_store::{ExternalLinkStore, SqliteTaskStore, TaskStore};
    use task_sync::{match_remote_identity, ExternalLink, IdentityMatch, SyncState};

    #[test]
    fn parses_tasknotes_identity_from_frontmatter() {
        let document = parse_tasknotes_document(
            "Tasks/write-sync-tests.md",
            r#"---
id: tasknotes-1
todo_app_id: task-1
title: Write sync tests
status: TODO
---

Body text.
"#,
        );

        assert_eq!(document.external_id, Some("tasknotes-1".into()));
        assert_eq!(document.todo_app_id, Some("task-1".into()));
        assert_eq!(document.title, "Write sync tests");
        assert_eq!(document.body, "Body text.");
        assert_eq!(
            document.remote_identity("Tasks/write-sync-tests.md"),
            RemoteTaskIdentity {
                provider: ExternalProvider::TaskNotes,
                external_id: Some("tasknotes-1".into()),
                external_path: Some("Tasks/write-sync-tests.md".into()),
                todo_app_id: Some("task-1".into()),
                title: "Write sync tests".into(),
                content_hash: document.content_hash.clone(),
            }
        );
    }

    #[test]
    fn falls_back_to_heading_then_file_stem_for_title() {
        let heading_document =
            parse_tasknotes_document("Tasks/ignored.md", "# Heading title\n\nBody text.");
        let stem_document = parse_tasknotes_document("Tasks/file-stem-title.md", "Body text.");

        assert_eq!(heading_document.title, "Heading title");
        assert_eq!(stem_document.title, "file-stem-title");
    }

    #[test]
    fn parsed_identity_links_by_app_id_without_creating_duplicate() {
        let local_task = task("task-1", "Old title");
        let document = parse_tasknotes_document(
            "Tasks/write-sync-tests.md",
            r#"---
id: tasknotes-1
todo_app_id: task-1
title: New title
---
"#,
        );

        assert_eq!(
            match_remote_identity(
                &document.remote_identity("Tasks/write-sync-tests.md"),
                &[],
                &[local_task]
            ),
            IdentityMatch::Linked("task-1".into())
        );
    }

    #[test]
    fn parsed_identity_links_by_external_id_when_note_moves() {
        let local_task = task("task-1", "Write sync tests");
        let existing_link = ExternalLink {
            id: "link-1".into(),
            task_id: "task-1".into(),
            provider: ExternalProvider::TaskNotes,
            external_id: Some("tasknotes-1".into()),
            external_path: Some("Tasks/original.md".into()),
            last_synced_at_ms: Some(1),
            sync_hash: Some("hash".into()),
            sync_state: SyncState::Linked,
        };
        let document = parse_tasknotes_document(
            "Archive/original.md",
            r#"---
id: tasknotes-1
title: Write sync tests
---
"#,
        );

        assert_eq!(
            match_remote_identity(
                &document.remote_identity("Archive/original.md"),
                &[existing_link],
                &[local_task]
            ),
            IdentityMatch::Linked("task-1".into())
        );
    }

    #[test]
    fn import_documents_is_idempotent_for_existing_tasknotes_link() {
        let path = temp_store_path("tasknotes-import-idempotent");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let document = scanned_document(
            "Tasks/write-sync-tests.md",
            r#"---
id: tasknotes-1
title: Write sync tests
---

Body text.
"#,
        );

        let first_summary = import_tasknotes_documents(&mut store, &[document.clone()], 100)
            .expect("first import should succeed");
        let second_summary = import_tasknotes_documents(&mut store, &[document], 200)
            .expect("second import should succeed");

        assert_eq!(
            first_summary,
            TaskNotesImportSummary {
                created: 1,
                updated: 0,
                linked: 0,
                conflicts: Vec::new(),
            }
        );
        assert_eq!(
            second_summary,
            TaskNotesImportSummary {
                created: 0,
                updated: 1,
                linked: 0,
                conflicts: Vec::new(),
            }
        );
        assert_eq!(store.list_tasks().expect("tasks should load").len(), 1);
        assert_eq!(
            store
                .list_external_links()
                .expect("links should load")
                .len(),
            1
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn import_documents_updates_link_path_when_tasknotes_id_moves() {
        let path = temp_store_path("tasknotes-import-move");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let original = scanned_document(
            "Tasks/original.md",
            r#"---
id: tasknotes-1
title: Write sync tests
---
"#,
        );
        let moved = scanned_document(
            "Archive/original.md",
            r#"---
id: tasknotes-1
title: Write sync tests
---
"#,
        );

        import_tasknotes_documents(&mut store, &[original], 100)
            .expect("first import should succeed");
        import_tasknotes_documents(&mut store, &[moved], 200)
            .expect("second import should succeed");

        let links = store.list_external_links().expect("links should load");
        assert_eq!(store.list_tasks().expect("tasks should load").len(), 1);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].external_path, Some("Archive/original.md".into()));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn import_documents_refuses_title_only_duplicate() {
        let path = temp_store_path("tasknotes-import-title-only");
        let mut store = SqliteTaskStore::open(&path).expect("store should open");
        let local_task = task("task-1", "Write sync tests");
        store
            .upsert_task(local_task)
            .expect("local task should insert");
        let document = scanned_document(
            "Tasks/unlinked.md",
            r#"---
title: Write sync tests
---
"#,
        );

        let summary = import_tasknotes_documents(&mut store, &[document], 100)
            .expect("import should complete with conflict");

        assert_eq!(summary.created, 0);
        assert_eq!(summary.updated, 0);
        assert_eq!(summary.conflicts.len(), 1);
        assert_eq!(store.list_tasks().expect("tasks should load").len(), 1);
        assert_eq!(
            store
                .list_external_links()
                .expect("links should load")
                .len(),
            0
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn scans_markdown_documents_from_vault() {
        let vault = temp_vault_path("tasknotes-scan");
        fs::create_dir_all(vault.join(".obsidian")).expect("obsidian dir should create");
        fs::create_dir_all(vault.join("Tasks")).expect("tasks dir should create");
        fs::write(
            vault.join("Tasks/write-sync-tests.md"),
            "---\nid: tasknotes-1\ntitle: Write sync tests\n---\n",
        )
        .expect("task file should write");
        fs::write(vault.join(".obsidian/ignored.md"), "# Ignored")
            .expect("ignored file should write");

        let documents = scan_tasknotes_documents(&vault).expect("vault should scan");

        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].relative_path, "Tasks/write-sync-tests.md");
        assert_eq!(
            documents[0].document.external_id,
            Some("tasknotes-1".into())
        );

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn provider_import_tasks_reads_scanned_vault_documents() {
        let vault = temp_vault_path("tasknotes-provider-import");
        fs::create_dir_all(vault.join("Tasks")).expect("tasks dir should create");
        fs::write(
            vault.join("Tasks/write-sync-tests.md"),
            "---\nid: tasknotes-1\ntitle: Write sync tests\n---\n\nBody text.",
        )
        .expect("task file should write");
        let provider = TaskNotesProvider::new(TaskNotesConfig {
            vault_path: vault.to_string_lossy().to_string(),
            tasks_glob: "**/*.md".into(),
            mode: SyncMode::TwoWay,
        });

        let tasks = provider
            .import_tasks()
            .expect("provider import should load tasks");
        let preview = provider.preview().expect("preview should load tasks");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "tasknotes-id-tasknotes-1");
        assert_eq!(tasks[0].title, "Write sync tests");
        assert_eq!(tasks[0].notes, Some("Body text.".into()));
        assert_eq!(preview.creates.len(), 1);

        let _ = fs::remove_dir_all(vault);
    }

    #[test]
    fn provider_import_tasks_is_disabled_when_mode_is_off() {
        let provider = TaskNotesProvider::new(TaskNotesConfig {
            vault_path: "/tmp/unused".into(),
            tasks_glob: "**/*.md".into(),
            mode: SyncMode::Off,
        });

        assert_eq!(provider.import_tasks(), Err(SyncError::Disabled));
        assert_eq!(provider.preview(), Err(SyncError::Disabled));
    }

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

    fn scanned_document(relative_path: &str, contents: &str) -> TaskNotesScannedDocument {
        TaskNotesScannedDocument {
            relative_path: relative_path.into(),
            document: parse_tasknotes_document(relative_path, contents),
        }
    }

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

    fn temp_vault_path(label: &str) -> std::path::PathBuf {
        env::temp_dir().join(format!(
            "todo-app-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after Unix epoch")
                .as_nanos()
        ))
    }
}
