//! Local-only display of the intended, short-lived pairing capability.
use coder_connect::{Error, ErrorCode, Result, pairing};
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
pub struct Display {
    pub id: String,
    pub expires_at: u64,
    path: PathBuf,
    pub active: bool,
    host: coder_connect::host::Host,
}
impl Display {
    pub fn show(
        directory: &Path,
        code: &str,
        policy: coder_connect::RelayPolicy,
        browser: bool,
    ) -> Result<Self> {
        let invitation = pairing::Invitation::parse(code, coder_connect::unix_time()?, policy)?;
        let svg = pairing::qr_svg(code)?;
        let terminal = pairing::terminal_qr(code)?;
        let path = directory.join(format!("pairing-{}.html", invitation.id));
        let html = format!(
            "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"referrer\" content=\"no-referrer\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Connect Coder</title><style>body{{font:18px system-ui;max-width:680px;margin:30px auto;padding:20px;background:#111;color:#eee}}svg{{display:block;width:min(100%,520px);height:auto;margin:20px auto}}input{{box-sizing:border-box;width:100%;padding:12px}}strong{{color:#ffbd45}}</style><h1>Connect your phone</h1><p>In Coder, choose <strong>Scan QR code</strong>.</p><p>Single use. Expires five minutes after creation. Only share this code with the phone you want to connect. Anyone who can see it can claim this invitation first.</p>{svg}<label>Paste instead<input readonly value=\"{code}\" aria-label=\"Pairing code\"></label><p>The computer command must stay running. This static page does not show pairing status; see the terminal. After successful pairing or expiry, this code cannot admit another phone.</p></html>",
        );
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| {
                Error::new(
                    ErrorCode::Unavailable,
                    "cannot create private local pairing page",
                )
            })?;
        if file
            .write_all(html.as_bytes())
            .and_then(|_| file.sync_all())
            .is_err()
        {
            let _ = std::fs::remove_file(&path);
            return Err(Error::new(
                ErrorCode::Unavailable,
                "cannot write private local pairing page",
            ));
        }
        println!("{terminal}");
        println!("Scan in Coder, or paste this pairing code:\n{code}\n");
        println!(
            "Single use; expires in five minutes. Only share it with your phone.\nLocal QR page: {}",
            path.display()
        );
        if browser {
            open_browser(&path);
        }
        Ok(Self {
            id: invitation.id,
            expires_at: invitation.expires_at,
            path,
            active: true,
            host: coder_connect::host::Host::new(directory, policy),
        })
    }
    pub fn clear(&mut self) {
        self.active = false;
        let _ = std::fs::remove_file(&self.path);
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if self.active {
            let _ = self.host.cancel_invitation(&self.id);
        }
        self.clear();
    }
}
fn open_browser(path: &Path) {
    #[cfg(target_os = "macos")]
    let command = "open";
    #[cfg(not(target_os = "macos"))]
    let command = "xdg-open";
    // This opens only the generated private local file, never a QR-supplied URL.
    if let Ok(mut child) = std::process::Command::new(command)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}
