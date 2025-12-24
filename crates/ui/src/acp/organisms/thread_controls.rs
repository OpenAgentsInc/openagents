//! Thread controls component with mode and model selectors.

use maud::{Markup, html};
use crate::acp::atoms::AgentMode;
use crate::acp::molecules::{ModeSelector, ModelSelector};
use crate::acp::styles::ACP_THREAD_CONTROLS_CLASS;

/// Todo item in a plan.
pub struct PlanTodo {
    pub content: String,
    pub completed: bool,
}

/// Thread controls bar with selectors and plan display.
pub struct ThreadControls {
    mode: AgentMode,
    model_id: String,
    session_id: String,
    todos: Vec<PlanTodo>,
}

impl ThreadControls {
    /// Create new thread controls.
    pub fn new(
        mode: AgentMode,
        model_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            mode,
            model_id: model_id.into(),
            session_id: session_id.into(),
            todos: Vec::new(),
        }
    }

    /// Set plan todos.
    pub fn todos(mut self, todos: Vec<PlanTodo>) -> Self {
        self.todos = todos;
        self
    }

    /// Build the component.
    pub fn build(self) -> Markup {
        let completed = self.todos.iter().filter(|t| t.completed).count();
        let total = self.todos.len();

        html! {
            div class=(ACP_THREAD_CONTROLS_CLASS) {
                // Mode selector
                (ModeSelector::new(self.mode, &self.session_id).build())

                // Model selector
                (ModelSelector::new(&self.model_id, &self.session_id).build())

                // Spacer
                div class="flex-1" {}

                // Plan progress (if todos exist)
                @if !self.todos.is_empty() {
                    div class="flex items-center gap-2" {
                        span class="text-xs text-muted-foreground" {
                            "Plan: " (completed) "/" (total)
                        }

                        // Mini progress bar
                        div class="w-16 h-1 bg-secondary" {
                            div
                                class="h-full bg-green"
                                style={ "width: " (if total > 0 { completed * 100 / total } else { 0 }) "%" }
                            {}
                        }
                    }
                }
            }

            // Expanded todo list (if in plan mode)
            @if !self.todos.is_empty() {
                div class="px-4 py-2 border-b border-border bg-secondary/50" {
                    ul class="space-y-1" {
                        @for todo in &self.todos {
                            li class="flex items-center gap-2 text-sm" {
                                @if todo.completed {
                                    span class="text-green" { "[x]" }
                                    span class="text-muted-foreground line-through" {
                                        (todo.content)
                                    }
                                } @else {
                                    span class="text-muted-foreground" { "[ ]" }
                                    span class="text-foreground" {
                                        (todo.content)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
