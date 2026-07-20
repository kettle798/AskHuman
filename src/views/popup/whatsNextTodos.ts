import type { OptionItem, TodoEntry } from "../../lib/types";

const MAX_WHATS_NEXT_OPTIONS = 10;

export interface RefreshedWhatsNextTodos {
  options: OptionItem[];
  selectedOptions: string[];
  hiddenTodos: number;
}

/** Rebuild the local whats-next TODO slice while keeping suggestions and the final end option. */
export function refreshWhatsNextTodos(
  currentOptions: OptionItem[],
  staticOptions: OptionItem[],
  latestTodos: TodoEntry[],
  selectedOptions: string[],
  todoPrefix: string,
): RefreshedWhatsNextTodos {
  const endOption = staticOptions[staticOptions.length - 1];
  if (!endOption) {
    return { options: currentOptions, selectedOptions, hiddenTodos: 0 };
  }

  const taskSlots = MAX_WHATS_NEXT_OPTIONS - 1;
  const suggestions = staticOptions.slice(0, -1).slice(0, taskSlots);
  const todoSlots = taskSlots - suggestions.length;
  const visibleTodos = latestTodos.slice(0, todoSlots);
  const todoOptions: OptionItem[] = visibleTodos.map((todo) => ({
    text: `${todoPrefix}${todo.text}`,
    recommended: false,
    todoId: todo.id,
  }));
  const options = [...suggestions, ...todoOptions, endOption];

  const selectedTodoIds = new Set(
    currentOptions
      .filter((option) => option.todoId && selectedOptions.includes(option.text))
      .map((option) => option.todoId as string),
  );
  const selectedStaticTexts = new Set(
    currentOptions
      .filter((option) => !option.todoId && selectedOptions.includes(option.text))
      .map((option) => option.text),
  );
  const reconciledSelection = options
    .filter((option) =>
      option.todoId
        ? selectedTodoIds.has(option.todoId)
        : selectedStaticTexts.has(option.text),
    )
    .map((option) => option.text);

  return {
    options,
    selectedOptions: reconciledSelection,
    hiddenTodos:
      todoSlots > 0
        ? Math.max(0, latestTodos.length - visibleTodos.length)
        : 0,
  };
}

export interface SelectedWhatsNextTodo {
  id: string;
  text: string;
  optionText: string;
}

/** Resolve a locally refreshed TODO selection without relying on the daemon's request snapshot. */
export function selectedWhatsNextTodo(
  options: OptionItem[],
  selectedOptions: string[],
  latestTodos: TodoEntry[],
  todoPrefix: string,
): SelectedWhatsNextTodo | null {
  const option = options.find(
    (candidate) => candidate.todoId && selectedOptions.includes(candidate.text),
  );
  if (!option?.todoId) return null;
  const latest = latestTodos.find((todo) => todo.id === option.todoId);
  const fallbackText = option.text.startsWith(todoPrefix)
    ? option.text.slice(todoPrefix.length)
    : option.text;
  return {
    id: option.todoId,
    text: latest?.text ?? fallbackText,
    optionText: option.text,
  };
}
