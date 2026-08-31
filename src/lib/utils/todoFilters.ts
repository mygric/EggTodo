import type { Todo } from "$lib/types";
import { isDueTodayOrOverdue } from "./todoDates";

export type TodoListView = "all" | "today" | "quadrants" | "calendar";

export interface TodoFilterOptions {
  view?: TodoListView;
  groupUuid?: string | null;
  now?: Date;
}

export function filterTodos(
  items: Todo[],
  query: string,
  showCompleted: boolean,
  options: TodoFilterOptions = {},
): Todo[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const view = options.view ?? "all";
  const groupUuid = options.groupUuid;
  const now = options.now ?? new Date();

  return items.filter((todo) => {
    if (groupUuid === null && todo.group_uuid !== null) return false;
    if (typeof groupUuid === "string" && todo.group_uuid !== groupUuid) {
      return false;
    }
    if (view === "today") {
      if (todo.completed) {
        // 已完成任务：只有今天完成的才显示在今天视图里
        if (!showCompleted) return false;
        if (!isCompletedToday(todo, now)) return false;
      } else {
        // 未完成任务：今天到期或逾期
        if (!isDueTodayOrOverdue(todo, now)) return false;
      }
    }
    if (!showCompleted && todo.completed) return false;
    if (!normalizedQuery) return true;
    return todo.title.toLocaleLowerCase().includes(normalizedQuery);
  });
}

function isCompletedToday(todo: Todo, now: Date): boolean {
  if (todo.completed_at === null) return false;
  const completed = new Date(todo.completed_at);
  return (
    completed.getFullYear() === now.getFullYear() &&
    completed.getMonth() === now.getMonth() &&
    completed.getDate() === now.getDate()
  );
}
