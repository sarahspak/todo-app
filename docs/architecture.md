# Todo App Architecture

## Product Direction

This app should work for three kinds of users:

- Users who do not use Obsidian TaskNotes.
- Users who use TaskNotes but do not want this app to sync with it.
- Users who use TaskNotes and want deep two-way sync.

That means the app should be app-first, with TaskNotes treated as an optional integration rather than the foundation of the data model.

The product principle is:

> The app owns planning and scheduling. TaskNotes is one possible storage and sync surface.

## Recommended Architecture

The core app should own its task database, scheduling state, user preferences, calendar links, and sync metadata.

TaskNotes, calendars, and other external systems should connect through integration providers.

```text
Core app database
  owns tasks, schedule state, calendar event IDs, preferences, history

Optional integrations
  TaskNotes sync
  calendar sync
  LLM providers
  import/export
```

This avoids forcing Obsidian concepts into every user's workflow while still allowing TaskNotes users to get deep integration.

## Data Model

The main task model should stay independent of any one external system.

```text
Task
  id
  title
  notes
  status
  priority
  due_at
  deadline_type
  scheduled_start
  scheduled_end
  estimate_minutes
  energy_level
  context
  tags
  project_id
  created_at
  updated_at

ExternalLink
  id
  task_id
  provider
  external_id
  external_path
  last_synced_at
  sync_hash
  sync_state

LLMProvider
  id
  provider
  base_url
  model
  auth_storage_ref
  enabled
```

`ExternalLink` records the relationship between an app task and an external object, such as a TaskNotes note or a calendar event. This keeps provider-specific details out of the main task model.

Examples:

- A TaskNotes task link can store the note path, frontmatter identifier, sync hash, and last synced timestamp.
- A calendar link can store the provider name and event ID.
- A task can have no external links and still be fully valid.
- An LLM provider can point to a hosted API, local model gateway, or custom OpenAI-compatible endpoint without changing the task model.

The first prototype schema should borrow heavily from the TaskNotes default task properties, while keeping app-owned IDs and scheduling metadata stable. TaskNotes property keys are configurable, so the app should store semantic fields internally and keep a per-vault field mapping for TaskNotes sync.

Initial app fields should include:

```text
Task
  id
  title
  body
  status
  priority
  due
  scheduled
  contexts
  projects
  tags
  time_estimate_minutes
  recurrence
  reminders
  blocked_by
  created_at
  modified_at
  completed_at
```

Fields to preserve during TaskNotes round-tripping:

```text
TaskNotes frontmatter
  unknown user fields
  configured custom fields
  timeEntries
  completeInstances
  archive tags
  plugin-specific calendar fields

Markdown body
  all user-authored content
```

Fields likely needed by this app but not assumed to exist in TaskNotes:

```text
App scheduling metadata
  schedule_locked
  schedule_source
  calendar_event_ids
  last_suggested_at
  llm_estimate_source
```

## Task Details Backend Gaps

The current backend already supports title and description editing through `update_task`; no additional command is needed for that part of the task details modal.

The backend also has task fields for priority, project, and due date. Those controls primarily need frontend wiring first: priority dropdown behavior, project selection, and a date picker that calls the existing update path.

The following task details controls need backend schema, persistence, and Tauri command support before they can do real work:

- Labels.
- Reminders.
- Subtasks.
- Comments.
- Attachments.
- Deadline.
- Location.

These should be added as first-class app-owned concepts, then mapped to TaskNotes or other providers through integration layers where possible.

## TaskNotes Sync Modes

TaskNotes should support at least three modes:

```text
Off
  No vault access.
  No TaskNotes reads or writes.

Import only
  Read TaskNotes tasks into the app.
  Do not write changes back to the vault.

Two-way sync
  Create and update app tasks from TaskNotes.
  Create and update TaskNotes notes from app tasks.
```

For an MVP, the safest initial sync behavior is:

```text
Default conflict policy: newest edit wins
Safety behavior: keep conflict snapshots for review or rollback
```

Later versions can add field-level merge, manual conflict review, or provider-specific rules.

Direct file access should be the first TaskNotes integration path. It keeps the app local-first, works when Obsidian is closed, supports bulk import/export, and matches TaskNotes' portable Markdown design.

The TaskNotes HTTP API may still be useful later:

- It can respect TaskNotes' own runtime behavior, validation, and configured defaults.
- It may reduce drift if TaskNotes adds behavior that is not obvious from frontmatter alone.
- It can let the app ask TaskNotes how the user's property mappings are configured instead of inferring them.
- It can avoid some file watcher edge cases while Obsidian is actively editing the same task.
- It may support commands or derived state that are not stored directly in task files.

The tradeoff is that HTTP API sync requires Obsidian and the plugin to be running, introduces connection/authentication setup, and makes the app less independently local-first. The app should not depend on it for MVP.

## Proposed Rust Workspace

```text
crates/
  task-core
    domain models
    scheduling engine
    prioritization

  task-store
    SQLite persistence
    migrations

  task-sync
    sync engine
    provider traits
    conflict detection

  tasknotes-provider
    markdown/frontmatter mapping
    vault watcher
    optional HTTP API client

  calendar-provider
    provider abstraction
    Apple EventKit / Google Calendar / CalDAV later

  llm-provider
    provider abstraction
    prompt orchestration
    structured output parsing
    local keychain integration

apps/
  desktop-tauri
  web
  ios
```

The app should use Rust for core logic, Tauri 2 for the macOS desktop shell, and a TypeScript frontend. Tauri 2 is stable and supports desktop and mobile targets, including macOS and iOS, while still allowing native Swift or platform-specific plugins where necessary.

The first production target should be macOS desktop. iOS should come later, after the scheduling model, sync model, and local database are stable.

## Scheduling Responsibilities

The scheduling engine should be part of the Rust core, not tied to any UI, calendar provider, TaskNotes provider, or LLM provider.

It should eventually support:

- Free/busy calendar awareness.
- Working hours.
- Task estimates.
- Deadlines.
- Priority.
- Energy level or focus requirements.
- Hard versus soft constraints.
- Rescheduling unfinished work.
- Detecting overload when tasks do not fit.
- Optional task splitting.

The calendar integration should be behind a provider interface, because Apple Calendar, Google Calendar, Microsoft, and CalDAV have different APIs and authorization models.

Google Calendar should be the first calendar integration. The macOS app can add Apple Calendar later for users who use it natively, but the first integration should match the primary source of truth for scheduling.

Calendar implementation order:

```text
1. Google Calendar read access.
2. Google Calendar write access.
3. Apple Calendar read/write access.
4. CalDAV later, if needed.
```

The first scheduling UX should be hybrid:

```text
User control
  The user can manually time-block tasks, pin tasks, edit estimates, and reject suggestions.

Automatic suggestions
  The app proposes schedule changes using deterministic scheduling logic, optionally assisted by an LLM.
```

LLMs should assist with interpretation, prioritization, summarization, task decomposition, and explanation. The deterministic scheduler should remain responsible for calendar math, conflict detection, and final schedule validation.

## LLM Integration

The app should allow users to bring their own LLM provider and API keys. No single LLM vendor should be required.

Supported provider types should include:

- OpenAI.
- Anthropic.
- Ollama.
- OpenAI-compatible HTTP APIs later.
- Other hosted providers with custom adapters later.

API keys and credentials should be stored in the platform credential store, not in the SQLite database. On macOS, that means Keychain.

LLM features should be optional. The app should still work as a task manager and scheduler without any LLM configured.

Initial LLM-assisted features could include:

- Onboarding questions that infer work hours, planning preferences, and scheduling style.
- Turning messy user input into structured tasks.
- Estimating task duration.
- Suggesting priority.
- Breaking large tasks into smaller tasks.
- Explaining why a schedule was proposed.
- Identifying overloaded days or unrealistic plans.

LLM responses should be treated as suggestions. The app should validate all structured outputs before writing to the database, TaskNotes, or a calendar.

If the user does not provide an estimate, the LLM can ask follow-up questions, infer a duration, suggest task priority, and propose splitting a large task into smaller discrete tasks. These remain suggestions until the user accepts them or the deterministic scheduler validates them.

The first prototype should support three concrete LLM adapters:

```text
OpenAI
  Hosted API.
  User provides API key.
  Credentials stored in macOS Keychain.

Anthropic
  Hosted API.
  User provides API key.
  Credentials stored in macOS Keychain.

Ollama
  Local HTTP API.
  No API key required by default.
  User configures base URL and model.
```

The provider interface should normalize requests and responses into app-owned types so scheduling and onboarding code does not depend directly on any provider SDK.

## Open Questions

1. Should OpenAI-compatible custom endpoints be supported immediately after OpenAI, Anthropic, and Ollama?
2. What exact TaskNotes field mapping should the app assume before it can read a user's TaskNotes settings?
3. Which TaskNotes fields should be first-class app fields versus preserved provider metadata?
4. What should the first conflict review UI show beyond "newest edit wins" snapshots?
5. How should Google Calendar OAuth be packaged for a desktop app?
6. Should app scheduling metadata be written into TaskNotes frontmatter, kept only in SQLite, or both?
7. Which recurrence behavior belongs in MVP?
8. How much of onboarding should happen before the user sees the main app?

## Current Decisions

1. First target platform: macOS desktop.
2. App shell and UI: Tauri 2 with a TypeScript frontend.
3. Local storage: SQLite from day one, stored by default in `~/.todo-app`.
4. TaskNotes sync: direct vault file access first, with optional HTTP API support later.
5. Scheduling UX: hybrid manual scheduling plus automatic suggestions.
6. LLM strategy: bring your own provider and API key; LLM support is optional.
7. First LLM providers: OpenAI, Anthropic, and Ollama.
8. First calendar provider: Google Calendar read/write, followed by Apple Calendar later.
9. TaskNotes round-tripping should preserve unknown frontmatter, custom fields, and markdown body content.
10. MVP conflict policy: newest edit wins with conflict snapshots.
11. Cloud sync is out of scope for MVP.
12. MVP onboarding asks about calendar integration and LLM provider, but does not ask for a local database location.

## Suggested MVP

Build the first version as a macOS desktop app with:

- Local SQLite task storage.
- A basic task inbox.
- Today and upcoming views.
- Manual scheduling onto a timeline.
- A simple automatic scheduler based on due date, priority, estimate, and available time.
- Optional LLM setup during onboarding.
- LLM-assisted task capture, duration estimates, task splitting, and schedule suggestions.
- Read-only TaskNotes import.
- Google Calendar read access.

After that works, add:

- Two-way TaskNotes sync.
- Google Calendar write access.
- Apple Calendar read/write access.
- More LLM provider adapters.
- Web app support.
- iOS support.
