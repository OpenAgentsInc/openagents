//! The way out of the offline demo (#12011). The demo shows a chip at the
//! top of the shell and a row in Settings; both lead back to the person's
//! own chats without restarting the app. Rust owns their words and forgets
//! what the demo saved, so nothing from it reaches the account.
use std::path::Path;

/// The words of the demo's way out.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DemoExitCopy {
    /// The chip at the top of the shell.
    pub chip: String,
    /// What the chip does, read aloud by VoiceOver.
    pub chip_hint: String,
    /// The Settings row.
    pub settings_title: String,
    pub settings_detail: String,
}

/// The chip's and the Settings row's words.
#[uniffi::export]
pub fn demo_exit_copy() -> DemoExitCopy {
    DemoExitCopy {
        chip: "Demo · Exit".into(),
        chip_hint: "Leaves the demo and goes back to your own chats.".into(),
        settings_title: "Exit demo".into(),
        settings_detail: "Back to your own chats".into(),
    }
}

/// The folder name the app keeps the demo's data in. Only a folder with
/// this name is ever removed by [`forget_demo_data`].
pub const DEMO_DIR_NAME: &str = "demo";

/// Removes the demo's data folder after the demo client has stopped, so
/// the next demo starts fresh and nothing the demo saved stays on the
/// phone. Returns whether the folder is gone.
///
/// It refuses any folder not named [`DEMO_DIR_NAME`], so a wrong path can
/// never remove the account's own data.
#[uniffi::export]
pub fn forget_demo_data(demo_dir: String) -> bool {
    let dir = Path::new(&demo_dir);
    if dir.file_name().and_then(|n| n.to_str()) != Some(DEMO_DIR_NAME) {
        return false;
    }
    match std::fs::remove_dir_all(dir) {
        Ok(()) => true,
        Err(e) => e.kind() == std::io::ErrorKind::NotFound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_ffi::{
        ClientEvent, ClientListener, CoreClient, CoreConfig, Credentials, DemoFixture, DemoOptions,
        StreamSpeed, TranscriptScale,
    };
    use std::sync::Arc;

    struct Quiet;
    impl ClientListener for Quiet {
        fn on_event(&self, _event: ClientEvent) {}
    }

    fn config(dir: &Path) -> CoreConfig {
        CoreConfig {
            edge_url: "https://edge.invalid".into(),
            data_dir: dir.display().to_string(),
            device_id: "ios-test".into(),
            device_name: "Test phone".into(),
            platform: "ios".into(),
            app_version: "0".into(),
        }
    }

    fn demo() -> Credentials {
        Credentials::Demo {
            options: DemoOptions {
                fixture: DemoFixture::Standard,
                transcript_scale: TranscriptScale::Normal,
                stream_speed: StreamSpeed::Fast,
                long_reply: false,
            },
        }
    }

    #[test]
    fn exiting_the_demo_forgets_it_and_keeps_the_account() {
        let support = tempfile::tempdir().unwrap();
        let core = support.path().join("core");
        std::fs::create_dir_all(&core).unwrap();
        std::fs::write(core.join(".owner"), "user/org").unwrap();
        let demo_dir = support.path().join(DEMO_DIR_NAME);
        std::fs::create_dir_all(&demo_dir).unwrap();
        std::fs::write(demo_dir.join("left-over"), "demo").unwrap();

        let client = CoreClient::new(config(&demo_dir), demo(), Arc::new(Quiet)).unwrap();
        assert!(client.is_demo());
        client.shutdown();
        drop(client);

        assert!(forget_demo_data(demo_dir.display().to_string()));
        assert!(!demo_dir.exists());
        // The account's folder is untouched.
        assert_eq!(
            std::fs::read_to_string(core.join(".owner")).unwrap(),
            "user/org"
        );
        // Forgetting twice is fine: the demo is already gone.
        assert!(forget_demo_data(demo_dir.display().to_string()));
    }

    #[test]
    fn only_the_demo_folder_is_ever_removed() {
        let support = tempfile::tempdir().unwrap();
        let core = support.path().join("core");
        std::fs::create_dir_all(&core).unwrap();
        assert!(!forget_demo_data(core.display().to_string()));
        assert!(core.exists());
        assert!(!forget_demo_data(support.path().display().to_string()));
        assert!(!forget_demo_data(String::new()));
        assert!(support.path().exists());
    }

    #[test]
    fn the_way_out_reads_plainly() {
        let copy = demo_exit_copy();
        for text in [
            &copy.chip,
            &copy.chip_hint,
            &copy.settings_title,
            &copy.settings_detail,
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
            // OpenAgents speaks as "we".
            assert!(!text.split_whitespace().any(|w| w == "I"), "{text}");
        }
        assert!(copy.chip.contains("Exit"));
        assert_eq!(copy.settings_title, "Exit demo");
    }
}
