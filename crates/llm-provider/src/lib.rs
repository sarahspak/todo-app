//! LLM provider contracts for onboarding and scheduling suggestions.

use task_core::{SchedulePlan, Task};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProviderKind {
    OpenAi,
    Anthropic,
    Ollama,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmProviderConfig {
    pub id: String,
    pub kind: LlmProviderKind,
    pub model: String,
    pub base_url: Option<String>,
    pub auth_storage_ref: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    Disabled,
    MissingCredentials,
    InvalidStructuredOutput(String),
    Provider(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCaptureRequest {
    pub input: String,
    pub now_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleExplanationRequest {
    pub tasks: Vec<Task>,
    pub plan: SchedulePlan,
}

pub trait LlmProvider {
    fn config(&self) -> &LlmProviderConfig;
    fn capture_tasks(&self, request: TaskCaptureRequest) -> Result<Vec<Task>, LlmError>;
    fn explain_schedule(&self, request: ScheduleExplanationRequest) -> Result<String, LlmError>;
}

#[derive(Debug, Clone)]
pub struct ProviderStub {
    config: LlmProviderConfig,
}

impl ProviderStub {
    pub fn new(config: LlmProviderConfig) -> Self {
        Self { config }
    }
}

impl LlmProvider for ProviderStub {
    fn config(&self) -> &LlmProviderConfig {
        &self.config
    }

    fn capture_tasks(&self, _request: TaskCaptureRequest) -> Result<Vec<Task>, LlmError> {
        if !self.config.enabled {
            return Err(LlmError::Disabled);
        }

        Ok(Vec::new())
    }

    fn explain_schedule(&self, _request: ScheduleExplanationRequest) -> Result<String, LlmError> {
        if !self.config.enabled {
            return Err(LlmError::Disabled);
        }

        Ok("This schedule is a provider stub and has not called an LLM.".into())
    }
}
