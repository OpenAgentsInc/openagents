//! Unit and plist rendering, and the exact service manager commands each
//! operation runs, against a recording runner. Nothing here touches the
//! machine's launchd or systemd.

use std::collections::BTreeMap;

use super::*;
use crate::launcher::{CONFIG_SCHEMA, initialize};

const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
    replies: BTreeMap<String, Vec<Output>>,
}

impl Recorder {
    /// Queues a reply. Replies to one command are used in order, and the
    /// last one repeats.
    fn reply(&mut self, command: &str, code: i32, stdout: &str) {
        self.replies
            .entry(command.into())
            .or_default()
            .push(Output {
                code: Some(code),
                stdout: stdout.into(),
                stderr: String::new(),
            });
    }
}

impl Runner for Recorder {
    fn run(&mut self, program: &str, args: &[String]) -> Result<Output> {
        let line = format!("{program} {}", args.join(" "));
        self.calls.push(line.clone());
        Ok(match self.replies.get_mut(&line) {
            Some(queue) if queue.len() > 1 => queue.remove(0),
            Some(queue) => queue[0].clone(),
            None => Output {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            },
        })
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    layout: Layout,
    config: Config,
}

fn fixture(platform: Platform) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().canonicalize().unwrap();
    let bundles = base.join("bundles");
    let script = "#!/bin/sh\nexit 0\n";
    let digest = fsx::sha256_hex(script.as_bytes());
    let directory = bundles.join("versions").join(&digest);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("coder"), script).unwrap();
    fs::write(
        directory.join("manifest.json"),
        serde_json::json!({"schema": crate::bundle::BUNDLE_SCHEMA, "binary_sha256": digest})
            .to_string(),
    )
    .unwrap();
    let layout = Layout::new(base.join("home/.openagents/host"));
    let config = Config {
        schema: CONFIG_SCHEMA.into(),
        label: "org.openagents.coder-host".into(),
        platform,
        registration_dir: base.join("registration"),
        launcher: base.join("home/.openagents/host/bin/abc/coder-service"),
        bundle_root: bundles,
        host_args: vec!["host".into(), "serve".into()],
        state_dirs: vec![base.join("home/.openagents/tasks")],
        listen: "127.0.0.1:47100".into(),
        host_key: KEY.into(),
        ready_timeout_secs: 60,
        stop_grace_secs: 10,
        snapshot_max_bytes: 1 << 30,
    };
    initialize(&layout, &config, &digest).unwrap();
    Fixture {
        _temp: temp,
        layout,
        config,
    }
}

#[test]
fn systemd_unit_renders_a_restarting_user_service() {
    let fixture = fixture(Platform::Linux);
    let unit = render(&fixture.layout, &fixture.config).unwrap();
    let root = fixture.layout.root().display().to_string();
    let launcher = fixture.config.launcher.display().to_string();
    let expected = format!(
        "[Unit]\n\
         Description=OpenAgents Coder host (org.openagents.coder-host)\n\
         StartLimitIntervalSec=300\n\
         StartLimitBurst=5\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart=\"{launcher}\" \"--root\" \"{root}\" \"run\"\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         KillMode=control-group\n\
         TimeoutStopSec=20\n\
         UMask=0077\n\
         NoNewPrivileges=yes\n\
         Environment=\"PATH=/usr/bin:/bin\"\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );
    assert_eq!(unit, expected);
}

#[test]
fn systemd_quoting_escapes_specifiers_and_refuses_newlines() {
    assert_eq!(
        systemd_quote("a b%c$d\"e\\f").unwrap(),
        "\"a b%%c$$d\\\"e\\\\f\""
    );
    assert!(systemd_quote("a\nExecStartPre=/bin/evil").is_err());
}

#[test]
fn launchd_plist_renders_a_login_agent_that_restarts_on_failure() {
    let fixture = fixture(Platform::Macos);
    let plist = render(&fixture.layout, &fixture.config).unwrap();
    let root = fixture.layout.root().display().to_string();
    for needle in [
        "<key>Label</key>\n\t<string>org.openagents.coder-host</string>",
        "<key>RunAtLoad</key>\n\t<true/>",
        "<key>KeepAlive</key>\n\t<dict>\n\t\t<key>SuccessfulExit</key>\n\t\t<false/>\n\t</dict>",
        "<key>AbandonProcessGroup</key>\n\t<false/>",
        "<key>Umask</key>\n\t<integer>63</integer>",
        "<key>ExitTimeOut</key>\n\t<integer>20</integer>",
        &format!(
            "\t\t<string>--root</string>\n\t\t<string>{root}</string>\n\t\t<string>run</string>\n"
        ),
        &format!("<key>StandardOutPath</key>\n\t<string>{root}/logs/launcher.log</string>"),
    ] {
        assert!(plist.contains(needle), "missing {needle:?} in\n{plist}");
    }
    assert!(plist.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist"));
    assert!(plist.ends_with("</dict>\n</plist>\n"));
    assert_eq!(xml("a&b<c>\"'").unwrap(), "a&amp;b&lt;c&gt;&quot;&apos;");
    assert!(xml("a\u{7}").is_err());
}

#[test]
fn plutil_accepts_the_plist_when_available() {
    let fixture = fixture(Platform::Macos);
    let plist = render(&fixture.layout, &fixture.config).unwrap();
    let path = fixture.layout.root().join("check.plist");
    fs::write(&path, plist).unwrap();
    match Command::new("/usr/bin/plutil")
        .arg("-lint")
        .arg(&path)
        .output()
    {
        Ok(output) => assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        ),
        Err(_) => eprintln!("plutil is not available on this host; lint skipped"),
    }
}

#[test]
fn labels_are_checked() {
    assert!(validate_label("org.openagents.coder-host").is_ok());
    for bad in ["", ".hidden", "-x", "a/b", "a b", &"x".repeat(81)] {
        assert!(validate_label(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn systemd_install_status_restart_and_uninstall_run_the_expected_commands() {
    let fixture = fixture(Platform::Linux);
    let mut runner = Recorder::default();
    let report = install(&fixture.layout, &fixture.config, &mut runner, true, true).unwrap();
    assert_eq!(
        runner.calls,
        [
            "systemctl --user daemon-reload",
            "systemctl --user enable org.openagents.coder-host.service",
            "systemctl --user restart org.openagents.coder-host.service",
            "loginctl enable-linger",
        ]
    );
    assert!(report.linger_enabled);
    assert_eq!(
        fs::read_link(&report.registration).unwrap(),
        report.definition
    );
    let mode = fs::metadata(&report.definition)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let mut runner = Recorder::default();
    runner.reply(
        "systemctl --user show org.openagents.coder-host.service --property=LoadState,ActiveState,SubState,UnitFileState,NeedDaemonReload,MainPID",
        0,
        "LoadState=loaded\nActiveState=active\nSubState=running\nUnitFileState=linked\nNeedDaemonReload=no\nMainPID=4242\n",
    );
    runner.reply(
        &format!("loginctl show-user {} --property=Linger", uid()),
        0,
        "Linger=yes\n",
    );
    let status = super::status(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert!(status.loaded && status.running && status.enabled && status.registered);
    assert!(status.definition_current);
    assert_eq!(status.pid, Some(4242));
    assert_eq!(status.linger, Some(true));
    assert!(status.survives_logout);
    assert_eq!(status.starts_at, StartsAt::Boot);
    assert!(!status.pending_restart, "{:?}", status.pending_reasons);

    let mut runner = Recorder::default();
    runner.reply(
        "systemctl --user show org.openagents.coder-host.service --property=LoadState,ActiveState,SubState,UnitFileState,NeedDaemonReload,MainPID",
        0,
        "LoadState=loaded\nActiveState=active\nSubState=running\nUnitFileState=enabled\nNeedDaemonReload=yes\nMainPID=4242\n",
    );
    runner.reply(
        &format!("loginctl show-user {} --property=Linger", uid()),
        0,
        "Linger=no\n",
    );
    let status = super::status(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert!(!status.survives_logout);
    assert_eq!(status.starts_at, StartsAt::Login);
    assert!(status.pending_restart);

    let mut runner = Recorder::default();
    restart(&fixture.config, &mut runner).unwrap();
    assert_eq!(
        runner.calls,
        ["systemctl --user restart org.openagents.coder-host.service"]
    );

    let mut runner = Recorder::default();
    let removed = uninstall(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert_eq!(
        runner.calls,
        [
            "systemctl --user disable --now org.openagents.coder-host.service",
            "systemctl --user show org.openagents.coder-host.service --property=ActiveState",
            "systemctl --user daemon-reload",
        ]
    );
    assert!(removed.stopped && removed.registration_removed && removed.definition_removed);
    assert!(
        removed
            .preserved
            .iter()
            .any(|item| item == "the linger setting")
    );
    assert!(fs::symlink_metadata(&report.registration).is_err());
    assert!(
        fixture.layout.state().exists(),
        "uninstall keeps the launcher record"
    );
}

#[test]
fn launchd_install_status_restart_and_uninstall_run_the_expected_commands() {
    let fixture = fixture(Platform::Macos);
    let target = format!("gui/{}/org.openagents.coder-host", uid());
    let domain = format!("gui/{}", uid());
    let mut runner = Recorder::default();
    runner.reply(&format!("launchctl print {target}"), 113, "");
    assert!(
        install(&fixture.layout, &fixture.config, &mut runner, true, true).is_err(),
        "linger is Linux only"
    );
    let report = install(&fixture.layout, &fixture.config, &mut runner, true, false).unwrap();
    let registration = report.registration.display().to_string();
    assert_eq!(
        runner.calls,
        [
            format!("launchctl print {target}"),
            format!("launchctl print-disabled {domain}"),
            format!("launchctl bootstrap {domain} {registration}"),
        ]
    );

    // Reinstalling a loaded, disabled agent unloads it and enables it.
    let mut runner = Recorder::default();
    runner.reply(
        &format!("launchctl print-disabled {domain}"),
        0,
        "\t\"org.openagents.coder-host\" => disabled\n",
    );
    install(&fixture.layout, &fixture.config, &mut runner, true, false).unwrap();
    assert_eq!(
        runner.calls,
        [
            format!("launchctl print {target}"),
            format!("launchctl bootout {target}"),
            format!("launchctl print-disabled {domain}"),
            format!("launchctl enable {target}"),
            format!("launchctl bootstrap {domain} {registration}"),
        ]
    );

    let mut runner = Recorder::default();
    runner.reply(
        &format!("launchctl print {target}"),
        0,
        "\tstate = running\n\tpid = 99\n",
    );
    runner.reply(
        &format!("launchctl print-disabled {domain}"),
        0,
        "\t\"org.other\" => disabled\n",
    );
    let status = super::status(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert!(status.loaded && status.running && status.enabled);
    assert_eq!(status.pid, Some(99));
    assert_eq!(status.starts_at, StartsAt::Login);
    assert!(!status.survives_logout);
    assert_eq!(status.linger, None);

    let mut runner = Recorder::default();
    runner.reply(&format!("launchctl print {target}"), 0, "");
    runner.reply(
        &format!("launchctl print-disabled {domain}"),
        0,
        "\t\"org.openagents.coder-host\" => disabled\n",
    );
    let status = super::status(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert!(!status.enabled && !status.running);
    assert_eq!(status.starts_at, StartsAt::Never);

    let mut runner = Recorder::default();
    runner.reply(&format!("launchctl print {target}"), 0, "");
    restart(&fixture.config, &mut runner).unwrap();
    assert_eq!(
        runner.calls,
        [
            format!("launchctl print {target}"),
            format!("launchctl kickstart -k {target}")
        ]
    );

    let mut runner = Recorder::default();
    // Loaded before `bootout`, still loaded once after it, then gone.
    runner.reply(&format!("launchctl print {target}"), 0, "");
    runner.reply(&format!("launchctl print {target}"), 0, "");
    runner.reply(&format!("launchctl print {target}"), 113, "");
    let removed = uninstall(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert_eq!(
        runner.calls,
        [
            format!("launchctl print {target}"),
            format!("launchctl bootout {target}"),
            format!("launchctl print {target}"),
            format!("launchctl print {target}"),
        ]
    );
    assert!(removed.stopped && removed.registration_removed);
    assert!(fs::symlink_metadata(&report.registration).is_err());
}

#[test]
fn install_refuses_a_registration_that_belongs_to_something_else() {
    let fixture = fixture(Platform::Linux);
    fs::create_dir_all(&fixture.config.registration_dir).unwrap();
    fs::write(registration_path(&fixture.config), "[Unit]\n").unwrap();
    let mut runner = Recorder::default();
    assert!(install(&fixture.layout, &fixture.config, &mut runner, true, false).is_err());
    assert!(runner.calls.is_empty());
    let removed = uninstall(&fixture.layout, &fixture.config, &mut runner).unwrap();
    assert!(
        !removed.registration_removed,
        "a foreign file is never removed"
    );
    assert!(registration_path(&fixture.config).exists());
}

#[test]
fn launchd_disabled_parsing_accepts_both_spellings() {
    assert!(parse_launchd_disabled("\t\"x\" => true\n", "x"));
    assert!(parse_launchd_disabled("\t\"x\" => disabled\n", "x"));
    assert!(!parse_launchd_disabled("\t\"x\" => enabled\n", "x"));
    assert!(!parse_launchd_disabled("\t\"x.y\" => disabled\n", "x"));
}

#[test]
fn install_creates_the_log_directory_privately_before_launchd_can() {
    let fixture = fixture(Platform::Macos);
    for dir in [
        fixture.layout.logs(),
        fixture.layout.run_dir(),
        fixture.layout.snapshots(),
    ] {
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{}", dir.display());
    }
}
