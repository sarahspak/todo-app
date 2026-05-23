//! Core task and scheduling types.

pub type TaskId = String;
pub type ProjectId = String;
pub type TimestampMs = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Inbox,
    Planned,
    InProgress,
    Done,
    Canceled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadlineType {
    None,
    Soft,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnergyLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeBlock {
    pub start_ms: TimestampMs,
    pub end_ms: TimestampMs,
}

impl TimeBlock {
    pub fn duration_minutes(&self) -> i64 {
        (self.end_ms - self.start_ms) / 60_000
    }

    pub fn overlaps(&self, other: &TimeBlock) -> bool {
        self.start_ms < other.end_ms && other.start_ms < self.end_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub notes: Option<String>,
    pub status: TaskStatus,
    pub priority: Priority,
    pub due_at_ms: Option<TimestampMs>,
    pub deadline_type: DeadlineType,
    pub scheduled: Option<TimeBlock>,
    pub estimate_minutes: Option<i64>,
    pub energy_level: Option<EnergyLevel>,
    pub context: Option<String>,
    pub tags: Vec<String>,
    pub project_id: Option<ProjectId>,
    pub created_at_ms: TimestampMs,
    pub updated_at_ms: TimestampMs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarBusyBlock {
    pub source: String,
    pub block: TimeBlock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRequest {
    pub tasks: Vec<Task>,
    pub busy: Vec<CalendarBusyBlock>,
    pub candidate_blocks: Vec<TimeBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub task_id: TaskId,
    pub block: TimeBlock,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulePlan {
    pub scheduled: Vec<ScheduledTask>,
    pub unscheduled_task_ids: Vec<TaskId>,
}

pub trait Scheduler {
    fn propose_schedule(&self, request: ScheduleRequest) -> SchedulePlan;
}

#[derive(Debug, Default, Clone)]
pub struct GreedyScheduler;

impl Scheduler for GreedyScheduler {
    fn propose_schedule(&self, mut request: ScheduleRequest) -> SchedulePlan {
        request.tasks.sort_by(|a, b| {
            b.priority
                .priority_rank()
                .cmp(&a.priority.priority_rank())
                .then_with(|| a.due_at_ms.cmp(&b.due_at_ms))
                .then_with(|| a.created_at_ms.cmp(&b.created_at_ms))
        });

        let mut scheduled = Vec::new();
        let mut occupied: Vec<TimeBlock> = request.busy.into_iter().map(|b| b.block).collect();
        let mut unscheduled_task_ids = Vec::new();

        for task in request.tasks {
            let estimate = task.estimate_minutes.unwrap_or(30).max(5);
            let placement = request
                .candidate_blocks
                .iter()
                .find_map(|candidate| first_available_slot(candidate, &occupied, estimate));

            match placement {
                Some(block) => {
                    occupied.push(block.clone());
                    scheduled.push(ScheduledTask {
                        task_id: task.id,
                        block,
                        reason: "Fits the earliest available block using priority order.".into(),
                    });
                }
                None => unscheduled_task_ids.push(task.id),
            }
        }

        SchedulePlan {
            scheduled,
            unscheduled_task_ids,
        }
    }
}

impl Priority {
    fn priority_rank(&self) -> u8 {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
            Priority::Urgent => 3,
        }
    }
}

fn first_available_slot(
    candidate: &TimeBlock,
    occupied: &[TimeBlock],
    estimate_minutes: i64,
) -> Option<TimeBlock> {
    let duration_ms = estimate_minutes * 60_000;
    let mut cursor = candidate.start_ms;
    let mut overlaps: Vec<&TimeBlock> = occupied
        .iter()
        .filter(|block| block.overlaps(candidate))
        .collect();
    overlaps.sort_by_key(|block| block.start_ms);

    for block in overlaps {
        if cursor + duration_ms <= block.start_ms {
            return Some(TimeBlock {
                start_ms: cursor,
                end_ms: cursor + duration_ms,
            });
        }
        cursor = cursor.max(block.end_ms);
    }

    if cursor + duration_ms <= candidate.end_ms {
        Some(TimeBlock {
            start_ms: cursor,
            end_ms: cursor + duration_ms,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greedy_scheduler_uses_free_time_around_busy_blocks() {
        let scheduler = GreedyScheduler;
        let plan = scheduler.propose_schedule(ScheduleRequest {
            tasks: vec![Task {
                id: "task-1".into(),
                title: "Write plan".into(),
                notes: None,
                status: TaskStatus::Inbox,
                priority: Priority::High,
                due_at_ms: None,
                deadline_type: DeadlineType::None,
                scheduled: None,
                estimate_minutes: Some(30),
                energy_level: None,
                context: None,
                tags: vec![],
                project_id: None,
                created_at_ms: 0,
                updated_at_ms: 0,
            }],
            busy: vec![CalendarBusyBlock {
                source: "calendar".into(),
                block: TimeBlock {
                    start_ms: 30 * 60_000,
                    end_ms: 60 * 60_000,
                },
            }],
            candidate_blocks: vec![TimeBlock {
                start_ms: 0,
                end_ms: 120 * 60_000,
            }],
        });

        assert_eq!(plan.scheduled.len(), 1);
        assert_eq!(plan.scheduled[0].block.start_ms, 0);
        assert!(plan.unscheduled_task_ids.is_empty());
    }
}
