//! Project CRUD operations

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Result};
use uuid::Uuid;

/// Project struct
#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
    pub default_model: Option<String>,
    pub default_budget: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Create a new project
pub fn create_project(
    conn: &Connection,
    name: &str,
    path: &str,
    description: Option<&str>,
    default_model: Option<&str>,
    default_budget: Option<f64>,
) -> Result<Project> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO projects (id, name, path, description, default_model, default_budget, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            id,
            name,
            path,
            description,
            default_model,
            default_budget,
            now,
            now
        ],
    )?;

    Ok(Project {
        id,
        name: name.to_string(),
        path: path.to_string(),
        description: description.map(String::from),
        default_model: default_model.map(String::from),
        default_budget,
        created_at: now.clone(),
        updated_at: now,
    })
}

/// List all projects
pub fn list_projects(conn: &Connection) -> Result<Vec<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, description, default_model, default_budget, created_at, updated_at
         FROM projects
         ORDER BY created_at DESC",
    )?;

    let projects = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                description: row.get(3)?,
                default_model: row.get(4)?,
                default_budget: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(projects)
}

/// Get a project by name
pub fn get_project_by_name(conn: &Connection, name: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, description, default_model, default_budget, created_at, updated_at
         FROM projects
         WHERE name = ?1",
    )?;

    let project = stmt
        .query_row([name], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                description: row.get(3)?,
                default_model: row.get(4)?,
                default_budget: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .optional()?;

    Ok(project)
}

/// Get a project by ID
pub fn get_project_by_id(conn: &Connection, id: &str) -> Result<Option<Project>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, path, description, default_model, default_budget, created_at, updated_at
         FROM projects
         WHERE id = ?1",
    )?;

    let project = stmt
        .query_row([id], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                description: row.get(3)?,
                default_model: row.get(4)?,
                default_budget: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .optional()?;

    Ok(project)
}

/// Delete a project by ID (will cascade delete all sessions)
pub fn delete_project(conn: &Connection, id: &str) -> Result<bool> {
    let rows = conn.execute("DELETE FROM projects WHERE id = ?1", [id])?;
    Ok(rows > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;

    #[test]
    fn test_create_and_list_projects() {
        let conn = init_memory_db().unwrap();

        // Create a project
        let project = create_project(
            &conn,
            "test-project",
            "/path/to/project",
            Some("Test project description"),
            Some("sonnet"),
            Some(5.0),
        )
        .unwrap();

        assert_eq!(project.name, "test-project");
        assert_eq!(project.path, "/path/to/project");
        assert_eq!(
            project.description,
            Some("Test project description".to_string())
        );
        assert_eq!(project.default_model, Some("sonnet".to_string()));
        assert_eq!(project.default_budget, Some(5.0));

        // List projects
        let projects = list_projects(&conn).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");
    }

    #[test]
    fn test_get_project_by_name() {
        let conn = init_memory_db().unwrap();

        create_project(&conn, "test-project", "/path", None, None, None).unwrap();

        let project = get_project_by_name(&conn, "test-project").unwrap();
        assert!(project.is_some());
        assert_eq!(project.unwrap().name, "test-project");

        let not_found = get_project_by_name(&conn, "nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_delete_project() {
        let conn = init_memory_db().unwrap();

        let project = create_project(&conn, "test-project", "/path", None, None, None).unwrap();

        // Delete project
        let deleted = delete_project(&conn, &project.id).unwrap();
        assert!(deleted);

        // Verify it's gone
        let found = get_project_by_name(&conn, "test-project").unwrap();
        assert!(found.is_none());

        // Try deleting again
        let deleted_again = delete_project(&conn, &project.id).unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_unique_project_name() {
        let conn = init_memory_db().unwrap();

        create_project(&conn, "test-project", "/path1", None, None, None).unwrap();

        // Try to create another project with the same name
        let result = create_project(&conn, "test-project", "/path2", None, None, None);
        assert!(result.is_err());
    }
}
