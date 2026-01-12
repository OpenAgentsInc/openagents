use std::fs;
use std::path::{Path, PathBuf};

use wgpui::components::molecules::{SkillCategory, SkillInfo, SkillInstallStatus};

use super::super::parsing::{
    first_nonempty_line, frontmatter_list, frontmatter_scalar, parse_frontmatter, Frontmatter,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SkillSource {
    Project,
    User,
    Codex,
}

#[derive(Clone, Debug)]
pub(crate) struct SkillEntry {
    pub(crate) info: SkillInfo,
    pub(crate) source: SkillSource,
    pub(crate) path: PathBuf,
}

pub(crate) struct SkillCatalog {
    pub(crate) entries: Vec<SkillEntry>,
    pub(crate) error: Option<String>,
    pub(crate) project_path: Option<PathBuf>,
    pub(crate) user_path: Option<PathBuf>,
}

fn parse_price_sats(value: &str) -> Option<u64> {
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn parse_u32(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}

fn parse_f32(value: &str) -> Option<f32> {
    value.trim().parse().ok()
}

fn parse_skill_category(frontmatter: &Frontmatter) -> SkillCategory {
    let mut candidates = Vec::new();
    if let Some(value) = frontmatter_scalar(frontmatter, "category") {
        candidates.push(value);
    }
    if let Some(list) = frontmatter_list(frontmatter, "categories") {
        candidates.extend(list);
    }
    if let Some(list) = frontmatter_list(frontmatter, "tags") {
        candidates.extend(list);
    }

    for candidate in candidates {
        let normalized = candidate.to_ascii_lowercase();
        if normalized.contains("code") || normalized.contains("generation") {
            return SkillCategory::CodeGeneration;
        }
        if normalized.contains("data")
            || normalized.contains("analysis")
            || normalized.contains("analytics")
        {
            return SkillCategory::DataAnalysis;
        }
        if normalized.contains("web")
            || normalized.contains("browser")
            || normalized.contains("automation")
            || normalized.contains("scrape")
        {
            return SkillCategory::WebAutomation;
        }
        if normalized.contains("file")
            || normalized.contains("filesystem")
            || normalized.contains("document")
        {
            return SkillCategory::FileProcessing;
        }
        if normalized.contains("api") || normalized.contains("integration") || normalized.contains("http") {
            return SkillCategory::ApiIntegration;
        }
        if normalized.contains("text") || normalized.contains("nlp") || normalized.contains("writing") {
            return SkillCategory::TextProcessing;
        }
        if normalized.contains("image") || normalized.contains("vision") || normalized.contains("ocr") {
            return SkillCategory::ImageProcessing;
        }
    }

    SkillCategory::Other
}

fn skill_project_dir(cwd: &Path) -> PathBuf {
    cwd.join(".openagents").join("skills")
}

fn skill_user_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".openagents").join("skills"))
}

pub(crate) fn load_skill_entries(cwd: &Path) -> SkillCatalog {
    let project_dir = skill_project_dir(cwd);
    let user_dir = skill_user_dir();
    let mut errors = Vec::new();
    let mut entries = Vec::new();

    if let Some(user_dir) = user_dir.as_ref() {
        entries.extend(load_skill_dir(user_dir, SkillSource::User, &mut errors));
    }
    entries.extend(load_skill_dir(&project_dir, SkillSource::Project, &mut errors));

    entries.sort_by(|a, b| a.info.name.cmp(&b.info.name));

    SkillCatalog {
        entries,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join(" | "))
        },
        project_path: Some(project_dir),
        user_path: user_dir,
    }
}

fn load_skill_dir(dir: &Path, source: SkillSource, errors: &mut Vec<String>) -> Vec<SkillEntry> {
    if !dir.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            errors.push(format!("Failed to read {}: {}", dir.display(), err));
            return Vec::new();
        }
    };

    let mut skills = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                errors.push(format!("Failed to read skill entry: {}", err));
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_file = path.join("SKILL.md");
        let skill_file = if skill_file.is_file() {
            skill_file
        } else {
            let alt = path.join("skill.md");
            if alt.is_file() {
                alt
            } else {
                continue;
            }
        };
        match parse_skill_file(&skill_file, source) {
            Ok(Some(skill)) => skills.push(skill),
            Ok(None) => {}
            Err(err) => errors.push(err),
        }
    }
    skills
}

fn parse_skill_file(path: &Path, source: SkillSource) -> Result<Option<SkillEntry>, String> {
    let content = fs::read_to_string(path)
        .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
    let (frontmatter, body) = parse_frontmatter(&content);
    let folder_name = path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("skill");
    let name = frontmatter_scalar(&frontmatter, "name")
        .unwrap_or_else(|| folder_name.to_string());
    let description = frontmatter_scalar(&frontmatter, "description")
        .or_else(|| first_nonempty_line(&body))
        .unwrap_or_else(|| "No description provided.".to_string());

    let category = parse_skill_category(&frontmatter);
    let author =
        frontmatter_scalar(&frontmatter, "author").unwrap_or_else(|| "unknown".to_string());
    let version =
        frontmatter_scalar(&frontmatter, "version").unwrap_or_else(|| "1.0.0".to_string());
    let price = frontmatter_scalar(&frontmatter, "price_sats")
        .or_else(|| frontmatter_scalar(&frontmatter, "price"))
        .and_then(|value| parse_price_sats(&value));
    let downloads = frontmatter_scalar(&frontmatter, "downloads").and_then(|value| parse_u32(&value));
    let rating = frontmatter_scalar(&frontmatter, "rating").and_then(|value| parse_f32(&value));

    let id = match source {
        SkillSource::Project => format!("project:{}", folder_name),
        SkillSource::User => format!("user:{}", folder_name),
        SkillSource::Codex => format!("codex:{}", folder_name),
    };

    let mut info = SkillInfo::new(id, name, description)
        .status(SkillInstallStatus::Installed)
        .category(category)
        .author(author)
        .version(version);
    if let Some(price) = price {
        info = info.price(price);
    }
    if let Some(downloads) = downloads {
        info = info.downloads(downloads);
    }
    if let Some(rating) = rating {
        info = info.rating(rating);
    }

    Ok(Some(SkillEntry {
        info,
        source,
        path: path.to_path_buf(),
    }))
}
