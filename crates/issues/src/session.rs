//! Session CRUD operations

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Result};
use uuid::Uuid;

/// Session status
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl SessionStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SessionStatus::Running => "running",
            SessionStatus::Completed => "completed",
            SessionStatus::Failed => "failed",
            SessionStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => SessionStatus::Running,
            "completed" => SessionStatus::Completed,
            "failed" => SessionStatus::Failed,
            "cancelled" => SessionStatus::Cancelled,
            _ => SessionStatus::Running,
        }
    }
}

/// Session struct
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub status: SessionStatus,
    pub prompt: String,
    pub model: String,
    pub pid: Option<i32>,
    pub trajectory_path: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub budget_spent: f64,
    pub issues_completed: i32,
}

/// Create a new session
pub fn create_session(
    conn: &Connection,
    project_id: &str,
    prompt: &str,
    model: &str,
    pid: Option<i32>,
) -> Result<Session> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO sessions (id, project_id, status, prompt, model, pid, started_at, budget_spent, issues_completed)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0.0, 0)",
        rusqlite::params![
            id,
            project_id,
            SessionStatus::Running.as_str(),
            prompt,
            model,
            pid,
            now,
        ],
    )?;

    Ok(Session {
        id,
        project_id: project_id.to_string(),
        status: SessionStatus::Running,
        prompt: prompt.to_string(),
        model: model.to_string(),
        pid,
        trajectory_path: None,
        started_at: now,
        ended_at: None,
        budget_spent: 0.0,
        issues_completed: 0,
    })
}

/// Update session status
pub fn update_session_status(
    conn: &Connection,
    session_id: &str,
    status: SessionStatus,
) -> Result<bool> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE sessions SET status = ?1, ended_at = ?2 WHERE id = ?3",
        rusqlite::params![status.as_str(), now, session_id],
    )?;
    Ok(rows > 0)
}

/// Update session trajectory path
pub fn update_session_trajectory(
    conn: &Connection,
    session_id: &str,
    trajectory_path: &str,
) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE sessions SET trajectory_path = ?1 WHERE id = ?2",
        rusqlite::params![trajectory_path, session_id],
    )?;
    Ok(rows > 0)
}

/// Update session budget and issues completed
pub fn update_session_metrics(
    conn: &Connection,
    session_id: &str,
    budget_spent: f64,
    issues_completed: i32,
) -> Result<bool> {
    let rows = conn.execute(
        "UPDATE sessions SET budget_spent = ?1, issues_completed = ?2 WHERE id = ?3",
        rusqlite::params![budget_spent, issues_completed, session_id],
    )?;
    Ok(rows > 0)
}

/// List sessions for a project
pub fn list_sessions(conn: &Connection, project_id: Option<&str>) -> Result<Vec<Session>> {
    // Use parameterized query to prevent SQL injection
    let sessions = if let Some(pid) = project_id {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, status, prompt, model, pid, trajectory_path, started_at, ended_at, budget_spent, issues_completed
             FROM sessions
             WHERE project_id = ?
             ORDER BY started_at DESC"
        )?;

        stmt.query_map([pid], |row| {
            Ok(Session {
                id: row.get(0)?,
                project_id: row.get(1)?,
                status: SessionStatus::from_str(&row.get::<_, String>(2)?),
                prompt: row.get(3)?,
                model: row.get(4)?,
                pid: row.get(5)?,
                trajectory_path: row.get(6)?,
                started_at: row.get(7)?,
                ended_at: row.get(8)?,
                budget_spent: row.get(9)?,
                issues_completed: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, status, prompt, model, pid, trajectory_path, started_at, ended_at, budget_spent, issues_completed
             FROM sessions
             ORDER BY started_at DESC"
        )?;

        stmt.query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                project_id: row.get(1)?,
                status: SessionStatus::from_str(&row.get::<_, String>(2)?),
                prompt: row.get(3)?,
                model: row.get(4)?,
                pid: row.get(5)?,
                trajectory_path: row.get(6)?,
                started_at: row.get(7)?,
                ended_at: row.get(8)?,
                budget_spent: row.get(9)?,
                issues_completed: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?
    };

    Ok(sessions)
}

/// Get a session by ID
pub fn get_session(conn: &Connection, session_id: &str) -> Result<Option<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, status, prompt, model, pid, trajectory_path, started_at, ended_at, budget_spent, issues_completed
         FROM sessions
         WHERE id = ?1",
    )?;

    let session = stmt
        .query_row([session_id], |row| {
            Ok(Session {
                id: row.get(0)?,
                project_id: row.get(1)?,
                status: SessionStatus::from_str(&row.get::<_, String>(2)?),
                prompt: row.get(3)?,
                model: row.get(4)?,
                pid: row.get(5)?,
                trajectory_path: row.get(6)?,
                started_at: row.get(7)?,
                ended_at: row.get(8)?,
                budget_spent: row.get(9)?,
                issues_completed: row.get(10)?,
            })
        })
        .optional()?;

    Ok(session)
}

/// Get active (running) sessions
pub fn get_active_sessions(conn: &Connection) -> Result<Vec<Session>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, status, prompt, model, pid, trajectory_path, started_at, ended_at, budget_spent, issues_completed
         FROM sessions
         WHERE status = 'running'
         ORDER BY started_at DESC",
    )?;

    let sessions = stmt
        .query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                project_id: row.get(1)?,
                status: SessionStatus::from_str(&row.get::<_, String>(2)?),
                prompt: row.get(3)?,
                model: row.get(4)?,
                pid: row.get(5)?,
                trajectory_path: row.get(6)?,
                started_at: row.get(7)?,
                ended_at: row.get(8)?,
                budget_spent: row.get(9)?,
                issues_completed: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;
    use crate::project;

    #[test]
    fn test_create_and_list_sessions() {
        let conn = init_memory_db().unwrap();

        // Create a project first
        let project =
            project::create_project(&conn, "test-project", "/path", None, None, None).unwrap();

        // Create a session
        let session =
            create_session(&conn, &project.id, "test prompt", "sonnet", Some(1234)).unwrap();

        assert_eq!(session.project_id, project.id);
        assert_eq!(session.prompt, "test prompt");
        assert_eq!(session.model, "sonnet");
        assert_eq!(session.pid, Some(1234));
        assert_eq!(session.status, SessionStatus::Running);

        // List sessions
        let sessions = list_sessions(&conn, Some(&project.id)).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].prompt, "test prompt");
    }

    #[test]
    fn test_update_session_status() {
        let conn = init_memory_db().unwrap();

        let project =
            project::create_project(&conn, "test-project", "/path", None, None, None).unwrap();
        let session = create_session(&conn, &project.id, "test", "sonnet", None).unwrap();

        // Update status to completed
        let updated = update_session_status(&conn, &session.id, SessionStatus::Completed).unwrap();
        assert!(updated);

        // Verify update
        let session = get_session(&conn, &session.id).unwrap().unwrap();
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(session.ended_at.is_some());
    }

    #[test]
    fn test_update_session_metrics() {
        let conn = init_memory_db().unwrap();

        let project =
            project::create_project(&conn, "test-project", "/path", None, None, None).unwrap();
        let session = create_session(&conn, &project.id, "test", "sonnet", None).unwrap();

        // Update metrics
        let updated = update_session_metrics(&conn, &session.id, 2.5, 3).unwrap();
        assert!(updated);

        // Verify update
        let session = get_session(&conn, &session.id).unwrap().unwrap();
        assert_eq!(session.budget_spent, 2.5);
        assert_eq!(session.issues_completed, 3);
    }

    #[test]
    fn test_get_active_sessions() {
        let conn = init_memory_db().unwrap();

        let project =
            project::create_project(&conn, "test-project", "/path", None, None, None).unwrap();

        // Create multiple sessions
        let session1 = create_session(&conn, &project.id, "test1", "sonnet", None).unwrap();
        let session2 = create_session(&conn, &project.id, "test2", "sonnet", None).unwrap();

        // Complete one session
        update_session_status(&conn, &session1.id, SessionStatus::Completed).unwrap();

        // Get active sessions
        let active = get_active_sessions(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, session2.id);
    }

    #[test]
    fn test_cascade_delete_on_project_delete() {
        let conn = init_memory_db().unwrap();

        let project =
            project::create_project(&conn, "test-project", "/path", None, None, None).unwrap();
        create_session(&conn, &project.id, "test", "sonnet", None).unwrap();

        // Delete project
        project::delete_project(&conn, &project.id).unwrap();

        // Verify sessions are deleted (cascade)
        let sessions = list_sessions(&conn, Some(&project.id)).unwrap();
        assert_eq!(sessions.len(), 0);
    }
}
