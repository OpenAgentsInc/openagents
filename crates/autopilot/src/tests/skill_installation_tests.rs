use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use wgpui::components::molecules::SkillCategory;

use crate::app::catalog::{SkillSource, load_skill_entries};

struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => unsafe {
                env::set_var(self.key, value);
            },
            None => unsafe {
                env::remove_var(self.key);
            },
        }
    }
}

fn write_skill_dir(root: &Path, folder: &str, filename: &str, contents: &str) -> PathBuf {
    let skill_dir = root.join(folder);
    fs::create_dir_all(&skill_dir).expect("create skill dir");
    fs::write(skill_dir.join(filename), contents).expect("write SKILL.md");
    skill_dir
}

#[test]
fn skill_install_workflow_discovers_project_and_user_skills() {
    let workspace = TempDir::new().expect("temp workspace");
    let home = TempDir::new().expect("temp home");
    let _home_guard = EnvVarGuard::set("HOME", home.path());

    let project_skills_root = workspace.path().join(".openagents").join("skills");
    let user_skills_root = home
        .path()
        .join(".openagents")
        .join("skills");

    write_skill_dir(
        &project_skills_root,
        "project-skill",
        "SKILL.md",
        r#"---
name: Project Skill
description: Installed into the workspace.
categories: ["code generation"]
author: acme
version: 0.1.0
---

Body is ignored when description is present.
"#,
    );

    write_skill_dir(
        &user_skills_root,
        "user-skill",
        "SKILL.md",
        r#"---
name: User Skill
tags: ["web automation"]
author: bob
version: 2.0.0
---

First non-empty line becomes the description.
"#,
    );

    let catalog = load_skill_entries(workspace.path());
    assert!(
        catalog.error.is_none(),
        "unexpected catalog errors: {:?}",
        catalog.error
    );

    let project = catalog
        .entries
        .iter()
        .find(|entry| entry.source == SkillSource::Project)
        .expect("project skill entry");
    assert_eq!(project.info.id, "project:project-skill");
    assert_eq!(project.info.name, "Project Skill");
    assert_eq!(project.info.description, "Installed into the workspace.");
    assert_eq!(project.info.category, SkillCategory::CodeGeneration);
    assert_eq!(project.info.author, "acme");
    assert_eq!(project.info.version, "0.1.0");

    let user = catalog
        .entries
        .iter()
        .find(|entry| entry.source == SkillSource::User)
        .expect("user skill entry");
    assert_eq!(user.info.id, "user:user-skill");
    assert_eq!(user.info.name, "User Skill");
    assert_eq!(
        user.info.description,
        "First non-empty line becomes the description."
    );
    assert_eq!(user.info.category, SkillCategory::WebAutomation);
    assert_eq!(user.info.author, "bob");
    assert_eq!(user.info.version, "2.0.0");
}

#[test]
fn skill_install_workflow_supports_lowercase_skill_filename() {
    let workspace = TempDir::new().expect("temp workspace");

    let project_skills_root = workspace.path().join(".openagents").join("skills");
    write_skill_dir(
        &project_skills_root,
        "lowercase",
        "skill.md",
        r#"---
name: Lowercase Skill
---

Description from body.
"#,
    );

    let catalog = load_skill_entries(workspace.path());
    assert!(catalog.error.is_none());

    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.info.id == "project:lowercase")
        .expect("lowercase skill entry");
    assert_eq!(entry.info.name, "Lowercase Skill");
    assert_eq!(entry.info.description, "Description from body.");
}
