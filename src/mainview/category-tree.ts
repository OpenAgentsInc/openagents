/**
 * Category Tree Module
 *
 * Displays TB tasks grouped by category in a collapsible tree view.
 * Supports expand/collapse all, task status icons, and category statistics.
 */

import type { TBState, TBTaskStatus, CategoryData } from "./shared-types.js"

// ============================================================================
// State
// ============================================================================

let tbState: TBState | null = null

// Track collapsed categories
const collapsedCategories = new Set<string>()

// ============================================================================
// DOM Elements
// ============================================================================

let categoryTree: HTMLElement | null = null
let treeContent: HTMLElement | null = null
let treeExpandBtn: HTMLElement | null = null
let treeCollapseBtn: HTMLElement | null = null
let treeCloseBtn: HTMLElement | null = null

// ============================================================================
// Public API
// ============================================================================

/**
 * Show the category tree
 */
export function showCategoryTree(): void {
  categoryTree?.classList.remove("hidden")
}

/**
 * Hide the category tree
 */
function hideCategoryTree(): void {
  categoryTree?.classList.add("hidden")
}

/**
 * Group tasks by category
 */
function groupTasksByCategory(): Map<string, CategoryData> {
  const categories = new Map<string, CategoryData>()

  if (!tbState) return categories

  for (const task of tbState.tasks.values()) {
    const cat = task.category || "uncategorized"
    if (!categories.has(cat)) {
      categories.set(cat, { category: cat, tasks: [], passed: 0, failed: 0, pending: 0, total: 0 })
    }
    const catData = categories.get(cat)!
    catData.tasks.push(task)
    catData.total++
    if (task.status === "passed") catData.passed++
    if (task.status === "failed" || task.status === "error" || task.status === "timeout") {
      catData.failed++
    }
    if (task.status === "pending") catData.pending++
  }

  return categories
}

/**
 * Get status icon for a task
 */
function getTaskStatusIcon(status: TBTaskStatus): string {
  switch (status) {
    case "passed": return "✓"
    case "failed": return "✗"
    case "error": return "⚠"
    case "timeout": return "⏱"
    case "running": return "▶"
    default: return "○"
  }
}

/**
 * Render the category tree
 */
export function renderCategoryTree(): void {
  if (!treeContent) return

  const categories = groupTasksByCategory()
  if (categories.size === 0) {
    treeContent.innerHTML = '<div style="padding: 12px; color: var(--text-muted); font-size: 11px;">No tasks loaded</div>'
    return
  }

  const categoryHtml = Array.from(categories.entries())
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([catName, catData]) => {
      const isCollapsed = collapsedCategories.has(catName)
      const tasksHtml = catData.tasks.map(task => {
        const icon = getTaskStatusIcon(task.status)
        const isRunning = task.status === "running"
        return `
          <div class="tb-tree-task ${task.status}${isRunning ? " running" : ""}" data-task-id="${task.id}">
            <span class="tb-task-status-icon ${task.status}">${icon}</span>
            <span class="tb-tree-task-name" title="${task.name}">${task.name}</span>
            ${task.difficulty ? `<span class="tb-tree-task-diff ${task.difficulty}">${task.difficulty.slice(0, 1).toUpperCase()}</span>` : ""}
          </div>
        `
      }).join("")

      const statsHtml = catData.passed > 0 || catData.failed > 0
        ? `<span class="tb-category-pass">✓${catData.passed}</span><span class="tb-category-fail">✗${catData.failed}</span>`
        : ""

      return `
        <div class="tb-category${isCollapsed ? " collapsed" : ""}" data-category="${catName}">
          <div class="tb-category-header">
            <span class="tb-category-chevron">▼</span>
            <span class="tb-category-name">${catName}</span>
            <div class="tb-category-stats">
              ${statsHtml}
              <span class="tb-category-count">${catData.total}</span>
            </div>
          </div>
          <div class="tb-category-tasks">
            ${tasksHtml}
          </div>
        </div>
      `
    }).join("")

  treeContent.innerHTML = categoryHtml

  // Add click handlers for category headers (toggle collapse)
  treeContent.querySelectorAll(".tb-category-header").forEach(header => {
    header.addEventListener("click", () => {
      const category = header.closest(".tb-category") as HTMLElement
      const catName = category?.dataset.category
      if (catName) {
        category.classList.toggle("collapsed")
        if (category.classList.contains("collapsed")) {
          collapsedCategories.add(catName)
        } else {
          collapsedCategories.delete(catName)
        }
      }
    })
  })

  // Add click handlers for tasks
  treeContent.querySelectorAll(".tb-tree-task").forEach(taskEl => {
    taskEl.addEventListener("click", () => {
      const taskId = (taskEl as HTMLElement).dataset.taskId
      if (taskId) {
        console.log("[TB] Task clicked:", taskId)
      }
    })
  })
}

// ============================================================================
// Event Handlers
// ============================================================================

function expandAllCategories(): void {
  collapsedCategories.clear()
  treeContent?.querySelectorAll(".tb-category").forEach(cat => {
    cat.classList.remove("collapsed")
  })
}

function collapseAllCategories(): void {
  const categories = groupTasksByCategory()
  for (const catName of categories.keys()) {
    collapsedCategories.add(catName)
  }
  treeContent?.querySelectorAll(".tb-category").forEach(cat => {
    cat.classList.add("collapsed")
  })
}

// ============================================================================
// Initialization
// ============================================================================

/**
 * Initialize the category tree
 * @param state - TB state reference
 */
export function initCategoryTree(state: TBState): void {
  tbState = state

  // Cache DOM elements
  categoryTree = document.getElementById("tb-category-tree")
  treeContent = document.getElementById("tb-tree-content")
  treeExpandBtn = document.getElementById("tb-tree-expand")
  treeCollapseBtn = document.getElementById("tb-tree-collapse")
  treeCloseBtn = document.getElementById("tb-tree-close")

  // Wire up tree controls
  treeExpandBtn?.addEventListener("click", expandAllCategories)
  treeCollapseBtn?.addEventListener("click", collapseAllCategories)
  treeCloseBtn?.addEventListener("click", hideCategoryTree)

  // Expose tree functions for triggering from handleHudMessage
  ;(window as unknown as Record<string, unknown>).__showCategoryTree = showCategoryTree
  ;(window as unknown as Record<string, unknown>).__renderCategoryTree = renderCategoryTree
}
