//! A short native-signed grant for real browser idle-expiry acceptance.
//! The earlier invitation clock is explicit fixture input; redemption and expiry
//! use the real host and browser clocks. No grant or device key is fabricated.
use std::{
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [directory] = args.as_slice() else {
        return Err("usage: cloud_terminal_invitation_fixture SYNTHETIC_TERMINAL_ROOT".into());
    };
    let directory = PathBuf::from(directory)
        .canonicalize()
        .map_err(|_| "Synthetic terminal root is unavailable.")?;
    if !directory
        .file_name()
        .is_some_and(|s| s.to_string_lossy().starts_with("cloud-terminal-browser-"))
        || !directory.join("synthetic-home").is_dir()
        || !directory.join("resident-access/access.json").is_file()
    {
        return Err("An existing isolated terminal fixture is required.".into());
    }
    let access = coder_access::Access::parse(
        &std::fs::read(directory.join("resident-device.access"))
            .map_err(|_| "Synthetic relay metadata is unavailable.")?,
    )
    .map_err(|_| "Synthetic relay metadata is malformed.")?;
    if !access.grant.relay.starts_with("ws://127.0.0.1:") {
        return Err("An explicitly configured synthetic loopback relay is required.".into());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Synthetic clock is unavailable.")?
        .as_secs();
    let host = coder_access::host::Host::new(
        directory.join("resident-access"),
        coder_access::RelayPolicy::LoopbackTest,
    );
    // Native invitations have a fixed five-minute window. The explicit earlier
    // fixture clock leaves fifteen seconds to redeem a real thirty-one-second
    // grant, whose original native authorization expires under real time.
    let grant_expires_at = now + 31;
    let invitation = host
        .invite(
            &access.grant.relay,
            coder_access::Rights::new([
                coder_access::Right::Observe,
                coder_access::Right::Terminal,
            ])
            .map_err(|_| "Synthetic rights are unavailable.")?,
            now.checked_sub(285).ok_or("Synthetic clock is invalid.")?,
            grant_expires_at,
        )
        .map_err(|_| "Synthetic native invitation could not be issued.")?;
    let path = directory.join("browser-terminal.invitation");
    let staged = directory.join(format!(
        "invitation-stage-{}",
        coder_access::protocol::random_id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&staged)
        .map_err(|_| "Synthetic invitation file could not be created.")?;
    file.write_all(invitation.code.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| "Synthetic invitation file could not be written.")?;
    std::fs::rename(&staged, &path)
        .map_err(|_| "Synthetic invitation file could not be installed.")?;
    println!(
        "{}",
        serde_json::json!({
            "synthetic":true,"private_invitation_file":path,
            "grant_expires_at":grant_expires_at,"deliberate_earlier_invitation_clock":true
        })
    );
    Ok(())
}
