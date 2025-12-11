//! Projects module - Browse, create, and manage Vibe projects
//!
//! Components:
//! - ProjectGrid: Grid of project cards with search
//! - ProjectCard: Individual project display
//! - TemplatePicker: Browse and select starter templates

mod project_grid;
mod project_card;
mod template_picker;

pub use project_grid::render_project_grid;
pub use project_card::render_project_card;
pub use template_picker::render_template_picker;
