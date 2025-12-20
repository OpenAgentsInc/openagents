//! Repository trait and implementation for marketplace items

use crate::types::ItemStatus;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};

/// Skill data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub version: String,
    pub status: ItemStatus,
    pub icon_url: Option<String>,
    pub readme: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub installed_at: Option<DateTime<Utc>>,
}

/// Skill version data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersion {
    pub id: String,
    pub skill_id: String,
    pub version: String,
    pub changelog: Option<String>,
    pub published_at: DateTime<Utc>,
}

/// Repository trait for marketplace operations
pub trait Repository {
    /// Create a new skill
    fn create(&self, skill: &Skill) -> Result<()>;

    /// Get a skill by slug
    fn get_by_slug(&self, slug: &str) -> Result<Option<Skill>>;

    /// List all skills
    fn list(&self) -> Result<Vec<Skill>>;

    /// Update skill status
    fn update_status(&self, slug: &str, status: ItemStatus) -> Result<()>;

    /// Add a skill version
    fn add_version(&self, version: &SkillVersion) -> Result<()>;

    /// Get versions for a skill
    fn get_versions(&self, skill_id: &str) -> Result<Vec<SkillVersion>>;
}

/// SQLite implementation of Repository
pub struct SkillRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SkillRepository<'a> {
    /// Create a new repository
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl Repository for SkillRepository<'_> {
    fn create(&self, skill: &Skill) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO skills (
                id, slug, name, description, author, version, status,
                icon_url, readme, created_at, updated_at, installed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
            params![
                skill.id,
                skill.slug,
                skill.name,
                skill.description,
                skill.author,
                skill.version,
                skill.status.as_str(),
                skill.icon_url,
                skill.readme,
                skill.created_at.to_rfc3339(),
                skill.updated_at.to_rfc3339(),
                skill.installed_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    fn get_by_slug(&self, slug: &str) -> Result<Option<Skill>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, slug, name, description, author, version, status,
                   icon_url, readme, created_at, updated_at, installed_at
            FROM skills WHERE slug = ?1
            "#,
        )?;

        let skill = stmt
            .query_row([slug], |row| {
                let status_str: String = row.get(6)?;
                let status = serde_json::from_value(serde_json::json!(status_str))
                    .unwrap_or(ItemStatus::Available);

                let created_at: String = row.get(9)?;
                let updated_at: String = row.get(10)?;
                let installed_at: Option<String> = row.get(11)?;

                Ok(Skill {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    author: row.get(4)?,
                    version: row.get(5)?,
                    status,
                    icon_url: row.get(7)?,
                    readme: row.get(8)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .unwrap()
                        .with_timezone(&Utc),
                    installed_at: installed_at
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            })
            .optional()?;

        Ok(skill)
    }

    fn list(&self) -> Result<Vec<Skill>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, slug, name, description, author, version, status,
                   icon_url, readme, created_at, updated_at, installed_at
            FROM skills ORDER BY created_at DESC
            "#,
        )?;

        let skills = stmt
            .query_map([], |row| {
                let status_str: String = row.get(6)?;
                let status = serde_json::from_value(serde_json::json!(status_str))
                    .unwrap_or(ItemStatus::Available);

                let created_at: String = row.get(9)?;
                let updated_at: String = row.get(10)?;
                let installed_at: Option<String> = row.get(11)?;

                Ok(Skill {
                    id: row.get(0)?,
                    slug: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    author: row.get(4)?,
                    version: row.get(5)?,
                    status,
                    icon_url: row.get(7)?,
                    readme: row.get(8)?,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .unwrap()
                        .with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .unwrap()
                        .with_timezone(&Utc),
                    installed_at: installed_at
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(skills)
    }

    fn update_status(&self, slug: &str, status: ItemStatus) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE skills SET status = ?1, updated_at = ?2 WHERE slug = ?3",
            params![status.as_str(), now.to_rfc3339(), slug],
        )?;
        Ok(())
    }

    fn add_version(&self, version: &SkillVersion) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO skill_versions (id, skill_id, version, changelog, published_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                version.id,
                version.skill_id,
                version.version,
                version.changelog,
                version.published_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    fn get_versions(&self, skill_id: &str) -> Result<Vec<SkillVersion>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, skill_id, version, changelog, published_at
            FROM skill_versions
            WHERE skill_id = ?1
            ORDER BY published_at DESC
            "#,
        )?;

        let versions = stmt
            .query_map([skill_id], |row| {
                let published_at: String = row.get(4)?;
                Ok(SkillVersion {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    version: row.get(2)?,
                    changelog: row.get(3)?,
                    published_at: DateTime::parse_from_rfc3339(&published_at)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(versions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_memory_db;
    use uuid::Uuid;

    fn create_test_skill() -> Skill {
        Skill {
            id: Uuid::new_v4().to_string(),
            slug: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            description: Some("A test skill".to_string()),
            author: Some("Test Author".to_string()),
            version: "1.0.0".to_string(),
            status: ItemStatus::Available,
            icon_url: None,
            readme: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            installed_at: None,
        }
    }

    #[test]
    fn test_create_and_get_skill() {
        let conn = init_memory_db().unwrap();
        let repo = SkillRepository::new(&conn);
        let skill = create_test_skill();

        repo.create(&skill).unwrap();

        let retrieved = repo.get_by_slug(&skill.slug).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.slug, skill.slug);
        assert_eq!(retrieved.name, skill.name);
    }

    #[test]
    fn test_list_skills() {
        let conn = init_memory_db().unwrap();
        let repo = SkillRepository::new(&conn);

        let skill1 = create_test_skill();
        let mut skill2 = create_test_skill();
        skill2.slug = "another-skill".to_string();

        repo.create(&skill1).unwrap();
        repo.create(&skill2).unwrap();

        let skills = repo.list().unwrap();
        assert_eq!(skills.len(), 2);
    }

    #[test]
    fn test_update_status() {
        let conn = init_memory_db().unwrap();
        let repo = SkillRepository::new(&conn);
        let skill = create_test_skill();

        repo.create(&skill).unwrap();
        repo.update_status(&skill.slug, ItemStatus::Installed)
            .unwrap();

        let updated = repo.get_by_slug(&skill.slug).unwrap().unwrap();
        assert_eq!(updated.status, ItemStatus::Installed);
    }

    #[test]
    fn test_add_and_get_versions() {
        let conn = init_memory_db().unwrap();
        let repo = SkillRepository::new(&conn);
        let skill = create_test_skill();

        repo.create(&skill).unwrap();

        let version = SkillVersion {
            id: Uuid::new_v4().to_string(),
            skill_id: skill.id.clone(),
            version: "1.0.1".to_string(),
            changelog: Some("Bug fixes".to_string()),
            published_at: Utc::now(),
        };

        repo.add_version(&version).unwrap();

        let versions = repo.get_versions(&skill.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "1.0.1");
    }
}
