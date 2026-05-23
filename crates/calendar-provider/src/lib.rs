//! Calendar provider contracts.

use task_core::{CalendarBusyBlock, ScheduledTask, TimeBlock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarProviderKind {
    AppleEventKit,
    Google,
    CalDav,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarError {
    PermissionDenied,
    Provider(String),
}

pub trait CalendarProvider {
    fn kind(&self) -> CalendarProviderKind;
    fn list_busy_blocks(&self, window: TimeBlock) -> Result<Vec<CalendarBusyBlock>, CalendarError>;
    fn create_task_event(&self, scheduled: &ScheduledTask) -> Result<String, CalendarError>;
}
