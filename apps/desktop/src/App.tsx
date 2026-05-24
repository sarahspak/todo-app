import { useState } from "react";
import {
  Bell,
  Bot,
  CalendarDays,
  ChevronDown,
  CheckCircle2,
  Clock3,
  Database,
  FileText,
  Flag,
  Inbox,
  MoreHorizontal,
  Paperclip,
  Sparkles,
} from "lucide-react";

const tasks = [
  {
    title: "Draft scheduling model",
    meta: "High priority · 45m · Focus",
    status: "Suggested 10:00 AM",
  },
  {
    title: "Map TaskNotes frontmatter",
    meta: "Normal priority · 30m · Obsidian",
    status: "Inbox",
  },
  {
    title: "Review calendar permissions",
    meta: "High priority · 25m · macOS",
    status: "Suggested 1:30 PM",
  },
];

const providers = [
  { name: "TaskNotes", detail: "Direct vault access first", icon: FileText },
  { name: "Calendar", detail: "Read free/busy before writes", icon: CalendarDays },
  { name: "SQLite", detail: "Local source of truth", icon: Database },
  { name: "LLMs", detail: "OpenAI, Anthropic, Ollama", icon: Bot },
];

type View = "inbox" | "today" | "schedule" | "suggestions";

export function App() {
  const [activeView, setActiveView] = useState<View>("inbox");
  const [taskEditorOpen, setTaskEditorOpen] = useState(false);
  const [taskTitle, setTaskTitle] = useState("");
  const isInbox = activeView === "inbox";

  const closeTaskEditor = () => {
    setTaskEditorOpen(false);
    setTaskTitle("");
  };

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <CheckCircle2 aria-hidden="true" />
          <span>Todo App</span>
        </div>

        <nav className="nav">
          <button
            className={`nav-item ${activeView === "inbox" ? "active" : ""}`}
            onClick={() => setActiveView("inbox")}
          >
            <Inbox aria-hidden="true" />
            Inbox
          </button>
          <button
            className={`nav-item ${activeView === "today" ? "active" : ""}`}
            onClick={() => setActiveView("today")}
          >
            <Clock3 aria-hidden="true" />
            Today
          </button>
          <button
            className={`nav-item ${activeView === "schedule" ? "active" : ""}`}
            onClick={() => setActiveView("schedule")}
          >
            <CalendarDays aria-hidden="true" />
            Schedule
          </button>
          <button
            className={`nav-item ${activeView === "suggestions" ? "active" : ""}`}
            onClick={() => setActiveView("suggestions")}
          >
            <Sparkles aria-hidden="true" />
            Suggestions
          </button>
        </nav>
      </aside>

      <section className={`workspace ${isInbox ? "inbox-workspace" : ""}`}>
        {isInbox ? (
          <section className="inbox-page" aria-labelledby="inbox-title">
            <header className="inbox-header">
              <h1 id="inbox-title">Inbox</h1>
            </header>
            <div className="empty-inbox">
              <button
                className="add-task-button"
                onClick={() => setTaskEditorOpen(true)}
              >
                + Add task
              </button>
            </div>
            {taskEditorOpen ? (
              <div className="task-popover-backdrop" role="presentation">
                <form
                  aria-label="Add task"
                  className="task-editor-popover"
                  onSubmit={(event) => {
                    event.preventDefault();
                    if (taskTitle.trim()) {
                      closeTaskEditor();
                    }
                  }}
                >
                  <div className="task-editor-fields">
                    <input
                      aria-label="Task name"
                      autoFocus
                      className="task-title-input"
                      onChange={(event) => setTaskTitle(event.target.value)}
                      placeholder="Task name"
                      value={taskTitle}
                    />
                    <textarea
                      aria-label="Description"
                      className="task-description-input"
                      placeholder="Description"
                      rows={2}
                    />
                  </div>

                  <div className="task-editor-tools" aria-label="Task options">
                    <button type="button" className="task-tool-button">
                      <CalendarDays aria-hidden="true" />
                      Date
                    </button>
                    <button type="button" className="task-tool-button">
                      <Paperclip aria-hidden="true" />
                      Attachment
                    </button>
                    <button type="button" className="task-tool-button">
                      <Flag aria-hidden="true" />
                      Priority
                    </button>
                    <button type="button" className="task-tool-button">
                      <Bell aria-hidden="true" />
                      Reminders
                    </button>
                    <button
                      aria-label="More actions"
                      type="button"
                      className="task-icon-button"
                    >
                      <MoreHorizontal aria-hidden="true" />
                    </button>
                  </div>

                  <footer className="task-editor-footer">
                    <button type="button" className="project-picker">
                      <Inbox aria-hidden="true" />
                      Inbox
                      <ChevronDown aria-hidden="true" />
                    </button>
                    <div className="task-editor-actions">
                      <button
                        type="button"
                        className="secondary-task-action"
                        onClick={closeTaskEditor}
                      >
                        Cancel
                      </button>
                      <button
                        type="submit"
                        className="submit-task-action"
                        disabled={!taskTitle.trim()}
                      >
                        Add task
                      </button>
                    </div>
                  </footer>
                </form>
              </div>
            ) : null}
          </section>
        ) : (
          <>
            <header className="topbar">
              <div>
                <p className="eyebrow">macOS prototype</p>
                <h1>Plan the day</h1>
              </div>
              <button className="primary-action">
                <Sparkles aria-hidden="true" />
                Suggest schedule
              </button>
            </header>

            <section className="grid">
              <div className="panel task-panel">
                <div className="panel-heading">
                  <h2>Inbox and proposed blocks</h2>
                  <span>3 tasks</span>
                </div>
                <div className="task-list">
                  {tasks.map((task) => (
                    <article className="task-row" key={task.title}>
                      <div>
                        <h3>{task.title}</h3>
                        <p>{task.meta}</p>
                      </div>
                      <span>{task.status}</span>
                    </article>
                  ))}
                </div>
              </div>

              <div className="panel timeline-panel">
                <div className="panel-heading">
                  <h2>Timeline</h2>
                  <span>Hybrid scheduling</span>
                </div>
                <div className="timeline">
                  <div className="time-row">
                    <span>9 AM</span>
                    <div className="busy">Calendar hold</div>
                  </div>
                  <div className="time-row">
                    <span>10 AM</span>
                    <div className="suggested">Draft scheduling model</div>
                  </div>
                  <div className="time-row">
                    <span>1 PM</span>
                    <div className="suggested">Review calendar permissions</div>
                  </div>
                </div>
              </div>
            </section>

            <section className="provider-strip">
              {providers.map((provider) => {
                const Icon = provider.icon;
                return (
                  <article className="provider-card" key={provider.name}>
                    <Icon aria-hidden="true" />
                    <div>
                      <h3>{provider.name}</h3>
                      <p>{provider.detail}</p>
                    </div>
                  </article>
                );
              })}
            </section>
          </>
        )}
      </section>
    </main>
  );
}
