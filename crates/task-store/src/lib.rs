//! Storage traits for the local task database.

use std::collections::BTreeMap;

use task_core::{Task, TaskId};

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
        Ok(self.tasks.values().cloned().collect())
    }
}
