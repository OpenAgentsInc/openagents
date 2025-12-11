//! Database module - Visual database management
//!
//! Components:
//! - TableBrowser: Browse tables and records
//! - SQLEditor: Execute SQL queries
//! - SchemaView: Visual schema editor

mod table_browser;
mod sql_editor;
mod schema_view;
mod dashboard;

pub use table_browser::render_table_browser;
pub use sql_editor::render_sql_editor;
pub use schema_view::render_schema_view;
pub use dashboard::render_database_dashboard;
