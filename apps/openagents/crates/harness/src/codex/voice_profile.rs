//! Voice reads authentication alone from the user's Codex home.
use super::*;
use std::path::Path;

pub(super) const INSTRUCTIONS: &str = "This is an OpenAgents voice call. Use only the OpenAgents MCP tools and the call instructions. Keep spoken replies short and conversational.";

pub(super) fn thread_config() -> Value {
    json!({
        "project_doc_max_bytes": 0, "project_root_markers": [],
        "features.plugins": false, "features.apps": false,
        "features.memories": false, "features.multi_agent": false,
        "features.multi_agent_v2": false, "agents.enabled": false,
        "features.browser_use": false, "features.computer_use": false,
        "features.js_repl": false, "features.image_generation": false,
        "features.request_permissions_tool": false,
        "skills.bundled.enabled": false, "skills.include_instructions": false,
        "features.shell_tool": false, "features.apply_patch_freeform": false,
        "web_search": "disabled"
    })
}

pub(super) struct Profile(PathBuf);
impl Profile {
    pub(super) fn prepare(
        cmd: &mut Command,
        cwd: &str,
        auth_home: Option<&Path>,
    ) -> Result<Self, HarnessError> {
        let root = Path::new(cwd).join(format!("voice-runtime-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root)?;
        let profile = Self(root);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&profile.0, std::fs::Permissions::from_mode(0o700))?;
        }
        let home = profile.0.join("home");
        let codex = profile.0.join("codex");
        std::fs::create_dir(&home)?;
        std::fs::create_dir(&codex)?;
        let source = auth_home
            .map(Path::to_path_buf)
            .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
            .or_else(|| crate::executable::home_dir().map(|h| h.join(".codex")));
        if let Some(source) = source {
            match std::fs::copy(source.join("auth.json"), codex.join("auth.json")) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        cmd.env("CODEX_HOME", &codex)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("BROWSER", "/usr/bin/true");
        for (key, value) in thread_config().as_object().unwrap() {
            cmd.args(["-c", &format!("{key}={value}")]);
        }
        Ok(profile)
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_only_authentication_into_a_private_fresh_home() {
        let fixture = tempfile::tempdir().unwrap();
        let source = fixture.path().join("user");
        std::fs::create_dir_all(source.join("plugins")).unwrap();
        std::fs::write(source.join("auth.json"), "fixture-auth").unwrap();
        std::fs::write(
            source.join("config.toml"),
            "developer_instructions = 'leaked'",
        )
        .unwrap();
        std::fs::write(source.join("AGENTS.md"), "leaked").unwrap();
        let mut cmd = Command::new("fake");
        let profile =
            Profile::prepare(&mut cmd, fixture.path().to_str().unwrap(), Some(&source)).unwrap();
        let codex = profile.0.join("codex");
        assert_eq!(
            std::fs::read_to_string(codex.join("auth.json")).unwrap(),
            "fixture-auth"
        );
        assert_eq!(std::fs::read_dir(&codex).unwrap().count(), 1);
        assert!(
            std::fs::read_dir(profile.0.join("home"))
                .unwrap()
                .next()
                .is_none()
        );
        let path = profile.0.clone();
        drop(profile);
        assert!(!path.exists());
    }
}
