//! Optional source-backed code guides. Offered only with an installed index.

use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};

pub const PLUGIN: &str = "code-index";
pub const DESCRIPTION: &str =
    "Find examples, the places to change, and related checks for a coding task.";
pub const ENV: &str = "OPENAGENTS_CODE_INDEX";
pub const INSTRUCTIONS: &str = code_index::runtime::INSTRUCTIONS;
static STATE: AtomicU8 = AtomicU8::new(0);

pub fn set_enabled(enabled: bool) {
    STATE.store(if enabled { 2 } else { 1 }, Ordering::Relaxed);
}
pub fn enabled() -> bool {
    match std::env::var(ENV).as_deref() {
        Ok("0") => return false,
        Ok("1") => return true,
        _ => {}
    }
    match STATE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ if cfg!(test) => false,
        _ => crate::home::data_dir()
            .is_some_and(|dir| crate::bundled_settings::saved_enabled(&dir, PLUGIN)),
    }
}
fn configuration() -> code_index::runtime::Configuration {
    code_index::runtime::Configuration {
        store: std::env::var_os("CODE_INDEX_STORE").map(Into::into),
        workspace: std::env::var("CODE_INDEX_WORKSPACE").unwrap_or_else(|_| "local".into()),
    }
}
pub fn offered(cwd: &Path) -> bool {
    enabled() && code_index::runtime::available(cwd, &configuration())
}
pub fn definitions() -> Vec<Value> {
    vec![code_index::runtime::definition()]
}
pub fn is_tool(name: &str) -> bool {
    name == code_index::runtime::TOOL
}
pub async fn execute(name: &str, arguments: Value, cwd: &Path) -> Result<Value, String> {
    if !is_tool(name) {
        return Err("This is not a code guide tool.".into());
    }
    let cwd = cwd.to_path_buf();
    let config = configuration();
    tokio::task::spawn_blocking(move || code_index::runtime::execute(&cwd, &arguments, &config))
        .await
        .map_err(|_| "The code guide query stopped.".to_owned())?
}

#[cfg(test)]
mod tests {
    #[test]
    fn code_guide_has_one_typed_tool_and_no_placeholder_index() {
        assert!(super::is_tool(
            super::definitions()[0]["function"]["name"]
                .as_str()
                .unwrap()
        ));
        let dir = tempfile::tempdir().unwrap();
        assert!(!code_index::runtime::available(
            dir.path(),
            &Default::default()
        ));
    }
}
