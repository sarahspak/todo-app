import {
  Bot,
  CalendarDays,
  CheckCircle2,
  Clock3,
  Database,
  FileText,
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

export function App() {
  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <CheckCircle2 aria-hidden="true" />
          <span>Todo App</span>
        </div>

        <nav className="nav">
          <button className="nav-item active">
            <Clock3 aria-hidden="true" />
            Today
          </button>
          <button className="nav-item">
            <CalendarDays aria-hidden="true" />
            Schedule
          </button>
          <button className="nav-item">
            <Sparkles aria-hidden="true" />
            Suggestions
          </button>
        </nav>
      </aside>

      <section className="workspace">
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
      </section>
    </main>
  );
}
