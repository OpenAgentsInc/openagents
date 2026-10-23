//! The bounded, opt-in shader workspace used by the Haunted House preview.

pub const SCENE_PATH: &str = "haunt/scene.json";
pub const PATH: &str = "haunt/material.wgsl";
pub const MAX_BYTES: usize = 64 * 1024;
// A saved scene can include a shader draft with six-byte JSON escapes.
pub const MAX_PROJECT_BYTES: usize = 64 * 1024 + 6 * MAX_BYTES + 256;

/// The two creator labs keep separate workspace snapshots and editable files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lab {
    #[default]
    Haunt,
    Medieval,
}
impl Lab {
    pub fn parse(name: &str) -> Result<Self, &'static str> {
        match name {
            "haunt" => Ok(Self::Haunt),
            "medieval" => Ok(Self::Medieval),
            _ => Err("Choose Haunted House or Medieval Town Lab."),
        }
    }
    pub const fn shader(self) -> &'static str {
        match self {
            Self::Haunt => PATH,
            Self::Medieval => "medieval/material.wgsl",
        }
    }
    pub const fn scene(self) -> &'static str {
        match self {
            Self::Haunt => SCENE_PATH,
            Self::Medieval => "medieval/scene.json",
        }
    }
    pub const fn snapshot(self) -> &'static str {
        match self {
            Self::Haunt => "kiln-workspace-v1.bin",
            Self::Medieval => "kiln-medieval-workspace-v1.bin",
        }
    }
    pub const fn base_snapshot(self) -> &'static str {
        match self {
            Self::Haunt => "kiln-base-v1.bin",
            Self::Medieval => "kiln-medieval-base-v1.bin",
        }
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn workspace_source(vfs: &crate::vfs::Vfs, path: &str, lab: Lab) -> Option<String> {
    let bytes = vfs.read(path).ok()?;
    let limit = if path == lab.scene() {
        MAX_PROJECT_BYTES
    } else {
        MAX_BYTES
    };
    if bytes.len() > limit {
        return None;
    }
    // Empty drafts reach the validator without replacing the running preview.
    std::str::from_utf8(bytes).ok().map(str::to_owned)
}

/// Instructions for the existing Kiln agent, not a second agent loop.
pub const INSTRUCTIONS: &str = r#"You are Kiln editing the running Haunted House WebGPU preview.
The editable file is haunt/material.wgsl. It is WGSL, not Rust or Rhai.
Read it with {"run":"view haunt/material.wgsl 1 80"} before editing.
Use {"patch":"haunt/material.wgsl","search":"exact old text","replace":"new text"} for a small change.
Preserve the Scene struct, @group/@binding declarations, vertex inputs and entry points.
For lighting requests, change LANTERN_TINT only unless the person asks otherwise.
The browser validates the shader after every write. Do not run test, cargo, publish, or reload.
Reply with ONLY one JSON object: {"thought":"short explanation","commands":[COMMAND]}.
Commands are {"run":"view PATH START END"}, {"patch":"PATH","search":"exact text","replace":"new text"}, {"write":"PATH","content":"full text"}, or {"done":"short summary"}.
After making the requested edit, reply with a done command. Never change another file."#;

/// Reject empty and oversized drafts before writing to the workspace.
pub fn validate(source: &str) -> Result<(), &'static str> {
    if source.trim().is_empty() {
        Err("The shader is empty.")
    } else if source.len() > MAX_BYTES {
        Err("The shader is too large (maximum 64 KiB).")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labs_have_closed_names_and_separate_files_and_snapshots() {
        assert_eq!(Lab::parse("medieval"), Ok(Lab::Medieval));
        assert!(Lab::parse("../anything").is_err());
        assert_ne!(Lab::Haunt.scene(), Lab::Medieval.scene());
        assert_ne!(Lab::Haunt.shader(), Lab::Medieval.shader());
        assert_ne!(Lab::Haunt.snapshot(), Lab::Medieval.snapshot());
        assert_ne!(Lab::Haunt.base_snapshot(), Lab::Medieval.base_snapshot());
    }
    #[test]
    fn bounds_are_bytes_and_invalid_wgsl_can_reach_gpu_diagnostics() {
        assert!(validate("  \n").is_err());
        assert!(validate(&"é".repeat(MAX_BYTES / 2 + 1)).is_err());
        assert!(validate("this is a syntax error").is_ok());
    }

    #[test]
    fn escaped_project_drafts_survive_workspace_reopen_in_both_labs() {
        for lab in [Lab::Haunt, Lab::Medieval] {
            let draft = "\0".repeat(MAX_BYTES);
            let project = serde_json::json!({"materialDraft": {"source": draft}}).to_string();
            assert!(project.len() > MAX_BYTES);
            assert!(project.len() <= MAX_PROJECT_BYTES);
            let mut vfs = crate::vfs::Vfs::default();
            vfs.put(lab.scene(), project.as_bytes(), 1).unwrap();
            let reopened = crate::vfs::Vfs::decode(&vfs.encode()).unwrap();
            assert_eq!(workspace_source(&reopened, lab.scene(), lab), Some(project));
        }
    }

    #[test]
    fn only_the_selected_scene_gets_the_project_byte_limit() {
        for (lab, other) in [(Lab::Haunt, Lab::Medieval), (Lab::Medieval, Lab::Haunt)] {
            let mut vfs = crate::vfs::Vfs::default();
            let project = "x".repeat(MAX_PROJECT_BYTES);
            vfs.put(lab.scene(), project.as_bytes(), 1).unwrap();
            assert_eq!(
                workspace_source(&vfs, lab.scene(), lab),
                Some(project.clone())
            );
            for path in [other.scene(), lab.shader(), "notes.txt"] {
                vfs.put(path, project.as_bytes(), 1).unwrap();
                assert!(workspace_source(&vfs, path, lab).is_none());
            }
            vfs.put(lab.scene(), &vec![b'x'; MAX_PROJECT_BYTES + 1], 2)
                .unwrap();
            assert!(workspace_source(&vfs, lab.scene(), lab).is_none());
        }
    }

    #[test]
    fn workspace_source_keeps_shader_utf8_and_empty_draft_rules() {
        for lab in [Lab::Haunt, Lab::Medieval] {
            let mut vfs = crate::vfs::Vfs::default();
            let source = "é".repeat(MAX_BYTES / 2);
            vfs.put(lab.shader(), source.as_bytes(), 1).unwrap();
            assert_eq!(workspace_source(&vfs, lab.shader(), lab), Some(source));
            vfs.put(lab.shader(), "é".repeat(MAX_BYTES / 2 + 1).as_bytes(), 2)
                .unwrap();
            assert!(workspace_source(&vfs, lab.shader(), lab).is_none());
            vfs.put(lab.shader(), b"", 3).unwrap();
            assert_eq!(
                workspace_source(&vfs, lab.shader(), lab),
                Some(String::new())
            );
            vfs.put(lab.scene(), b"\xff", 4).unwrap();
            assert!(workspace_source(&vfs, lab.scene(), lab).is_none());
            assert!(workspace_source(&vfs, "../scene.json", lab).is_none());
            assert!(workspace_source(&vfs, "missing.wgsl", lab).is_none());
        }
    }

    #[test]
    fn shader_agent_uses_the_existing_patch_parser() {
        let plan = crate::agent::parse(
            r#"{"thought":"Warmer light","commands":[{"patch":"haunt/material.wgsl","search":"vec3<f32>(1.0, 0.72, 0.36)","replace":"vec3<f32>(1.0, 0.4, 0.12)"}]}"#,
        )
        .unwrap();
        assert!(
            matches!(&plan.commands[0], crate::agent::Command::Patch { path, .. } if path == PATH)
        );
    }
}
