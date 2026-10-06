//! Startup credential imports from explicit environment and filesystem roots.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use model_access::ApiKey;

const NAMES: [&str; 3] = [
    "OPENROUTER_API_KEY",
    "TYPESAFE_API_KEY",
    "TYPESAFE_BASE_URL",
];
const MAX_BYTES: usize = 64 * 1024;
const READ_ERROR: &str = "Cannot read the plugin credential source.";
const INVALID_ERROR: &str = "The plugin credential source is invalid.";

#[derive(Default, Debug)]
pub struct Imported {
    pub openrouter_key: Option<ApiKey>,
    pub jev_key: Option<ApiKey>,
    pub jev_endpoint: Option<String>,
}

/// Read only the declared keys, with process values before `.env` and key files.
/// This function does not discover a home directory, mutate the process, or write files.
pub fn load(
    cwd: &Path,
    openagents_root: Option<&Path>,
    get_env: impl Fn(&str) -> Option<String>,
) -> Result<Imported, String> {
    let dotenv = read(cwd.join(".env").as_path())?
        .map(|bytes| {
            let text = std::str::from_utf8(&bytes.0).map_err(|_| INVALID_ERROR.to_owned())?;
            parse_dotenv(text)
        })
        .transpose()?
        .unwrap_or_default();
    let value = |index: usize| {
        get_env(NAMES[index])
            .filter(|value| !value.trim().is_empty())
            .or_else(|| dotenv[index].clone())
    };
    let mut openrouter_key = key(value(0))?;
    if openrouter_key.is_none() {
        if let Some(root) = openagents_root {
            if let Some(bytes) = read(&root.join("openrouter.json"))? {
                let document: serde_json::Value =
                    serde_json::from_slice(&bytes.0).map_err(|_| INVALID_ERROR.to_owned())?;
                if !document.is_object() {
                    return Err(INVALID_ERROR.into());
                }
                let value = match document.get("api_key") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(serde_json::Value::String(text)) => Some(text.clone()),
                    _ => return Err(INVALID_ERROR.into()),
                };
                openrouter_key = key(value)?;
            }
        }
    }
    let jev_endpoint = value(2).map(|value| value.trim().to_owned());
    if jev_endpoint
        .as_deref()
        .is_some_and(|endpoint| !valid_endpoint(endpoint))
    {
        return Err(INVALID_ERROR.into());
    }
    Ok(Imported {
        openrouter_key,
        jev_key: key(value(1))?,
        jev_endpoint,
    })
}

fn key(value: Option<String>) -> Result<Option<ApiKey>, String> {
    match value {
        None => Ok(None),
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => {
            let value = value.trim();
            if value.len() > 16 * 1024
                || !value.is_ascii()
                || value
                    .chars()
                    .any(|character| character.is_control() || character.is_whitespace())
            {
                return Err(INVALID_ERROR.into());
            }
            Ok(Some(ApiKey::new(value)))
        }
    }
}

fn valid_endpoint(endpoint: &str) -> bool {
    if endpoint.len() > 2048 {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    let loopback = url.host_str().is_some_and(|host| {
        host.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    (url.scheme() == "https" || url.scheme() == "http" && loopback)
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn parse_dotenv(text: &str) -> Result<[Option<String>; 3], String> {
    let mut values = [None, None, None];
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((name, value)) = line.split_once('=') else {
            if NAMES.contains(&line) {
                return Err(INVALID_ERROR.into());
            }
            continue;
        };
        let Some(index) = NAMES.iter().position(|allowed| *allowed == name.trim()) else {
            continue;
        };
        let value = value.trim();
        let value = if let Some(quote) = value
            .chars()
            .next()
            .filter(|quote| matches!(quote, '\'' | '"'))
        {
            let tail = &value[quote.len_utf8()..];
            let end = tail.find(quote).ok_or(INVALID_ERROR)?;
            let trailing = tail[end + quote.len_utf8()..].trim();
            if !trailing.is_empty() && !trailing.starts_with('#') {
                return Err(INVALID_ERROR.into());
            }
            &tail[..end]
        } else {
            let end = value
                .char_indices()
                .find_map(|(index, character)| {
                    (character == '#'
                        && (index == 0 || value[..index].ends_with(char::is_whitespace)))
                    .then_some(index)
                })
                .unwrap_or(value.len());
            value[..end].trim_end()
        };
        values[index] = (!value.is_empty()).then(|| value.to_owned());
    }
    Ok(values)
}

struct PrivateBytes(Vec<u8>);
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

fn read(path: &Path) -> Result<Option<PrivateBytes>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(READ_ERROR.into()),
        Ok(metadata) => metadata,
    };
    if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
        return Err(INVALID_ERROR.into());
    }
    let mut bytes = PrivateBytes(Vec::new());
    File::open(path)
        .map_err(|_| READ_ERROR.to_owned())?
        .take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes.0)
        .map_err(|_| READ_ERROR.to_owned())?;
    if bytes.0.len() > MAX_BYTES {
        return Err(INVALID_ERROR.into());
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_handles_quotes_comments_export_and_only_declared_names() {
        let values = parse_dotenv("# comment\nexport OPENROUTER_API_KEY = 'dotenv-key' # ignored\nTYPESAFE_API_KEY=jev-key#literal\nTYPESAFE_BASE_URL=\"https://example.invalid\"\nUNRELATED=$(touch /tmp/never)\n").unwrap();
        assert_eq!(
            values,
            [
                Some("dotenv-key".into()),
                Some("jev-key#literal".into()),
                Some("https://example.invalid".into())
            ]
        );
        assert_eq!(
            parse_dotenv("OPENROUTER_API_KEY=${LITERAL}").unwrap()[0].as_deref(),
            Some("${LITERAL}")
        );
        assert!(parse_dotenv("OPENROUTER_API_KEY='unterminated").is_err());
        assert!(parse_dotenv("OPENROUTER_API_KEY='key' run-command").is_err());
    }

    #[test]
    fn process_values_override_dotenv_and_dotenv_overrides_legacy_key_file() {
        let temporary = tempfile::tempdir().unwrap();
        let cwd = temporary.path().join("project");
        let root = temporary.path().join("openagents");
        fs::create_dir(&cwd).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("openrouter.json"),
            r#"{"api_key":"file-key","label":"Retained settings"}"#,
        )
        .unwrap();
        fs::write(
            cwd.join(".env"),
            "OPENROUTER_API_KEY=dotenv-key\nTYPESAFE_API_KEY=jev-dotenv\n",
        )
        .unwrap();
        let loaded = load(&cwd, Some(&root), |name| {
            (name == "OPENROUTER_API_KEY").then(|| "process-key".into())
        })
        .unwrap();
        assert_eq!(loaded.openrouter_key.unwrap().expose(), "process-key");
        assert_eq!(loaded.jev_key.unwrap().expose(), "jev-dotenv");
        assert_eq!(
            load(&cwd, Some(&root), |_| None)
                .unwrap()
                .openrouter_key
                .unwrap()
                .expose(),
            "dotenv-key"
        );
        fs::remove_file(cwd.join(".env")).unwrap();
        assert_eq!(
            load(&cwd, Some(&root), |_| None)
                .unwrap()
                .openrouter_key
                .unwrap()
                .expose(),
            "file-key"
        );
    }

    #[test]
    fn missing_files_are_empty_and_invalid_sources_never_echo_values() {
        let temporary = tempfile::tempdir().unwrap();
        assert!(
            load(temporary.path(), Some(temporary.path()), |_| None)
                .unwrap()
                .openrouter_key
                .is_none()
        );
        fs::write(
            temporary.path().join(".env"),
            "OPENROUTER_API_KEY='private value'",
        )
        .unwrap();
        assert_eq!(
            load(temporary.path(), None, |_| None).unwrap_err(),
            INVALID_ERROR
        );
        fs::write(temporary.path().join(".env"), vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert_eq!(
            load(temporary.path(), None, |_| None).unwrap_err(),
            INVALID_ERROR
        );
    }

    #[cfg(unix)]
    #[test]
    fn credential_imports_do_not_follow_symlinks() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(
            temporary.path().join("other"),
            "OPENROUTER_API_KEY=fixture-key",
        )
        .unwrap();
        std::os::unix::fs::symlink(
            temporary.path().join("other"),
            temporary.path().join(".env"),
        )
        .unwrap();
        assert!(load(temporary.path(), None, |_| None).is_err());
    }
}
