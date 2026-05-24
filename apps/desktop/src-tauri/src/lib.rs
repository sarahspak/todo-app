use std::{
    fs,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use task_core::{DeadlineType, Priority, Task, TaskStatus};
use task_store::{SqliteTaskStore, TaskStore};
use tauri::Manager;

#[derive(Debug)]
struct AppState {
    store: Mutex<SqliteTaskStore>,
    next_task_id: AtomicU64,
}

impl AppState {
    fn new(store: SqliteTaskStore) -> Self {
        Self {
            store: Mutex::new(store),
            next_task_id: AtomicU64::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateTaskInput {
    title: String,
    notes: Option<String>,
    priority: Option<Priority>,
    due_at_ms: Option<i64>,
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListTasksInput {
    status: Option<TaskStatus>,
}

#[derive(Debug, Deserialize)]
struct UpdateTaskInput {
    id: String,
    title: Option<String>,
    notes: Option<Option<String>>,
    priority: Option<Priority>,
    due_at_ms: Option<Option<i64>>,
    project_id: Option<Option<String>>,
}

#[derive(Debug, Deserialize)]
struct ReorderTasksInput {
    task_ids: Vec<String>,
}

#[tauri::command]
fn app_status() -> &'static str {
    "ready"
}

#[tauri::command]
fn create_task(input: CreateTaskInput, state: tauri::State<'_, AppState>) -> Result<Task, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Task title is required.".into());
    }

    let now_ms = now_ms()?;
    let task_id = state.next_task_id.fetch_add(1, Ordering::Relaxed);
    let task = Task {
        id: format!("task-{now_ms}-{task_id}"),
        title: title.into(),
        notes: input
            .notes
            .and_then(|notes| non_empty_string(notes.trim().to_string())),
        status: TaskStatus::Inbox,
        priority: input.priority.unwrap_or(Priority::Normal),
        due_at_ms: input.due_at_ms,
        deadline_type: DeadlineType::None,
        scheduled: None,
        estimate_minutes: None,
        energy_level: None,
        context: None,
        tags: Vec::new(),
        project_id: input.project_id,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        completed_at_ms: None,
        sort_order: now_ms,
    };

    let mut store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;
    store
        .upsert_task(task.clone())
        .map_err(|error| format!("{error:?}"))?;

    Ok(task)
}

#[tauri::command]
fn list_tasks(
    input: Option<ListTasksInput>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Task>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;

    match input.and_then(|input| input.status) {
        Some(status) => store
            .list_tasks_by_status(status)
            .map_err(|error| format!("{error:?}")),
        None => store.list_tasks().map_err(|error| format!("{error:?}")),
    }
}

#[tauri::command]
fn complete_task(id: String, state: tauri::State<'_, AppState>) -> Result<Task, String> {
    let now_ms = now_ms()?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;
    let mut task = store
        .get_task(&id)
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| format!("Task not found: {id}"))?;

    task.status = TaskStatus::Done;
    task.updated_at_ms = now_ms;
    task.completed_at_ms = Some(now_ms);
    store
        .upsert_task(task.clone())
        .map_err(|error| format!("{error:?}"))?;

    Ok(task)
}

#[tauri::command]
fn update_task(input: UpdateTaskInput, state: tauri::State<'_, AppState>) -> Result<Task, String> {
    let now_ms = now_ms()?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;
    let mut task = store
        .get_task(&input.id)
        .map_err(|error| format!("{error:?}"))?
        .ok_or_else(|| format!("Task not found: {}", input.id))?;

    if let Some(title) = input.title {
        let title = title.trim();
        if title.is_empty() {
            return Err("Task title is required.".into());
        }
        task.title = title.into();
    }

    if let Some(notes) = input.notes {
        task.notes = notes.and_then(|notes| non_empty_string(notes.trim().to_string()));
    }

    if let Some(priority) = input.priority {
        task.priority = priority;
    }

    if let Some(due_at_ms) = input.due_at_ms {
        task.due_at_ms = due_at_ms;
    }

    if let Some(project_id) = input.project_id {
        task.project_id = project_id;
    }

    task.updated_at_ms = now_ms;
    store
        .upsert_task(task.clone())
        .map_err(|error| format!("{error:?}"))?;

    Ok(task)
}

#[tauri::command]
fn delete_task(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;
    store.delete_task(&id).map_err(|error| format!("{error:?}"))
}

#[tauri::command]
fn reorder_tasks(
    input: ReorderTasksInput,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Task store lock is poisoned.".to_string())?;
    store
        .reorder_tasks(&input.task_ids)
        .map_err(|error| format!("{error:?}"))
}

fn non_empty_string(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn now_ms() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("System clock is before Unix epoch: {error}"))?;

    Ok(duration.as_millis() as i64)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            let store = SqliteTaskStore::open(app_data_dir.join("tasks.sqlite3"))
                .map_err(|error| format!("failed to open task database: {error:?}"))?;
            app.manage(AppState::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            create_task,
            list_tasks,
            complete_task,
            update_task,
            delete_task,
            reorder_tasks
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
