//! The programs each recipe runs, in order, with no shell between them.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::spec::{Recipe, Spec};

/// Where a job runs on the Mac.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
    /// The job's own worktree at the ref's commit.
    pub checkout: PathBuf,
    /// The job's own build folder (`CARGO_TARGET_DIR`, derived data),
    /// removed when the job ends.
    pub target: PathBuf,
    /// What the job made, uploaded when it ends.
    pub out: PathBuf,
    /// The fresh simulator a release gate runs on, by UDID.
    pub simulator: Option<String>,
}

/// One program to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// What the step does, as the log says it.
    pub label: String,
    /// The program: a bare name found on `PATH`, or a script in the
    /// checkout by its full path.
    pub program: String,
    pub args: Vec<String>,
    /// Variables set for this step only.
    pub env: Vec<(String, String)>,
    /// Write the program's standard output to this file instead of the log.
    pub stdout_to: Option<PathBuf>,
    /// A failure is logged and the job goes on.
    pub optional: bool,
}

/// Files the job keeps from a folder outside `out`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collect {
    pub dir: PathBuf,
    pub extensions: Vec<&'static str>,
}

/// A recipe's steps and the files it keeps.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Besides everything in `out`.
    pub collect: Vec<Collect>,
}

fn path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn step(label: &str, program: impl Into<String>, args: &[&str]) -> Step {
    Step {
        label: label.to_owned(),
        program: program.into(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        env: Vec::new(),
        stdout_to: None,
        optional: false,
    }
}

/// The steps for `spec`, which [`Spec::check`] accepted, at `places`.
#[must_use]
pub fn plan(spec: &Spec, places: &Places) -> Plan {
    let target = path(&places.target);
    let cargo_target = ("CARGO_TARGET_DIR".to_owned(), target.clone());
    match spec.recipe {
        Recipe::IosReleaseGate => {
            let udid = places.simulator.clone().unwrap_or_default();
            let ios = places.target.join("openagents-ios");
            let derived = ios.join("DerivedData");
            let result = places.out.join("ReleaseGate.xcresult");
            let mut build = step(
                "Build the app and its Rust library for the simulator",
                path(&places.checkout.join("bins/openagents-ios/build.sh")),
                &["sim"],
            );
            build.env = vec![
                cargo_target.clone(),
                ("OPENAGENTS_IOS_DEVICE".into(), udid.clone()),
                ("OPENAGENTS_IOS_OUTPUT".into(), path(&ios)),
            ];
            let uninstall = Step {
                optional: true,
                ..step(
                    "Start the gate from a fresh install",
                    "xcrun",
                    &["simctl", "uninstall", &udid, "com.openagents.app"],
                )
            };
            let mut gate = step(
                "Run the release gate UI tests",
                "xcodebuild",
                &[
                    "test",
                    "-project",
                    &path(
                        &places
                            .checkout
                            .join("bins/openagents-ios/host/OpenAgents.xcodeproj"),
                    ),
                    "-scheme",
                    "OpenAgents",
                    "-configuration",
                    "Release",
                    "-destination",
                    &format!("id={udid}"),
                    "-derivedDataPath",
                    &path(&derived),
                    "-resultBundlePath",
                    &path(&result),
                    "-only-testing:OpenAgentsUITests/ReleaseGateUITests",
                    &format!(
                        "OPENAGENTS_RUST_LIBRARY_DIR={}",
                        path(&places.target.join("aarch64-apple-ios-sim/debug"))
                    ),
                    "CODE_SIGN_IDENTITY=-",
                ],
            );
            gate.env = vec![
                cargo_target,
                ("TEST_RUNNER_OPENAGENTS_UITEST_LIVE".into(), "1".into()),
                (
                    "TEST_RUNNER_OPENAGENTS_UITEST_SHOTS".into(),
                    path(&places.out.join("shots")),
                ),
            ];
            Plan {
                steps: vec![
                    build,
                    uninstall,
                    gate,
                    summary_step(&result, &places.out),
                    zip_app(
                        &derived.join("Build/Products/Release-iphonesimulator/OpenAgents.app"),
                        &places.out.join("OpenAgents-simulator.app.zip"),
                    ),
                ],
                collect: Vec::new(),
            }
        }
        Recipe::IosTestflight => {
            let mut args = vec!["run".to_owned()];
            args.extend(spec.args.iter().cloned());
            let ios = places.target.join("openagents-ios");
            let mut ship = step(
                "Archive, sign, and send the build to App Store Connect",
                path(&places.checkout.join("scripts/release/testflight.sh")),
                &[],
            );
            ship.args = args;
            ship.env = vec![
                cargo_target,
                ("OPENAGENTS_IOS_OUTPUT".into(), path(&ios)),
                (
                    "OPENAGENTS_SHIP_DIR".into(),
                    path(&places.target.join("ship")),
                ),
            ];
            Plan {
                steps: vec![ship],
                collect: vec![
                    Collect {
                        dir: ios.join("upload"),
                        extensions: vec!["ipa"],
                    },
                    Collect {
                        dir: ios.join("validate"),
                        extensions: vec!["ipa"],
                    },
                ],
            }
        }
        Recipe::DesktopCapture => {
            let flag = if spec.args.iter().any(|a| a == "--kept") {
                "--capture-kept"
            } else {
                "--capture"
            };
            let mut capture = step(
                "Build the desktop app and capture its screens",
                "cargo",
                &[
                    "run",
                    "--locked",
                    "-p",
                    "openagents-desktop",
                    "--",
                    flag,
                    &path(&places.out.join("desktop")),
                ],
            );
            capture.env = vec![cargo_target];
            Plan {
                steps: vec![capture],
                collect: Vec::new(),
            }
        }
        Recipe::Xcodebuild => {
            let tests = spec
                .args
                .iter()
                .any(|a| matches!(a.as_str(), "test" | "test-without-building"));
            let result = places.out.join("Result.xcresult");
            // Paths the job named are inside its checkout.
            let mut args: Vec<String> = Vec::new();
            let mut words = spec.args.iter();
            while let Some(word) = words.next() {
                args.push(word.clone());
                if matches!(word.as_str(), "-project" | "-workspace")
                    && let Some(value) = words.next()
                {
                    args.push(path(&places.checkout.join(value)));
                }
            }
            args.extend([
                "-derivedDataPath".to_owned(),
                path(&places.target.join("DerivedData")),
            ]);
            if tests {
                args.extend(["-resultBundlePath".to_owned(), path(&result)]);
            }
            let mut build = step("Run xcodebuild", "xcodebuild", &[]);
            build.args = args;
            build.env = vec![cargo_target];
            let mut steps = vec![build];
            if tests {
                steps.push(summary_step(&result, &places.out));
            }
            Plan {
                steps,
                collect: vec![Collect {
                    dir: places.target.join("DerivedData/Build/Products"),
                    extensions: vec!["zip"],
                }],
            }
        }
    }
}

/// The test results' summary as JSON, beside the result bundle.
fn summary_step(result: &Path, out: &Path) -> Step {
    Step {
        stdout_to: Some(out.join("xcresult-summary.json")),
        optional: true,
        ..step(
            "Summarize the test results",
            "xcrun",
            &[
                "xcresulttool",
                "get",
                "test-results",
                "summary",
                "--path",
                &path(result),
                "--compact",
            ],
        )
    }
}

/// The simulator app, zipped, so it can be installed elsewhere.
fn zip_app(app: &Path, zip: &Path) -> Step {
    Step {
        optional: true,
        ..step(
            "Keep the simulator app",
            "ditto",
            &["-c", "-k", "--keepParent", &path(app), &path(zip)],
        )
    }
}

/// The job's result in one line, from the test summary when it ran tests.
#[must_use]
pub fn summary_line(out: &Path, finished_ok: bool) -> String {
    let summary = std::fs::read_to_string(out.join("xcresult-summary.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    if let Some(summary) = summary {
        let count = |key: &str| summary[key].as_u64().unwrap_or(0);
        let result =
            summary["result"]
                .as_str()
                .unwrap_or(if finished_ok { "Passed" } else { "Failed" });
        let total = count("totalTestCount");
        let passed = count("passedTests");
        let failed = count("failedTests");
        let skipped = count("skippedTests");
        let mut line = format!("{result}: {passed} of {total} tests passed");
        if failed > 0 {
            line.push_str(&format!(", {failed} failed"));
        }
        if skipped > 0 {
            line.push_str(&format!(", {skipped} skipped"));
        }
        line.push('.');
        return line;
    }
    if finished_ok {
        "Finished.".into()
    } else {
        "Stopped with an error.".into()
    }
}

/// The name an artifact is stored under, from its path inside the job's
/// output: folders joined with `-`, only letters, digits, `.`, `_`, `-`,
/// at most 96 characters. `None` when nothing is left.
#[must_use]
pub fn artifact_name(relative: &Path) -> Option<String> {
    let joined = relative
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("-");
    let clean: String = joined
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let clean = clean.trim_start_matches(['.', '-']).to_owned();
    if clean.is_empty() {
        return None;
    }
    if clean.len() <= 96 {
        return Some(clean);
    }
    // Keep the end, where the extension is.
    let cut = clean.len() - 96;
    Some(clean[cut..].trim_start_matches(['.', '-']).to_owned())
}
