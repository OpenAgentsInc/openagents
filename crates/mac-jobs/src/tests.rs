use std::path::{Path, PathBuf};

use serde_json::json;

use super::*;

fn spec(recipe: Recipe, args: &[&str]) -> Spec {
    Spec {
        repo: "OpenAgentsInc/openagents".into(),
        git_ref: "main".into(),
        recipe,
        args: args.iter().map(|a| (*a).to_owned()).collect(),
    }
}

fn places() -> Places {
    Places {
        checkout: PathBuf::from("/jobs/j/src"),
        target: PathBuf::from("/jobs/j/target"),
        out: PathBuf::from("/jobs/j/out"),
        simulator: Some("UDID-1".into()),
    }
}

#[test]
fn a_job_reads_and_writes_its_json() {
    let job: Spec = serde_json::from_value(json!({
        "repo": "OpenAgentsInc/openagents", "ref": "release/1.0",
        "recipe": "ios-release-gate", "args": ["--simulator", "iPhone 17 Pro"]
    }))
    .unwrap();
    assert_eq!(job.recipe, Recipe::IosReleaseGate);
    assert_eq!(job.git_ref, "release/1.0");
    assert!(job.check().is_ok());
    assert_eq!(job.simulator(), Some("iPhone 17 Pro"));
    assert_eq!(serde_json::to_value(&job).unwrap()["ref"], "release/1.0");
    assert!(
        serde_json::from_value::<Spec>(json!({
            "repo": "a/b", "ref": "main", "recipe": "shell"
        }))
        .is_err()
    );
    for recipe in Recipe::ALL {
        assert_eq!(Recipe::parse(recipe.name()), Some(recipe));
    }
}

#[test]
fn repositories_and_refs_never_hide_an_option_or_a_path() {
    for (repo, git_ref) in [
        ("OpenAgentsInc/openagents", "main"),
        ("a-b/c.d_e", "feature/x-1"),
        ("o/r", "0123456789abcdef0123456789abcdef01234567"),
    ] {
        let mut job = spec(Recipe::DesktopCapture, &[]);
        job.repo = repo.into();
        job.git_ref = git_ref.into();
        assert!(job.check().is_ok(), "{repo} {git_ref}");
    }
    for (repo, git_ref) in [
        ("openagents", "main"),
        ("o/../x", "main"),
        ("-o/r", "main"),
        ("o/r", "--upload-pack=x"),
        ("o/r", "../etc"),
        ("o/r", "a..b"),
        ("o/r", "a b"),
        ("o/r", "/abs"),
        ("o/r", "x.lock"),
        ("o/r", "$(id)"),
        ("o/r", ""),
    ] {
        let mut job = spec(Recipe::DesktopCapture, &[]);
        job.repo = repo.into();
        job.git_ref = git_ref.into();
        assert!(job.check().is_err(), "{repo} {git_ref}");
    }
}

#[test]
fn each_recipe_takes_only_its_allowlisted_arguments() {
    let fine = [
        spec(Recipe::IosReleaseGate, &[]),
        spec(
            Recipe::IosReleaseGate,
            &["--simulator", "iPhone 17 Pro Max"],
        ),
        spec(Recipe::IosTestflight, &[]),
        spec(Recipe::IosTestflight, &["--validate-only", "--build", "42"]),
        spec(Recipe::DesktopCapture, &["--kept"]),
        spec(
            Recipe::Xcodebuild,
            &[
                "test",
                "-project",
                "bins/openagents-ios/host/OpenAgents.xcodeproj",
                "-scheme",
                "OpenAgents",
                "-configuration",
                "Debug",
                "-destination",
                "platform=iOS Simulator,name=iPhone 17 Pro",
                "-only-testing:OpenAgentsUITests/ReleaseGateUITests",
                "CODE_SIGN_IDENTITY=-",
                "-quiet",
            ],
        ),
    ];
    for job in &fine {
        assert!(job.check().is_ok(), "{job:?}: {:?}", job.check());
    }
    let refused = [
        spec(Recipe::IosReleaseGate, &["--simulator", "x; rm -rf /"]),
        spec(Recipe::IosTestflight, &["--build", "1;2"]),
        spec(Recipe::IosTestflight, &["--api-key", "x"]),
        spec(Recipe::DesktopCapture, &["--capture", "/etc"]),
        spec(Recipe::Xcodebuild, &["archive"]),
        spec(Recipe::Xcodebuild, &["-exportArchive"]),
        spec(Recipe::Xcodebuild, &["build", "-allowProvisioningUpdates"]),
        spec(
            Recipe::Xcodebuild,
            &["build", "-authenticationKeyPath", "/k.p8"],
        ),
        spec(Recipe::Xcodebuild, &["build", "-project", "../x.xcodeproj"]),
        spec(Recipe::Xcodebuild, &["build", "-project", "/x.xcodeproj"]),
        spec(Recipe::Xcodebuild, &["build", "CODE_SIGN_IDENTITY=Apple"]),
        spec(Recipe::Xcodebuild, &["build", "OTHER_LDFLAGS=-x"]),
        spec(Recipe::Xcodebuild, &["build", "-derivedDataPath", "/tmp"]),
        spec(Recipe::Xcodebuild, &["-scheme", "OpenAgents"]),
    ];
    for job in &refused {
        assert!(job.check().is_err(), "{job:?}");
    }
}

#[test]
fn kinds_and_the_outward_recipe() {
    assert_eq!(spec(Recipe::IosReleaseGate, &[]).kind(), Kind::Test);
    assert_eq!(spec(Recipe::IosTestflight, &[]).kind(), Kind::Upload);
    assert_eq!(spec(Recipe::DesktopCapture, &[]).kind(), Kind::Capture);
    assert_eq!(spec(Recipe::Xcodebuild, &["build"]).kind(), Kind::Build);
    assert_eq!(spec(Recipe::Xcodebuild, &["test"]).kind(), Kind::Test);
    assert!(spec(Recipe::IosTestflight, &["--validate-only"]).outward());
    assert!(!spec(Recipe::IosReleaseGate, &[]).outward());
    assert_eq!(
        spec(Recipe::IosReleaseGate, &[]).title(),
        "iOS release gate · openagents@main"
    );
    assert!(valid_job_id("mjob0123456789abcdef0123456789abcdef"));
    assert!(!valid_job_id("mjob../x"));
}

#[test]
fn the_release_gate_runs_on_its_own_simulator_and_build_folder() {
    let plan = plan(&spec(Recipe::IosReleaseGate, &[]), &places());
    let labels: Vec<&str> = plan.steps.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels.len(), 5);
    let build = &plan.steps[0];
    assert_eq!(build.program, "/jobs/j/src/bins/openagents-ios/build.sh");
    assert!(
        build
            .env
            .contains(&("CARGO_TARGET_DIR".into(), "/jobs/j/target".into()))
    );
    assert!(
        build
            .env
            .contains(&("OPENAGENTS_IOS_DEVICE".into(), "UDID-1".into()))
    );
    let gate = &plan.steps[2];
    assert_eq!(gate.program, "xcodebuild");
    assert!(gate.args.contains(&"id=UDID-1".to_owned()));
    assert!(
        gate.args
            .contains(&"-only-testing:OpenAgentsUITests/ReleaseGateUITests".to_owned())
    );
    assert!(gate.env.contains(&(
        "TEST_RUNNER_OPENAGENTS_UITEST_SHOTS".into(),
        "/jobs/j/out/shots".into()
    )));
    assert_eq!(
        plan.steps[3].stdout_to.as_deref(),
        Some(Path::new("/jobs/j/out/xcresult-summary.json"))
    );
    assert!(plan.steps[3].optional && plan.steps[4].optional);
    // No step runs a shell.
    for step in &plan.steps {
        assert!(!["sh", "bash", "zsh", "/bin/sh"].contains(&step.program.as_str()));
    }
}

#[test]
fn testflight_and_xcodebuild_plans() {
    let ship = plan(
        &spec(Recipe::IosTestflight, &["--validate-only"]),
        &places(),
    );
    assert_eq!(ship.steps.len(), 1);
    assert_eq!(
        ship.steps[0].program,
        "/jobs/j/src/scripts/release/testflight.sh"
    );
    assert_eq!(ship.steps[0].args, ["run", "--validate-only"]);
    assert_eq!(ship.collect.len(), 2);
    let build = plan(
        &spec(
            Recipe::Xcodebuild,
            &["test", "-project", "app/App.xcodeproj", "-scheme", "App"],
        ),
        &places(),
    );
    let args = &build.steps[0].args;
    assert!(args.contains(&"/jobs/j/src/app/App.xcodeproj".to_owned()));
    assert!(args.contains(&"/jobs/j/target/DerivedData".to_owned()));
    assert!(args.contains(&"/jobs/j/out/Result.xcresult".to_owned()));
    assert_eq!(build.steps.len(), 2);
    let capture = plan(&spec(Recipe::DesktopCapture, &["--kept"]), &places());
    assert!(capture.steps[0].args.contains(&"--capture-kept".to_owned()));
}

#[test]
fn capabilities_come_from_the_tools_output() {
    assert_eq!(
        parse_sw_vers("ProductName:\t\tmacOS\nProductVersion:\t\t26.4\nBuildVersion:\t\t25E246\n")
            .as_deref(),
        Some("26.4 (25E246)")
    );
    assert_eq!(
        parse_xcode_version("Xcode 26.6\nBuild version 17F113\n").as_deref(),
        Some("26.6 (17F113)")
    );
    assert_eq!(parse_xcode_version("xcode-select: error"), None);
    let identities = parse_identities(
        "  1) 0123456789ABCDEF0123456789ABCDEF01234567 \"Apple Development: A (T1)\"\n  \
         2) 89ABCDEF0123456789ABCDEF0123456789ABCDEF \"Apple Distribution: OpenAgents, Inc. (T2)\"\n  \
         3) FEDCBA9876543210FEDCBA9876543210FEDCBA98 \"Apple Distribution: OpenAgents, Inc. (T2)\"\n     \
         3 valid identities found\n",
    );
    assert_eq!(
        identities,
        [
            "Apple Development: A (T1)",
            "Apple Distribution: OpenAgents, Inc. (T2)"
        ]
    );
    assert!(
        identities
            .iter()
            .all(|name| !name.contains("0123456789ABCDEF"))
    );
    let simulators = parse_simctl(
        &json!({"devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-26-5": [
                {"name": "iPhone 17 Pro", "udid": "A", "state": "Shutdown", "isAvailable": true},
                {"name": "gate", "udid": "B", "state": "Booted", "isAvailable": true},
                {"name": "gone", "udid": "C", "state": "Shutdown", "isAvailable": false}
            ],
            "com.apple.CoreSimulator.SimRuntime.watchOS-11-0": [
                {"name": "Watch", "udid": "D", "state": "Shutdown", "isAvailable": true}
            ]
        }})
        .to_string(),
    );
    assert_eq!(simulators.len(), 2);
    assert_eq!(simulators[0].name, "gate");
    assert!(simulators[0].booted);
    assert_eq!(simulators[1].runtime, "iOS 26.5");
    assert_eq!(
        parse_df_available_gb(
            "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s5 1948404040 1798684968 111744864 95% /\n"
        ),
        Some(106)
    );
}

#[test]
fn the_app_store_key_is_present_or_not_and_nothing_more() {
    let dir = tempfile_dir();
    let key = dir.join("AuthKey.p8");
    let env_file = dir.join("asc.env");
    std::fs::write(
        &env_file,
        format!(
            "ASC_API_KEY_ID=X\nASC_API_PRIVATE_KEY_PATH=\"{}\"\n",
            key.display()
        ),
    )
    .unwrap();
    let env_file_text = env_file.to_string_lossy().into_owned();
    let env = move |name: &str| (name == "OPENAGENTS_ASC_ENV").then(|| env_file_text.clone());
    assert!(!asc_key_present(&env, None));
    std::fs::write(&key, "not a real key").unwrap();
    assert!(asc_key_present(&env, None));
    assert!(!asc_key_present(&|_| None, Some(&dir)));
    let _ = std::fs::remove_dir_all(&dir);
}

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mac-jobs-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_mac_supports_what_it_has() {
    let mut caps = Capabilities {
        macos: Some("26.4".into()),
        xcode: Some("26.6".into()),
        recipes: Recipe::ALL.to_vec(),
        simulators: vec![Simulator {
            name: "iPhone 17 Pro".into(),
            ..Simulator::default()
        }],
        ..Capabilities::default()
    };
    assert!(caps.supports(&spec(Recipe::IosReleaseGate, &[])).is_ok());
    let refused = caps
        .supports(&spec(Recipe::IosTestflight, &[]))
        .unwrap_err();
    assert!(refused.contains("App Store Connect key"));
    caps.asc_key = true;
    assert!(caps.supports(&spec(Recipe::IosTestflight, &[])).is_err());
    caps.signing_identities = vec!["Apple Distribution: OpenAgents, Inc. (T)".into()];
    assert!(caps.supports(&spec(Recipe::IosTestflight, &[])).is_ok());
    caps.recipes = vec![Recipe::DesktopCapture];
    assert!(caps.supports(&spec(Recipe::IosReleaseGate, &[])).is_err());
}

#[test]
fn log_lines_and_artifact_names_are_safe() {
    let line =
        redact_line("token sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789\tdone\u{1b}[0m");
    assert!(!line.contains("abcdefghijklmnop"), "{line}");
    assert!(!line.contains('\u{1b}'));
    assert!(redact_line(&"x".repeat(1000)).chars().count() <= LINE_CHARS);
    assert_eq!(
        artifact_name(Path::new("shots/01 chat.png")).as_deref(),
        Some("shots-01_chat.png")
    );
    assert_eq!(artifact_name(Path::new("..")).as_deref(), None);
    let long = artifact_name(Path::new(&format!("{}.png", "a".repeat(200)))).unwrap();
    assert!(long.len() <= 96 && long.ends_with(".png"));
}

#[test]
fn the_result_line_reads_the_test_summary() {
    let dir = tempfile_dir();
    assert_eq!(summary_line(&dir, true), "Finished.");
    std::fs::write(
        dir.join("xcresult-summary.json"),
        json!({"result": "Passed", "totalTestCount": 1, "passedTests": 1, "failedTests": 0, "skippedTests": 0}).to_string(),
    )
    .unwrap();
    assert_eq!(summary_line(&dir, true), "Passed: 1 of 1 tests passed.");
    let _ = std::fs::remove_dir_all(&dir);
}
