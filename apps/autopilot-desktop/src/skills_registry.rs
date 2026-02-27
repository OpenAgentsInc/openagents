use std::fs;
use std::path::{Path, PathBuf};

const SKILLS_ROOT_ENV: &str = "OPENAGENTS_SKILLS_DIR";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalSkill {
    pub project: String,
    pub name: String,
    pub skill_md_path: PathBuf,
}

impl LocalSkill {
    pub fn slug(&self) -> String {
        format!("{}/{}", self.project, self.name)
    }
}

#[derive(Debug)]
pub struct DerivedLocalSkill {
    pub local_skill: LocalSkill,
    pub derived: nostr::DerivedManifest,
}

pub fn discover_local_skills() -> Result<Vec<LocalSkill>, String> {
    discover_local_skills_in(&skills_root())
}

pub fn derive_local_skill_manifest(
    skill_slug: &str,
    publisher_pubkey: &str,
    created_at: u64,
) -> Result<DerivedLocalSkill, String> {
    derive_local_skill_manifest_in(&skills_root(), skill_slug, publisher_pubkey, created_at)
}

fn skills_root() -> PathBuf {
    std::env::var(SKILLS_ROOT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("skills"))
}

fn discover_local_skills_in(skills_root: &Path) -> Result<Vec<LocalSkill>, String> {
    if !skills_root.exists() {
        return Err(format!(
            "skills registry root does not exist: {}",
            skills_root.display()
        ));
    }
    if !skills_root.is_dir() {
        return Err(format!(
            "skills registry root is not a directory: {}",
            skills_root.display()
        ));
    }

    let mut skills = Vec::new();
    for project_entry in fs::read_dir(skills_root).map_err(|error| {
        format!(
            "failed reading skills registry root {}: {error}",
            skills_root.display()
        )
    })? {
        let project_entry = project_entry.map_err(|error| {
            format!(
                "failed reading skills registry entry under {}: {error}",
                skills_root.display()
            )
        })?;
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_name = project_entry.file_name().to_string_lossy().to_string();

        for skill_entry in fs::read_dir(&project_path).map_err(|error| {
            format!(
                "failed reading project namespace {}: {error}",
                project_path.display()
            )
        })? {
            let skill_entry = skill_entry.map_err(|error| {
                format!(
                    "failed reading skill entry under {}: {error}",
                    project_path.display()
                )
            })?;
            let skill_path = skill_entry.path();
            if !skill_path.is_dir() {
                continue;
            }
            let skill_name = skill_entry.file_name().to_string_lossy().to_string();

            let skill_md_path = resolve_skill_md_path(&skill_path);
            if let Some(skill_md_path) = skill_md_path {
                skills.push(LocalSkill {
                    project: project_name.clone(),
                    name: skill_name,
                    skill_md_path,
                });
            }
        }
    }

    skills.sort_by(|left, right| {
        left.project
            .cmp(&right.project)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(skills)
}

fn derive_local_skill_manifest_in(
    skills_root: &Path,
    skill_slug: &str,
    publisher_pubkey: &str,
    created_at: u64,
) -> Result<DerivedLocalSkill, String> {
    let local_skill = resolve_local_skill_in(skills_root, skill_slug)?;
    let payload = fs::read_to_string(&local_skill.skill_md_path).map_err(|error| {
        format!(
            "failed reading {}: {error}",
            local_skill.skill_md_path.display()
        )
    })?;
    let derived = nostr::derive_manifest_from_skill_payload(&payload, publisher_pubkey, created_at)
        .map_err(|error| {
            format!(
                "invalid SKILL frontmatter in {}: {error}",
                local_skill.skill_md_path.display()
            )
        })?;

    Ok(DerivedLocalSkill {
        local_skill,
        derived,
    })
}

fn resolve_local_skill_in(skills_root: &Path, skill_slug: &str) -> Result<LocalSkill, String> {
    let skills = discover_local_skills_in(skills_root)?;
    if skills.is_empty() {
        return Err(format!(
            "no skills discovered under {}",
            skills_root.display()
        ));
    }

    if let Some((project, skill_name)) = skill_slug.split_once('/') {
        return skills
            .into_iter()
            .find(|skill| skill.project == project && skill.name == skill_name)
            .ok_or_else(|| {
                format!(
                    "skill '{skill_slug}' not found in local registry {}",
                    skills_root.display()
                )
            });
    }

    let matches: Vec<LocalSkill> = skills
        .into_iter()
        .filter(|skill| skill.name == skill_slug)
        .collect();
    match matches.len() {
        1 => Ok(matches[0].clone()),
        0 => Err(format!(
            "skill '{skill_slug}' not found in local registry {}",
            skills_root.display()
        )),
        _ => {
            let mut slugs: Vec<String> = matches.iter().map(LocalSkill::slug).collect();
            slugs.sort();
            Err(format!(
                "skill slug '{skill_slug}' is ambiguous; use one of: {}",
                slugs.join(", ")
            ))
        }
    }
}

fn resolve_skill_md_path(skill_dir: &Path) -> Option<PathBuf> {
    let uppercase = skill_dir.join("SKILL.md");
    if uppercase.is_file() {
        return Some(uppercase);
    }

    let lowercase = skill_dir.join("skill.md");
    if lowercase.is_file() {
        return Some(lowercase);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{
        discover_local_skills_in, derive_local_skill_manifest_in, resolve_local_skill_in,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_skills_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("openagents-skills-test-{nanos}"));
        fs::create_dir_all(&root).expect("create temp skills root");
        root
    }

    fn write_skill(root: &PathBuf, project: &str, name: &str, body: &str) -> PathBuf {
        let skill_dir = root.join(project).join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        let skill_path = skill_dir.join("SKILL.md");
        fs::write(&skill_path, body).expect("write SKILL.md");
        skill_path
    }

    #[test]
    fn discover_local_skills_reads_project_namespace_layout() {
        let root = create_temp_skills_root();
        write_skill(
            &root,
            "mezo",
            "mezo-integration",
            "---\nname: mezo-integration\ndescription: test\n---\nbody\n",
        );

        let skills = discover_local_skills_in(&root).expect("discover skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].project, "mezo");
        assert_eq!(skills[0].name, "mezo-integration");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolve_local_skill_requires_disambiguation_when_names_collide() {
        let root = create_temp_skills_root();
        write_skill(
            &root,
            "mezo",
            "integration",
            "---\nname: integration\ndescription: test\n---\nbody\n",
        );
        write_skill(
            &root,
            "neutron",
            "integration",
            "---\nname: integration\ndescription: test\n---\nbody\n",
        );

        let error = resolve_local_skill_in(&root, "integration").unwrap_err();
        assert!(error.contains("ambiguous"));
        assert!(error.contains("mezo/integration"));
        assert!(error.contains("neutron/integration"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn derive_local_skill_manifest_uses_skill_payload_derivation() {
        let root = create_temp_skills_root();
        write_skill(
            &root,
            "moneydevkit",
            "agent-wallet-ops",
            r#"---
name: agent-wallet-ops
description: Operate agent wallet commands
metadata:
  oa:
    nostr:
      identifier: agent-wallet-ops
      version: "0.1.0"
      expiry_unix: 1756000000
      capabilities_csv: "http:outbound filesystem:read"
---
Run wallet checks
"#,
        );

        let derived = derive_local_skill_manifest_in(
            &root,
            "moneydevkit/agent-wallet-ops",
            "npub1agent",
            1_740_400_100,
        )
        .expect("derive manifest");

        assert_eq!(derived.local_skill.project, "moneydevkit");
        assert_eq!(derived.local_skill.name, "agent-wallet-ops");
        assert_eq!(derived.derived.manifest.identifier, "agent-wallet-ops");
        assert_eq!(derived.derived.manifest.version, "0.1.0");

        let _ = fs::remove_dir_all(root);
    }
}
