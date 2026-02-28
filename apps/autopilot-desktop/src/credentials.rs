use std::collections::HashMap;
use std::path::PathBuf;

pub const CREDENTIAL_SCOPE_CODEX: u8 = 1 << 0;
pub const CREDENTIAL_SCOPE_SPARK: u8 = 1 << 1;
pub const CREDENTIAL_SCOPE_SKILLS: u8 = 1 << 2;
pub const CREDENTIAL_SCOPE_GLOBAL: u8 = 1 << 3;

pub const CREDENTIAL_SCOPE_ALL: u8 = CREDENTIAL_SCOPE_CODEX
    | CREDENTIAL_SCOPE_SPARK
    | CREDENTIAL_SCOPE_SKILLS
    | CREDENTIAL_SCOPE_GLOBAL;

const CREDENTIALS_SCHEMA_VERSION: u16 = 1;
const CREDENTIALS_KEYRING_SERVICE: &str = "com.openagents.autopilot.credentials";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialTemplate {
    pub name: &'static str,
    pub secret: bool,
    pub scopes: u8,
}

pub const CREDENTIAL_TEMPLATES: [CredentialTemplate; 7] = [
    CredentialTemplate {
        name: "OPENAI_API_KEY",
        secret: true,
        scopes: CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_SKILLS | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "OPENAI_ACCESS_TOKEN",
        secret: true,
        scopes: CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "OPENAI_CHATGPT_ACCOUNT_ID",
        secret: true,
        scopes: CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "OPENAI_CHATGPT_PLAN_TYPE",
        secret: false,
        scopes: CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "OPENAGENTS_SPARK_API_KEY",
        secret: true,
        scopes: CREDENTIAL_SCOPE_SPARK | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "BLINK_API_KEY",
        secret: true,
        scopes: CREDENTIAL_SCOPE_SKILLS | CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_GLOBAL,
    },
    CredentialTemplate {
        name: "BLINK_API_URL",
        secret: false,
        scopes: CREDENTIAL_SCOPE_SKILLS | CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_GLOBAL,
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialRecord {
    pub name: String,
    pub enabled: bool,
    pub secret: bool,
    pub template: bool,
    pub scopes: u8,
    pub has_value: bool,
}

impl CredentialRecord {
    pub fn applies_to_scope(&self, scope: u8) -> bool {
        (self.scopes & scope) != 0
    }
}

#[derive(Default)]
pub struct CredentialRepository {
    metadata_path: PathBuf,
}

impl CredentialRepository {
    pub fn new() -> Self {
        Self {
            metadata_path: credentials_file_path(),
        }
    }

    pub fn load_records(&self) -> Result<Vec<CredentialRecord>, String> {
        let mut records = match std::fs::read_to_string(&self.metadata_path) {
            Ok(raw) => parse_credentials_metadata(&raw)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                return Err(format!(
                    "Failed to read credential metadata {}: {error}",
                    self.metadata_path.display()
                ));
            }
        };
        merge_template_records(&mut records);
        sort_records(&mut records);

        for record in &mut records {
            record.has_value = self.read_value(record.name.as_str())?.is_some();
        }
        Ok(records)
    }

    pub fn persist_records(&self, records: &[CredentialRecord]) -> Result<(), String> {
        if let Some(parent) = self.metadata_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create credential metadata directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let mut ordered = records.to_vec();
        sort_records(&mut ordered);
        let serialized = serialize_credentials_metadata(ordered.as_slice());
        std::fs::write(&self.metadata_path, serialized).map_err(|error| {
            format!(
                "Failed to persist credential metadata {}: {error}",
                self.metadata_path.display()
            )
        })?;
        Ok(())
    }

    pub fn set_value(&self, name: &str, value: &str) -> Result<(), String> {
        let normalized = normalize_env_var_name(name);
        if !is_valid_env_var_name(normalized.as_str()) {
            return Err("Credential name must match [A-Z_][A-Z0-9_]*".to_string());
        }
        if value.trim().is_empty() {
            return Err(format!(
                "Credential {} value cannot be empty",
                normalized.as_str()
            ));
        }
        let entry = keyring_entry(normalized.as_str())?;
        entry
            .set_password(value)
            .map_err(|error| format!("Failed to store credential {}: {error}", normalized))?;
        Ok(())
    }

    pub fn delete_value(&self, name: &str) -> Result<(), String> {
        let normalized = normalize_env_var_name(name);
        let entry = keyring_entry(normalized.as_str())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(format!(
                "Failed to delete credential {} from secure storage: {error}",
                normalized
            )),
        }
    }

    pub fn read_value(&self, name: &str) -> Result<Option<String>, String> {
        let normalized = normalize_env_var_name(name);
        let env_fallback = std::env::var(normalized.as_str())
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let entry = keyring_entry(normalized.as_str())?;
        match entry.get_password() {
            Ok(value) => {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    Ok(env_fallback)
                } else {
                    Ok(Some(trimmed))
                }
            }
            Err(keyring::Error::NoEntry) => Ok(env_fallback),
            Err(error) => {
                if env_fallback.is_some() {
                    Ok(env_fallback)
                } else {
                    Err(format!(
                        "Failed to read credential {} from secure storage: {error}",
                        normalized
                    ))
                }
            }
        }
    }

    pub fn resolve_env_for_scope(
        &self,
        records: &[CredentialRecord],
        scope: u8,
    ) -> Result<Vec<(String, String)>, String> {
        let mut merged = HashMap::<String, String>::new();
        for record in records {
            if !record.enabled {
                continue;
            }
            if !record.applies_to_scope(scope) && !record.applies_to_scope(CREDENTIAL_SCOPE_GLOBAL)
            {
                continue;
            }
            if let Some(value) = self.read_value(record.name.as_str())? {
                merged.insert(record.name.clone(), value);
            }
        }

        let mut pairs: Vec<(String, String)> = merged.into_iter().collect();
        pairs.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        Ok(pairs)
    }
}

pub fn is_valid_env_var_name(raw: &str) -> bool {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first.is_ascii_uppercase()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub fn normalize_env_var_name(raw: &str) -> String {
    raw.trim().to_ascii_uppercase()
}

pub fn infer_secret_from_name(name: &str) -> bool {
    let normalized = normalize_env_var_name(name);
    normalized.ends_with("_KEY")
        || normalized.ends_with("_TOKEN")
        || normalized.contains("SECRET")
        || normalized.contains("PASSWORD")
}

fn keyring_entry(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(CREDENTIALS_KEYRING_SERVICE, name)
        .map_err(|error| format!("Credential store unavailable for {name}: {error}"))
}

fn credentials_file_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot-credentials-v1.conf")
}

fn serialize_credentials_metadata(records: &[CredentialRecord]) -> String {
    let mut lines = vec![format!("schema_version={CREDENTIALS_SCHEMA_VERSION}")];
    for record in records {
        lines.push(format!(
            "entry={}|{}|{}|{}|{}",
            record.name,
            bool_flag(record.enabled),
            bool_flag(record.secret),
            bool_flag(record.template),
            record.scopes & CREDENTIAL_SCOPE_ALL,
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn parse_credentials_metadata(raw: &str) -> Result<Vec<CredentialRecord>, String> {
    let mut schema_version = CREDENTIALS_SCHEMA_VERSION;
    let mut records = Vec::<CredentialRecord>::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(format!("Invalid credential metadata line: {trimmed}"));
        };
        match key.trim() {
            "schema_version" => {
                schema_version = value
                    .trim()
                    .parse::<u16>()
                    .map_err(|error| format!("Invalid credential schema_version: {error}"))?;
            }
            "entry" => {
                let mut parts = value.split('|');
                let Some(name_raw) = parts.next() else {
                    return Err("Invalid credential entry: missing name".to_string());
                };
                let Some(enabled_raw) = parts.next() else {
                    return Err(format!(
                        "Invalid credential entry {}: missing enabled flag",
                        name_raw
                    ));
                };
                let Some(secret_raw) = parts.next() else {
                    return Err(format!(
                        "Invalid credential entry {}: missing secret flag",
                        name_raw
                    ));
                };
                let Some(template_raw) = parts.next() else {
                    return Err(format!(
                        "Invalid credential entry {}: missing template flag",
                        name_raw
                    ));
                };
                let Some(scopes_raw) = parts.next() else {
                    return Err(format!(
                        "Invalid credential entry {}: missing scopes",
                        name_raw
                    ));
                };
                if parts.next().is_some() {
                    return Err(format!(
                        "Invalid credential entry {}: too many fields",
                        name_raw
                    ));
                }

                let normalized_name = normalize_env_var_name(name_raw);
                if !is_valid_env_var_name(normalized_name.as_str()) {
                    return Err(format!(
                        "Invalid credential name {} (expected [A-Z_][A-Z0-9_]*)",
                        name_raw
                    ));
                }

                let enabled = parse_bool_flag(enabled_raw)?;
                let secret = parse_bool_flag(secret_raw)?;
                let template = parse_bool_flag(template_raw)?;
                let scopes = scopes_raw.trim().parse::<u8>().map_err(|error| {
                    format!(
                        "Invalid credential scopes for {}: {error}",
                        normalized_name.as_str()
                    )
                })? & CREDENTIAL_SCOPE_ALL;
                records.push(CredentialRecord {
                    name: normalized_name,
                    enabled,
                    secret,
                    template,
                    scopes,
                    has_value: false,
                });
            }
            _ => {}
        }
    }

    if schema_version != CREDENTIALS_SCHEMA_VERSION {
        return Err(format!(
            "Unsupported credential schema version {schema_version}, expected {}",
            CREDENTIALS_SCHEMA_VERSION
        ));
    }

    dedupe_records(&mut records);
    Ok(records)
}

fn parse_bool_flag(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "1" | "true" => Ok(true),
        "0" | "false" => Ok(false),
        other => Err(format!("Invalid boolean flag {other}")),
    }
}

fn bool_flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn merge_template_records(records: &mut Vec<CredentialRecord>) {
    dedupe_records(records);
    for template in CREDENTIAL_TEMPLATES {
        if records.iter().any(|record| record.name == template.name) {
            continue;
        }
        records.push(CredentialRecord {
            name: template.name.to_string(),
            enabled: true,
            secret: template.secret,
            template: true,
            scopes: template.scopes,
            has_value: false,
        });
    }
}

fn dedupe_records(records: &mut Vec<CredentialRecord>) {
    let mut seen = std::collections::BTreeSet::<String>::new();
    records.retain(|record| seen.insert(record.name.clone()));
}

fn sort_records(records: &mut [CredentialRecord]) {
    records.sort_by(|lhs, rhs| record_sort_key(lhs).cmp(&record_sort_key(rhs)));
}

fn record_sort_key(record: &CredentialRecord) -> (u8, usize, String) {
    if let Some((index, _template)) = CREDENTIAL_TEMPLATES
        .iter()
        .enumerate()
        .find(|(_, template)| template.name == record.name)
    {
        (0, index, record.name.clone())
    } else {
        (1, usize::MAX, record.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CREDENTIAL_SCOPE_CODEX, CREDENTIAL_SCOPE_GLOBAL, CREDENTIAL_SCOPE_SKILLS,
        CREDENTIAL_SCOPE_SPARK, CREDENTIAL_TEMPLATES, CredentialRecord, is_valid_env_var_name,
        normalize_env_var_name, parse_credentials_metadata, serialize_credentials_metadata,
    };

    #[test]
    fn env_var_name_validation_rejects_invalid_shapes() {
        assert!(is_valid_env_var_name("OPENAI_API_KEY"));
        assert!(is_valid_env_var_name("_CUSTOM_ENV"));
        assert!(!is_valid_env_var_name("openai_api_key"));
        assert!(!is_valid_env_var_name("1OPENAI_API_KEY"));
        assert!(!is_valid_env_var_name("OPENAI-API-KEY"));
        assert_eq!(normalize_env_var_name("  blink_api_key "), "BLINK_API_KEY");
    }

    #[test]
    fn metadata_round_trip_keeps_scopes_and_flags() {
        let records = vec![CredentialRecord {
            name: "FLAMP_API_KEY".to_string(),
            enabled: true,
            secret: true,
            template: false,
            scopes: CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_SKILLS | CREDENTIAL_SCOPE_GLOBAL,
            has_value: true,
        }];
        let serialized = serialize_credentials_metadata(records.as_slice());
        let parsed =
            parse_credentials_metadata(serialized.as_str()).expect("metadata parse should succeed");
        assert_eq!(parsed[0].name, "FLAMP_API_KEY");
        assert!(parsed[0].enabled);
        assert!(parsed[0].secret);
        assert!(!parsed[0].template);
        assert_eq!(
            parsed[0].scopes,
            CREDENTIAL_SCOPE_CODEX | CREDENTIAL_SCOPE_SKILLS | CREDENTIAL_SCOPE_GLOBAL
        );
    }

    #[test]
    fn metadata_parse_rejects_invalid_schema() {
        let raw = "schema_version=999\n";
        let error = parse_credentials_metadata(raw)
            .err()
            .unwrap_or_else(|| "expected error".to_string());
        assert!(error.contains("Unsupported credential schema version"));
    }

    #[test]
    fn metadata_parse_rejects_invalid_names() {
        let raw = "schema_version=1\nentry=bad-name|1|1|0|1\n";
        let error = parse_credentials_metadata(raw)
            .err()
            .unwrap_or_else(|| "expected error".to_string());
        assert!(error.contains("Invalid credential name"));
    }

    #[test]
    fn template_scope_contracts_cover_primary_services() {
        let mut has_codex = false;
        let mut has_spark = false;
        let mut has_skills = false;
        for template in CREDENTIAL_TEMPLATES {
            has_codex |= (template.scopes & CREDENTIAL_SCOPE_CODEX) != 0;
            has_spark |= (template.scopes & CREDENTIAL_SCOPE_SPARK) != 0;
            has_skills |= (template.scopes & CREDENTIAL_SCOPE_SKILLS) != 0;
        }
        assert!(has_codex);
        assert!(has_spark);
        assert!(has_skills);
    }
}
