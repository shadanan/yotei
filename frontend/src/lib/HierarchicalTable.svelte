<script lang="ts">
  import { AngleRightOutline, BarsOutline } from "flowbite-svelte-icons";
  import type { Task } from "./task";

  export let tasks: Map<string, Task>;
  export let rootTaskId: string;

  let draggedTaskId: string | null = null;
  let maybePeerTaskId: string | null = null;
  let maybeChildTaskId: string | null = null;

  let expanded = new Map<string, boolean>();
  for (const [taskId, _] of tasks) {
    expanded.set(taskId, true);
  }

  function toggleExpanded(taskId: string) {
    expanded = expanded.set(taskId, !expanded.get(taskId));
  }

  function hasCycle(parent: string, child: string): boolean {
    if (child === parent) {
      return true;
    }
    for (const next of tasks.get(child)!.children) {
      if (hasCycle(parent, next)) {
        return true;
      }
    }
    return false;
  }

  function isValid(parent: string, child: string): boolean {
    if (hasCycle(parent, child)) {
      return false;
    }
    if (tasks.get(parent)?.children.includes(child)) {
      return false;
    }
    return true;
  }

  function dragstart(event: DragEvent, taskId: string) {
    draggedTaskId = taskId;
    event.dataTransfer!.effectAllowed = "linkMove";

    document.getElementById(`peer-dropzone-${taskId}`)!.hidden = true;
    document.getElementById(`child-dropzone-${taskId}`)!.hidden = true;

    const rowEl = document.getElementById(`row-${taskId}`)!;
    const handleEl = document.getElementById(`handle-${taskId}`)!;
    const rowRect = rowEl.getBoundingClientRect();
    const handleRect = handleEl.getBoundingClientRect();

    event.dataTransfer!.setDragImage(
      rowEl,
      handleRect.x - rowRect.x + event.offsetX,
      handleRect.y - rowRect.y + event.offsetY,
    );
  }

  function dragover(
    event: DragEvent,
    draggedOverTaskId: string,
    relationship: "peer" | "child",
  ) {
    event.preventDefault();
    if (draggedTaskId === null) {
      return;
    }
    if (!isValid(draggedOverTaskId, draggedTaskId)) {
      return;
    }
    if (relationship === "child") {
      maybeChildTaskId = draggedOverTaskId;
    } else if (relationship === "peer") {
      maybePeerTaskId = draggedOverTaskId;
    }
  }

  function dragleave(event: DragEvent) {
    event.preventDefault();
    maybeChildTaskId = null;
    maybePeerTaskId = null;
  }

  function drop(
    event: DragEvent,
    droppedTaskId: string,
    relationship: "peer" | "child",
  ) {
    event.preventDefault();
    if (draggedTaskId === null) {
      return;
    }
    if (!isValid(droppedTaskId, draggedTaskId)) {
      return;
    }
    if (relationship === "child") {
      tasks.get(droppedTaskId)!.children.splice(0, 0, draggedTaskId);
      tasks = tasks;
    } else if (relationship === "peer") {
      for (const [_, task] of tasks) {
        const index = task.children.indexOf(droppedTaskId);
        if (index !== -1) {
          task.children.splice(index + 1, 0, draggedTaskId);
        }
      }
      tasks = tasks;
    }
  }

  function dragend(event: DragEvent, taskId: string) {
    event.preventDefault();
    draggedTaskId = null;
    maybeChildTaskId = null;
    maybePeerTaskId = null;
    document.getElementById(`peer-dropzone-${taskId}`)!.hidden = false;
    document.getElementById(`child-dropzone-${taskId}`)!.hidden = false;
  }

  type RenderedTask = {
    task: Task;
    level: number;
    ghost: boolean;
  };

  let renderedTasks: RenderedTask[] = [];
  function flatten(task: Task, depth: number) {
    renderedTasks.push({
      task,
      level: depth,
      ghost: false,
    });
    if (expanded.get(task.id)!) {
      if (draggedTaskId && maybeChildTaskId === task.id) {
        renderedTasks.push({
          task: tasks.get(draggedTaskId)!,
          level: depth + 1,
          ghost: true,
        });
      }
      for (const childId of task.children) {
        const child = tasks.get(childId)!;
        flatten(child, depth + 1);
      }
    }
    if (draggedTaskId && maybePeerTaskId === task.id) {
      renderedTasks.push({
        task: tasks.get(draggedTaskId)!,
        level: depth,
        ghost: true,
      });
    }
  }
  $: {
    expanded, maybeChildTaskId, maybePeerTaskId;
    renderedTasks = [];
    flatten(tasks.get(rootTaskId)!, 0);
  }
</script>

<h1 class="my-8 text-4xl">Yotei Hierarchical Table</h1>

<div>
  <div id="header" class="rounded border text-xs font-bold uppercase">
    <div class="my-1 flex items-center rounded p-2">
      <div class="border-r" style="width: 12rem">
        <div class="flex items-center">
          <div class="w-5"></div>
          <div class="w-5"></div>
          <div>ID</div>
        </div>
      </div>
      <div class="w-40 px-2">Description</div>
    </div>
  </div>

  <div
    id="body"
    class="[&>*:nth-child(even)]:bg-gray-100 [&>*:nth-child(odd)]:bg-gray-200"
  >
    {#each renderedTasks as renderedTask}
      {@const isExpanded = expanded.get(renderedTask.task.id)}
      <div
        id="row-{renderedTask.task.id}"
        class="my-1 flex items-center rounded p-2"
        style="opacity: {renderedTask.ghost ? 0.5 : 1};"
      >
        <div style="width: 12rem">
          <div class="flex items-center">
            <button
              class="w-5"
              style="margin-left: {renderedTask.level * 1.25}rem;"
              on:click={() => toggleExpanded(renderedTask.task.id)}
            >
              {#if renderedTask.task.hasChildren()}
                <AngleRightOutline
                  class="h-4 transition-transform"
                  style="transform:rotate({isExpanded ? '90' : '0'}deg)"
                />
              {/if}
            </button>
            <button
              id="handle-{renderedTask.task.id}"
              class="relative w-5"
              draggable={true}
              on:dragstart={(event) => dragstart(event, renderedTask.task.id)}
              on:dragend={(event) => dragend(event, renderedTask.task.id)}
            >
              <BarsOutline class="h-4" />
              <div
                id="peer-dropzone-{renderedTask.task.id}"
                class="absolute -left-6 z-50 h-7 w-12"
                role="table"
                on:dragover={(event) =>
                  dragover(event, renderedTask.task.id, "peer")}
                on:dragleave={(event) => dragleave(event)}
                on:drop={(event) => drop(event, renderedTask.task.id, "peer")}
              />
              <div
                id="child-dropzone-{renderedTask.task.id}"
                class="absolute left-6 z-50 h-7 w-12"
                role="table"
                on:dragover={(event) =>
                  dragover(event, renderedTask.task.id, "child")}
                on:dragleave={(event) => dragleave(event)}
                on:drop={(event) => drop(event, renderedTask.task.id, "child")}
              />
            </button>
            <div>{renderedTask.task.id}</div>
          </div>
        </div>
        <div class="w-40 px-2">{renderedTask.task.title}</div>
      </div>
    {/each}
  </div>
</div>
