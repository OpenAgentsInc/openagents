use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const MANAGED_SKILLS_ROOT_ENV: &str = "OPENAGENTS_MANAGED_SKILLS_DIR";
const MANAGED_SKILLS_REMOTE_FETCH_ENV: &str = "OPENAGENTS_MANAGED_SKILLS_REMOTE_FETCH";
const MANAGED_SKILLS_REMOTE_TIMEOUT_SECS: u64 = 4;

pub const REQUIRED_CAD_POLICY_SKILLS: &[&str] =
    &["autopilot-cad-builder", "autopilot-pane-control"];
pub const REQUIRED_DATA_MARKET_POLICY_SKILLS: &[&str] = &[
    "autopilot-data-seller",
    "autopilot-data-market-control",
];

struct ManagedSkillSpec {
    name: &'static str,
    raw_url: &'static str,
    embedded_skill_md: &'static str,
}

const MANAGED_CAD_SKILLS: &[ManagedSkillSpec] = &[
    ManagedSkillSpec {
        name: "autopilot-cad-builder",
        raw_url: "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/skills/autopilot-cad-builder/SKILL.md",
        embedded_skill_md: include_str!("../../../skills/autopilot-cad-builder/SKILL.md"),
    },
    ManagedSkillSpec {
        name: "autopilot-pane-control",
        raw_url: "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/skills/autopilot-pane-control/SKILL.md",
        embedded_skill_md: include_str!("../../../skills/autopilot-pane-control/SKILL.md"),
    },
];

const MANAGED_DATA_MARKET_SKILLS: &[ManagedSkillSpec] = &[
    ManagedSkillSpec {
        name: "autopilot-data-seller",
        raw_url: "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/skills/autopilot-data-seller/SKILL.md",
        embedded_skill_md: include_str!("../../../skills/autopilot-data-seller/SKILL.md"),
    },
    ManagedSkillSpec {
        name: "autopilot-data-market-control",
        raw_url: "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/skills/autopilot-data-market-control/SKILL.md",
        embedded_skill_md: include_str!("../../../skills/autopilot-data-market-control/SKILL.md"),
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedSkillAttachment {
    pub name: String,
    pub path: String,
}

pub fn managed_skills_root() -> PathBuf {
    if let Ok(value) = std::env::var(MANAGED_SKILLS_ROOT_ENV)
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }
    if let Some(data_dir) = dirs::data_local_dir() {
        return data_dir.join("openagents").join("skills");
    }
    std::env::temp_dir().join("openagents").join("skills")
}

pub fn ensure_required_cad_skills() -> Result<Vec<ManagedSkillAttachment>, String> {
    ensure_required_skills_in(
        &managed_skills_root(),
        MANAGED_CAD_SKILLS,
        managed_skills_remote_fetch_enabled(),
        true,
    )
}

pub fn ensure_required_data_market_skills() -> Result<Vec<ManagedSkillAttachment>, String> {
    ensure_required_skills_in(
        &managed_skills_root(),
        MANAGED_DATA_MARKET_SKILLS,
        managed_skills_remote_fetch_enabled(),
        true,
    )
}

pub fn codex_extra_skill_roots(cwd: &Path) -> Vec<PathBuf> {
    codex_extra_skill_roots_with_managed_root(cwd, &managed_skills_root())
}

fn codex_extra_skill_roots_with_managed_root(cwd: &Path, managed_root: &Path) -> Vec<PathBuf> {
    let mut roots: BTreeSet<PathBuf> = BTreeSet::new();
    let repo_skills_root = cwd.join("skills");
    if repo_skills_root.is_absolute() && repo_skills_root.is_dir() {
        let _ = roots.insert(repo_skills_root);
    }

    if managed_root.is_absolute() && managed_root.is_dir() {
        let _ = roots.insert(managed_root.to_path_buf());
    }

    roots.into_iter().collect()
}

fn ensure_required_skills_in(
    managed_root: &Path,
    specs: &[ManagedSkillSpec],
    allow_remote_fetch: bool,
    write_existing_if_empty: bool,
) -> Result<Vec<ManagedSkillAttachment>, String> {
    fs::create_dir_all(managed_root).map_err(|error| {
        format!(
            "failed creating managed skills root {}: {error}",
            managed_root.display()
        )
    })?;

    let mut attachments = Vec::with_capacity(specs.len());
    for spec in specs {
        let skill_md_path = managed_root.join(spec.name).join("SKILL.md");
        ensure_skill_md_present(
            &skill_md_path,
            spec,
            allow_remote_fetch,
            write_existing_if_empty,
        )?;
        attachments.push(ManagedSkillAttachment {
            name: spec.name.to_string(),
            path: skill_md_path.display().to_string(),
        });
    }

    Ok(attachments)
}

fn ensure_skill_md_present(
    skill_md_path: &Path,
    spec: &ManagedSkillSpec,
    allow_remote_fetch: bool,
    write_existing_if_empty: bool,
) -> Result<(), String> {
    if skill_md_path.is_file() {
        let existing = fs::read_to_string(skill_md_path).map_err(|error| {
            format!(
                "failed reading managed skill {}: {error}",
                skill_md_path.display()
            )
        })?;
        if !existing.trim().is_empty() || !write_existing_if_empty {
            return Ok(());
        }
    }

    if let Some(parent) = skill_md_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed creating managed skill directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let payload = if allow_remote_fetch {
        match fetch_skill_payload(spec.raw_url) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(
                    "managed CAD skill fetch failed for {} from {}: {}; falling back to embedded payload",
                    spec.name,
                    spec.raw_url,
                    error
                );
                spec.embedded_skill_md.to_string()
            }
        }
    } else {
        spec.embedded_skill_md.to_string()
    };

    fs::write(skill_md_path, payload).map_err(|error| {
        format!(
            "failed writing managed skill {}: {error}",
            skill_md_path.display()
        )
    })?;
    Ok(())
}

fn fetch_skill_payload(raw_url: &str) -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(MANAGED_SKILLS_REMOTE_TIMEOUT_SECS))
        .build()
        .map_err(|error| format!("failed creating HTTP client: {error}"))?;
    let response = client
        .get(raw_url)
        .header("user-agent", "openagents-autopilot-desktop")
        .send()
        .map_err(|error| format!("request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("unexpected HTTP status {}", response.status()));
    }
    let payload = response
        .text()
        .map_err(|error| format!("failed reading response body: {error}"))?;
    if payload.trim().is_empty() {
        return Err("fetched payload was empty".to_string());
    }
    Ok(payload)
}

fn managed_skills_remote_fetch_enabled() -> bool {
    match std::env::var(MANAGED_SKILLS_REMOTE_FETCH_ENV) {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "off" | "no")
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{codex_extra_skill_roots_with_managed_root, ensure_required_skills_in};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let serial = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}-{serial}"));
        fs::create_dir_all(&path).unwrap_or_default();
        path
    }

    #[test]
    fn ensure_required_cad_skills_writes_embedded_fallback_payloads() {
        let managed_root = temp_dir("openagents-managed-cad-skills");
        let ensured = ensure_required_skills_in(
            &managed_root,
            super::MANAGED_CAD_SKILLS,
            false,
            true,
        )
        .expect("managed CAD skills should be provisioned");
        assert_eq!(ensured.len(), 2);
        for skill in ensured {
            let content = fs::read_to_string(&skill.path)
                .expect("managed CAD SKILL.md should be readable after provisioning");
            assert!(!content.trim().is_empty());
            assert!(content.contains("name:"));
        }

        let _ = fs::remove_dir_all(managed_root);
    }

    #[test]
    fn ensure_required_data_market_skills_writes_embedded_fallback_payloads() {
        let managed_root = temp_dir("openagents-managed-data-market-skills");
        let ensured = ensure_required_skills_in(
            &managed_root,
            super::MANAGED_DATA_MARKET_SKILLS,
            false,
            true,
        )
        .expect("managed data market skills should be provisioned");
        assert_eq!(ensured.len(), 2);
        for skill in ensured {
            let content = fs::read_to_string(&skill.path)
                .expect("managed Data Market SKILL.md should be readable after provisioning");
            assert!(!content.trim().is_empty());
            assert!(content.contains("name:"));
        }

        let _ = fs::remove_dir_all(managed_root);
    }

    #[test]
    fn codex_extra_skill_roots_includes_repo_and_managed_when_present() {
        let cwd = temp_dir("openagents-cwd");
        let repo_skills = cwd.join("skills");
        let _ = fs::create_dir_all(&repo_skills);

        let managed_root = temp_dir("openagents-managed");
        let _ = fs::create_dir_all(&managed_root);

        let roots = codex_extra_skill_roots_with_managed_root(&cwd, &managed_root);

        assert!(roots.contains(&repo_skills));
        assert!(roots.contains(&managed_root));

        let _ = fs::remove_dir_all(cwd);
        let _ = fs::remove_dir_all(managed_root);
    }
}
