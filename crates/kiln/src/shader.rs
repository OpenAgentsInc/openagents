//! The bounded, opt-in shader workspace used by the Haunted House preview.

pub const PATH: &str = "haunt/material.wgsl";
pub const MAX_BYTES: usize = 64 * 1024;

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
    fn bounds_are_bytes_and_invalid_wgsl_can_reach_gpu_diagnostics() {
        assert!(validate("  \n").is_err());
        assert!(validate(&"é".repeat(MAX_BYTES / 2 + 1)).is_err());
        assert!(validate("this is a syntax error").is_ok());
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
