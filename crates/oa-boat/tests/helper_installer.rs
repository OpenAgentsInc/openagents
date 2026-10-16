//! Exercise the exact SSH installer framing with private local sudo/run fixtures.
#![cfg(target_os = "linux")]
use oa_boat::remote::{Chunk, Local, Output, Remote};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
#[derive(Clone)]
struct Guest {
    local: Local,
    root: std::path::PathBuf,
    truncate: Arc<AtomicBool>,
}
impl Remote for Guest {
    async fn run(
        &self,
        host: &str,
        script: &str,
        mut input: Vec<u8>,
        time: Duration,
        cap: usize,
    ) -> Output {
        let script = script
            .replace("/usr/bin/sudo", self.root.join("sudo").to_str().unwrap())
            .replace(
                "/run/oa-computer-files.",
                self.root.join("helper.").to_str().unwrap(),
            );
        if self.truncate.swap(false, Ordering::SeqCst) {
            input.truncate(input.len().saturating_sub(4));
        }
        self.local.run(host, &script, input, time, cap).await
    }
    async fn stream(&self, host: &str, script: &str, time: Duration) -> mpsc::Receiver<Chunk> {
        self.local.stream(host, script, time).await
    }
    async fn forget(&self, host: &str) {
        self.local.forget(host).await
    }
}
#[tokio::test]
async fn installer_frames_binary_and_upload_then_removes_executable_on_every_exit() {
    let root = tempfile::tempdir().unwrap();
    let sudo = root.path().join("sudo");
    std::fs::write(
        &sudo,
        "#!/bin/sh\n[ \"$1\" = -n ] && shift\nif [ \"$1\" = -u ]; then shift 2; fi\nexec \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&sudo, std::fs::Permissions::from_mode(0o700)).unwrap();
    let helper = std::env::var_os("OA_COMPUTER_FILES_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("computer-files")
        });
    assert!(helper.is_file(), "build computer-files first");
    let truncate = Arc::new(AtomicBool::new(false));
    let guest = Guest {
        local: Local {
            root: root.path().to_owned(),
        },
        root: root.path().to_owned(),
        truncate: truncate.clone(),
    };
    let bytes = b"binary\x00\xff\n'payload".to_vec();
    let path = "file '\u{e9}.bin";
    let args = vec![
        "write".into(),
        path.into(),
        bytes.len().to_string(),
        computer_files::digest(&bytes),
        "true".into(),
    ];
    let out = guest
        .workspace_file("guest", &helper, &args, bytes.clone())
        .await;
    assert_eq!(
        out.code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = guest
        .workspace_file("guest", &helper, &["read".into(), path.into()], vec![])
        .await;
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stdout, bytes);
    truncate.store(true, Ordering::SeqCst);
    let out = guest
        .workspace_file("guest", &helper, &args, bytes.clone())
        .await;
    assert_eq!(
        out.code,
        Some(computer_files::FileError::Incomplete.exit_code())
    );
    assert_eq!(
        std::fs::read(root.path().join("guest/home/workspace").join(path)).unwrap(),
        bytes
    );
    assert!(
        !std::fs::read_dir(root.path()).unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("helper.")),
        "installer directory was left behind"
    );
}
