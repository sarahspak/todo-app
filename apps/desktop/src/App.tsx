import { invoke } from "@tauri-apps/api/core";
import {
  type KeyboardEvent,
  type PointerEvent,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  Bell,
  Bot,
  CalendarDays,
  ChevronDown,
  CheckCircle2,
  Clock3,
  Copy,
  Database,
  FileText,
  Flag,
  Inbox,
  MoreHorizontal,
  Paperclip,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
  X,
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

type Task = {
  id: string;
  title: string;
  notes?: string | null;
  status: "Inbox" | "Planned" | "InProgress" | "Done" | "Canceled";
  priority: "Low" | "Normal" | "High" | "Urgent";
  due_at_ms?: number | null;
  created_at_ms: number;
  updated_at_ms: number;
  completed_at_ms?: number | null;
  sort_order: number;
};

type TaskDragState = {
  id: string;
  startY: number;
  didDrag: boolean;
};

export function App() {
  const [activeView, setActiveView] = useState<View>("inbox");
  const [taskEditorOpen, setTaskEditorOpen] = useState(false);
  const [editingTaskId, setEditingTaskId] = useState<string | null>(null);
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDescription, setTaskDescription] = useState("");
  const [inboxTasks, setInboxTasks] = useState<Task[]>([]);
  const [taskError, setTaskError] = useState<string | null>(null);
  const [openTaskMenuId, setOpenTaskMenuId] = useState<string | null>(null);
  const [draggedTaskId, setDraggedTaskId] = useState<string | null>(null);
  const [dragOverTaskId, setDragOverTaskId] = useState<string | null>(null);
  const [taskSubmitting, setTaskSubmitting] = useState(false);
  const inboxTasksRef = useRef<Task[]>([]);
  const taskDragRef = useRef<TaskDragState | null>(null);
  const suppressTaskClickRef = useRef(false);
  const isInbox = activeView === "inbox";

  useEffect(() => {
    inboxTasksRef.current = inboxTasks;
  }, [inboxTasks]);

  useEffect(() => {
    invoke<Task[]>("list_tasks", { input: { status: "Inbox" } })
      .then((loadedTasks) => {
        setInboxTasks(loadedTasks);
      })
      .catch((error) => {
        setTaskError(String(error));
      });
  }, []);

  useEffect(() => {
    if (!taskEditorOpen) {
      return;
    }

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        closeTaskEditor();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [taskEditorOpen]);

  const closeTaskEditor = () => {
    setTaskEditorOpen(false);
    setEditingTaskId(null);
    setTaskTitle("");
    setTaskDescription("");
    setTaskError(null);
    setTaskSubmitting(false);
  };

  const openNewTaskEditor = () => {
    setEditingTaskId(null);
    setTaskTitle("");
    setTaskDescription("");
    setTaskError(null);
    setOpenTaskMenuId(null);
    setTaskEditorOpen(true);
  };

  const openEditTaskEditor = (task: Task) => {
    setEditingTaskId(task.id);
    setTaskTitle(task.title);
    setTaskDescription(task.notes ?? "");
    setTaskError(null);
    setOpenTaskMenuId(null);
    setTaskEditorOpen(true);
  };

  const saveInboxTask = async () => {
    const title = taskTitle.trim();
    if (!title) {
      return;
    }

    setTaskSubmitting(true);
    setTaskError(null);

    try {
      if (editingTaskId) {
        const task = await invoke<Task>("update_task", {
          input: {
            id: editingTaskId,
            title,
            notes: taskDescription.trim() || null,
          },
        });
        setInboxTasks((currentTasks) =>
          currentTasks.map((currentTask) =>
            currentTask.id === task.id ? task : currentTask,
          ),
        );
      } else {
        const task = await invoke<Task>("create_task", {
          input: {
            title,
            notes: taskDescription.trim() || null,
            priority: "Normal",
          },
        });
        setInboxTasks((currentTasks) => [...currentTasks, task]);
      }
      closeTaskEditor();
    } catch (error) {
      setTaskError(String(error));
      setTaskSubmitting(false);
    }
  };

  const saveTaskOnCommandEnter = (event: KeyboardEvent<HTMLElement>) => {
    if (event.metaKey && event.key === "Enter" && !taskSubmitting) {
      event.preventDefault();
      void saveInboxTask();
    }
  };

  const duplicateInboxTask = async (taskToDuplicate: Task) => {
    setTaskError(null);
    setOpenTaskMenuId(null);

    try {
      const duplicatedTask = await invoke<Task>("create_task", {
        input: {
          title: `${taskToDuplicate.title} copy`,
          notes: taskToDuplicate.notes ?? null,
          priority: taskToDuplicate.priority,
          due_at_ms: taskToDuplicate.due_at_ms ?? null,
        },
      });
      setInboxTasks((currentTasks) => {
        const taskIndex = currentTasks.findIndex(
          (task) => task.id === taskToDuplicate.id,
        );
        if (taskIndex === -1) {
          return [...currentTasks, duplicatedTask];
        }

        return [
          ...currentTasks.slice(0, taskIndex + 1),
          duplicatedTask,
          ...currentTasks.slice(taskIndex + 1),
        ];
      });
    } catch (error) {
      setTaskError(String(error));
    }
  };

  const deleteInboxTask = async (id: string) => {
    setTaskError(null);
    setOpenTaskMenuId(null);

    try {
      await invoke("delete_task", { id });
      setInboxTasks((currentTasks) =>
        currentTasks.filter((task) => task.id !== id),
      );
    } catch (error) {
      setTaskError(String(error));
    }
  };

  const completeInboxTask = async (id: string) => {
    setTaskError(null);
    setOpenTaskMenuId(null);

    try {
      await invoke<Task>("complete_task", { id });
      setInboxTasks((currentTasks) =>
        currentTasks.filter((task) => task.id !== id),
      );
    } catch (error) {
      setTaskError(String(error));
    }
  };

  const persistTaskOrder = async (orderedTasks: Task[]) => {
    try {
      await invoke("reorder_tasks", {
        input: { task_ids: orderedTasks.map((task) => task.id) },
      });
    } catch (error) {
      setTaskError(String(error));
    }
  };

  const reorderTaskList = (tasksToOrder: Task[], draggedId: string, targetId: string) => {
    const draggedIndex = tasksToOrder.findIndex((task) => task.id === draggedId);
    const targetIndex = tasksToOrder.findIndex((task) => task.id === targetId);

    if (draggedIndex === -1 || targetIndex === -1 || draggedIndex === targetIndex) {
      return tasksToOrder;
    }

    const nextTasks = [...tasksToOrder];
    const [draggedTask] = nextTasks.splice(draggedIndex, 1);
    nextTasks.splice(targetIndex, 0, draggedTask);

    return nextTasks.map((task, index) => ({
      ...task,
      sort_order: index,
    }));
  };

  const findTaskIdAtPoint = (x: number, y: number) => {
    return document
      .elementsFromPoint(x, y)
      .map((element) => element.closest<HTMLElement>("[data-task-id]"))
      .find(Boolean)?.dataset.taskId;
  };

  const startTaskPointerDrag = (event: PointerEvent<HTMLElement>, id: string) => {
    if (event.button !== 0) {
      return;
    }

    event.currentTarget.setPointerCapture(event.pointerId);
    taskDragRef.current = {
      id,
      startY: event.clientY,
      didDrag: false,
    };
    setOpenTaskMenuId(null);
  };

  const moveTaskPointerDrag = (event: PointerEvent<HTMLElement>) => {
    const dragState = taskDragRef.current;
    if (!dragState) {
      return;
    }

    const movedFarEnough = Math.abs(event.clientY - dragState.startY) > 5;
    if (!dragState.didDrag && !movedFarEnough) {
      return;
    }

    event.preventDefault();
    dragState.didDrag = true;
    suppressTaskClickRef.current = true;
    setDraggedTaskId(dragState.id);

    const targetId = findTaskIdAtPoint(event.clientX, event.clientY);
    setDragOverTaskId(targetId ?? null);

    if (targetId && targetId !== dragState.id) {
      const nextTasks = reorderTaskList(inboxTasksRef.current, dragState.id, targetId);
      if (nextTasks !== inboxTasksRef.current) {
        inboxTasksRef.current = nextTasks;
        setInboxTasks(nextTasks);
      }
    }
  };

  const endTaskPointerDrag = (event?: PointerEvent<HTMLElement>) => {
    if (event?.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    const dragState = taskDragRef.current;
    taskDragRef.current = null;
    setDraggedTaskId(null);
    setDragOverTaskId(null);

    if (dragState?.didDrag) {
      void persistTaskOrder(inboxTasksRef.current);
      window.setTimeout(() => {
        suppressTaskClickRef.current = false;
      }, 0);
    }
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
            {inboxTasks.length > 0 ? (
              <div className="inbox-task-list">
                {inboxTasks.map((task) => (
                  <article
                    className={`inbox-task-row ${
                      draggedTaskId === task.id ? "dragging" : ""
                    } ${dragOverTaskId === task.id ? "drag-over" : ""}`}
                    data-task-id={task.id}
                    key={task.id}
                  >
                    <button
                      aria-label={`Complete ${task.title}`}
                      className="task-complete-ring"
                      onClick={() => {
                        void completeInboxTask(task.id);
                      }}
                      type="button"
                    />
                    <div
                      className="task-content-button"
                      onClick={() => {
                        if (suppressTaskClickRef.current) {
                          suppressTaskClickRef.current = false;
                          return;
                        }
                        openEditTaskEditor(task);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          openEditTaskEditor(task);
                        }
                      }}
                      onPointerCancel={endTaskPointerDrag}
                      onPointerDown={(event) => startTaskPointerDrag(event, task.id)}
                      onPointerMove={moveTaskPointerDrag}
                      onPointerUp={endTaskPointerDrag}
                      role="button"
                      tabIndex={0}
                    >
                      <h2>{task.title}</h2>
                      {task.notes ? <p>{task.notes}</p> : null}
                    </div>
                    <button
                      aria-expanded={openTaskMenuId === task.id}
                      aria-haspopup="menu"
                      aria-label={`More actions for ${task.title}`}
                      className="task-row-icon-button"
                      onClick={() => {
                        setOpenTaskMenuId((currentId) =>
                          currentId === task.id ? null : task.id,
                        );
                      }}
                      type="button"
                    >
                      <MoreHorizontal aria-hidden="true" />
                    </button>
                    {openTaskMenuId === task.id ? (
                      <div className="task-row-menu" role="menu">
                        <button
                          onClick={() => {
                            openEditTaskEditor(task);
                          }}
                          role="menuitem"
                          type="button"
                        >
                          <Pencil aria-hidden="true" />
                          Edit
                        </button>
                        <button
                          onClick={() => {
                            void duplicateInboxTask(task);
                          }}
                          role="menuitem"
                          type="button"
                        >
                          <Copy aria-hidden="true" />
                          Duplicate
                        </button>
                        <button
                          className="danger-menu-item"
                          onClick={() => {
                            void deleteInboxTask(task.id);
                          }}
                          role="menuitem"
                          type="button"
                        >
                          <Trash2 aria-hidden="true" />
                          Delete
                        </button>
                      </div>
                    ) : null}
                  </article>
                ))}
                <button
                  className="inline-add-task-button"
                  onClick={openNewTaskEditor}
                >
                  + Add task
                </button>
              </div>
            ) : (
              <div className="empty-inbox">
                <button
                  className="add-task-button"
                  onClick={openNewTaskEditor}
                >
                  + Add task
                </button>
              </div>
            )}
            {taskEditorOpen ? (
              <div className="task-modal-backdrop" role="presentation">
                <form
                  aria-label={editingTaskId ? "Edit task" : "Add task"}
                  className="task-detail-modal"
                  role="dialog"
                  onSubmit={async (event) => {
                    event.preventDefault();
                    await saveInboxTask();
                  }}
                >
                  <header className="task-detail-header">
                    <button className="task-detail-project" type="button">
                      <Inbox aria-hidden="true" />
                      Inbox
                    </button>
                    <div className="task-detail-header-actions">
                      <button
                        aria-label="More actions"
                        className="task-detail-icon-button"
                        type="button"
                      >
                        <MoreHorizontal aria-hidden="true" />
                      </button>
                      <button
                        className="secondary-task-action"
                        onClick={closeTaskEditor}
                        type="button"
                      >
                        Cancel
                      </button>
                      <button
                        type="submit"
                        className="submit-task-action"
                        disabled={!taskTitle.trim() || taskSubmitting}
                      >
                        {taskSubmitting
                          ? editingTaskId
                            ? "Saving"
                            : "Adding"
                          : editingTaskId
                            ? "Save"
                            : "Add task"}
                      </button>
                      <button
                        aria-label="Close task"
                        className="task-detail-icon-button"
                        onClick={closeTaskEditor}
                        type="button"
                      >
                        <X aria-hidden="true" />
                      </button>
                    </div>
                  </header>

                  <div className="task-detail-body">
                    <main className="task-detail-main">
                      <section className="task-overview">
                        <button
                          aria-label={
                            editingTaskId
                              ? `Complete ${taskTitle || "task"}`
                              : "Complete task"
                          }
                          className="task-detail-checkbox"
                          onClick={() => {
                            if (editingTaskId) {
                              void completeInboxTask(editingTaskId);
                              closeTaskEditor();
                            }
                          }}
                          type="button"
                        />
                        <div className="task-detail-fields">
                          <input
                            aria-label="Task name"
                            autoFocus
                            className="task-detail-title-input"
                            onKeyDown={saveTaskOnCommandEnter}
                            onChange={(event) => setTaskTitle(event.target.value)}
                            placeholder="Task name"
                            value={taskTitle}
                          />
                          <textarea
                            aria-label="Task description"
                            className="task-detail-description-input"
                            onKeyDown={saveTaskOnCommandEnter}
                            onChange={(event) =>
                              setTaskDescription(event.target.value)
                            }
                            placeholder="Description"
                            rows={7}
                            value={taskDescription}
                          />
                        </div>
                      </section>

                      <button className="task-detail-add-subtask" type="button">
                        <Plus aria-hidden="true" />
                        Add sub-task
                      </button>

                      <section className="task-detail-comments">
                        <div className="task-comment-avatar">S</div>
                        <button className="task-comment-button" type="button">
                          Comment
                        </button>
                        <button
                          aria-label="Attach file"
                          className="task-detail-icon-button"
                          type="button"
                        >
                          <Paperclip aria-hidden="true" />
                        </button>
                      </section>
                    </main>

                    <aside className="task-detail-sidebar" aria-label="Task details">
                      <div className="task-detail-property">
                        <span>Project</span>
                        <button type="button">
                          <Inbox aria-hidden="true" />
                          Inbox
                          <ChevronDown aria-hidden="true" />
                        </button>
                      </div>
                      <div className="task-detail-property">
                        <span>Date</span>
                        <button type="button">
                          <CalendarDays aria-hidden="true" />
                          No date
                        </button>
                      </div>
                      <div className="task-detail-property">
                        <span>Deadline</span>
                        <button type="button">
                          <Plus aria-hidden="true" />
                          Add
                        </button>
                      </div>
                      <div className="task-detail-property">
                        <span>Priority</span>
                        <button type="button">
                          <Flag aria-hidden="true" />
                          {editingTaskId
                            ? inboxTasks.find((task) => task.id === editingTaskId)
                                ?.priority ?? "Normal"
                            : "Normal"}
                          <ChevronDown aria-hidden="true" />
                        </button>
                      </div>
                      <div className="task-detail-property">
                        <span>Labels</span>
                        <button type="button">
                          <Plus aria-hidden="true" />
                          Add
                        </button>
                      </div>
                      <div className="task-detail-property">
                        <span>Reminders</span>
                        <button type="button">
                          <Bell aria-hidden="true" />
                          Add
                        </button>
                      </div>
                    </aside>
                  </div>
                  {taskError ? <p className="task-editor-error">{taskError}</p> : null}
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
