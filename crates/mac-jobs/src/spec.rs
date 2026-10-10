//! The typed job: a repository, a ref, a recipe, and its allowlisted
//! arguments.

use serde::{Deserialize, Serialize};

/// What a job does, as the owner reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Build,
    Test,
    /// Sends something outside: waits for the owner's approval.
    Upload,
    Capture,
}

impl Kind {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Upload => "upload",
            Self::Capture => "capture",
        }
    }
}

/// The named recipes. Nothing else runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recipe {
    /// The iOS release gate UI tests (`ReleaseGateUITests`) on a fresh
    /// simulator.
    #[serde(rename = "ios-release-gate")]
    IosReleaseGate,
    /// A signed archive uploaded to TestFlight (`scripts/release/testflight.sh`).
    #[serde(rename = "ios-testflight")]
    IosTestflight,
    /// The desktop app's offscreen screen captures.
    #[serde(rename = "desktop-capture")]
    DesktopCapture,
    /// `xcodebuild` with allowlisted arguments.
    #[serde(rename = "xcodebuild")]
    Xcodebuild,
}

impl Recipe {
    pub const ALL: [Recipe; 4] = [
        Recipe::IosReleaseGate,
        Recipe::IosTestflight,
        Recipe::DesktopCapture,
        Recipe::Xcodebuild,
    ];

    /// The recipe's name, as jobs and the command line write it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::IosReleaseGate => "ios-release-gate",
            Self::IosTestflight => "ios-testflight",
            Self::DesktopCapture => "desktop-capture",
            Self::Xcodebuild => "xcodebuild",
        }
    }

    /// The recipe named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|recipe| recipe.name() == name)
    }

    /// The recipe as people read it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::IosReleaseGate => "iOS release gate",
            Self::IosTestflight => "TestFlight upload",
            Self::DesktopCapture => "Desktop captures",
            Self::Xcodebuild => "Xcode build",
        }
    }
}

/// A job as it is sent: the repository (`OWNER/NAME`), the ref, the recipe,
/// and its arguments.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub repo: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub recipe: Recipe,
    #[serde(default)]
    pub args: Vec<String>,
}

/// The most arguments a job takes.
const MAX_ARGS: usize = 32;
/// The longest argument, in bytes.
const MAX_ARG: usize = 200;

impl Spec {
    /// Check every field; the error says what is wrong, in plain words.
    ///
    /// # Errors
    /// The repository, the ref, or an argument isn't allowed.
    pub fn check(&self) -> Result<(), String> {
        if !valid_repo(&self.repo) {
            return Err("Name the repository as OWNER/NAME.".into());
        }
        if !valid_ref(&self.git_ref) {
            return Err(
                "Name the ref as a branch, a tag, or a commit (letters, digits, . _ / -).".into(),
            );
        }
        if self.args.len() > MAX_ARGS || self.args.iter().any(|a| a.len() > MAX_ARG) {
            return Err(format!(
                "A job takes at most {MAX_ARGS} arguments of {MAX_ARG} characters."
            ));
        }
        if self.args.iter().any(|a| a.chars().any(char::is_control)) {
            return Err("An argument holds a control character.".into());
        }
        match self.recipe {
            Recipe::IosReleaseGate => release_gate_args(&self.args).map(|_| ()),
            Recipe::IosTestflight => testflight_args(&self.args).map(|_| ()),
            Recipe::DesktopCapture => capture_args(&self.args).map(|_| ()),
            Recipe::Xcodebuild => xcodebuild_args(&self.args).map(|_| ()),
        }
    }

    /// What the job does.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self.recipe {
            Recipe::IosReleaseGate => Kind::Test,
            Recipe::IosTestflight => Kind::Upload,
            Recipe::DesktopCapture => Kind::Capture,
            Recipe::Xcodebuild => {
                if self
                    .args
                    .iter()
                    .any(|a| matches!(a.as_str(), "test" | "test-without-building"))
                {
                    Kind::Test
                } else {
                    Kind::Build
                }
            }
        }
    }

    /// Whether the job reaches outside the Mac (a store upload, or
    /// App Store Connect's validation), so it waits for the owner.
    #[must_use]
    pub fn outward(&self) -> bool {
        self.kind() == Kind::Upload
    }

    /// The job in one line: the recipe, the repository, and the ref.
    #[must_use]
    pub fn title(&self) -> String {
        let repo = self.repo.rsplit('/').next().unwrap_or(&self.repo);
        format!("{} · {repo}@{}", self.recipe.label(), self.git_ref)
    }

    /// The simulator a release gate asks for (`--simulator NAME`).
    #[must_use]
    pub fn simulator(&self) -> Option<&str> {
        release_gate_args(&self.args).ok().flatten()
    }
}

/// Job ids: `mjob` and 32 lowercase hex digits.
#[must_use]
pub fn valid_job_id(id: &str) -> bool {
    id.len() == 36
        && id.starts_with("mjob")
        && id[4..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `OWNER/NAME`, GitHub's characters.
fn valid_repo(repo: &str) -> bool {
    let Some((owner, name)) = repo.split_once('/') else {
        return false;
    };
    !owner.is_empty()
        && owner.len() <= 39
        && !owner.starts_with('-')
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !name.is_empty()
        && name.len() <= 100
        && !name.starts_with(['.', '-'])
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A branch, tag, or commit name git accepts and no option can hide in.
fn valid_ref(git_ref: &str) -> bool {
    !git_ref.is_empty()
        && git_ref.len() <= 200
        && !git_ref.starts_with(['-', '/', '.'])
        && !git_ref.ends_with(['/', '.'])
        && !git_ref.ends_with(".lock")
        && !git_ref.contains("..")
        && !git_ref.contains("//")
        && !git_ref.contains("/.")
        && git_ref
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'))
}

/// `--simulator NAME`: an iPhone model name such as "iPhone 17 Pro Max".
fn release_gate_args(args: &[String]) -> Result<Option<&str>, String> {
    match args {
        [] => Ok(None),
        [flag, name] if flag == "--simulator" && simulator_name(name) => Ok(Some(name)),
        _ => Err("ios-release-gate takes only --simulator NAME.".into()),
    }
}

fn simulator_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '(' | ')' | '.' | '-'))
}

/// `--validate-only` and `--build N`.
fn testflight_args(args: &[String]) -> Result<(), String> {
    let mut words = args.iter();
    let mut seen_validate = false;
    let mut seen_build = false;
    while let Some(word) = words.next() {
        match word.as_str() {
            "--validate-only" if !seen_validate => seen_validate = true,
            "--build" if !seen_build => {
                let number = words.next().map(String::as_str).unwrap_or_default();
                if number.is_empty()
                    || number.len() > 6
                    || !number.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err("--build takes a build number.".into());
                }
                seen_build = true;
            }
            _ => return Err("ios-testflight takes only --validate-only and --build N.".into()),
        }
    }
    Ok(())
}

/// `--kept`: the captures of every kept screen.
fn capture_args(args: &[String]) -> Result<(), String> {
    match args {
        [] => Ok(()),
        [kept] if kept == "--kept" => Ok(()),
        _ => Err("desktop-capture takes only --kept.".into()),
    }
}

/// The `xcodebuild` actions a job may run. `archive` and `-exportArchive`
/// sign and ship, so they are only ever the TestFlight recipe's.
const XCODE_ACTIONS: [&str; 5] = [
    "build",
    "test",
    "build-for-testing",
    "test-without-building",
    "analyze",
];
/// Options that take the next argument.
const XCODE_VALUED: [&str; 7] = [
    "-project",
    "-workspace",
    "-scheme",
    "-configuration",
    "-destination",
    "-sdk",
    "-testPlan",
];
/// Options that stand alone.
const XCODE_SWITCHES: [&str; 2] = ["-quiet", "-showBuildTimingSummary"];
/// Build settings a job may set, with the values each may take.
const XCODE_SETTINGS: [(&str, &[&str]); 4] = [
    ("CODE_SIGN_IDENTITY", &["-"]),
    ("CODE_SIGNING_ALLOWED", &["NO"]),
    ("CODE_SIGNING_REQUIRED", &["NO"]),
    ("ONLY_ACTIVE_ARCH", &["YES", "NO"]),
];

/// A path inside the checkout: relative, no `..`, no option.
fn checkout_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '-', '~'])
        && path.split('/').all(|part| !part.is_empty() && part != "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '/'))
}

fn plain_value(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '/' | '(' | ')')
        })
}

/// Check `xcodebuild` arguments against the allowlist.
fn xcodebuild_args(args: &[String]) -> Result<(), String> {
    let mut words = args.iter();
    let mut actions = 0;
    while let Some(word) = words.next() {
        let word = word.as_str();
        if XCODE_ACTIONS.contains(&word) {
            actions += 1;
            continue;
        }
        if XCODE_SWITCHES.contains(&word) {
            continue;
        }
        if let Some(target) = word
            .strip_prefix("-only-testing:")
            .or_else(|| word.strip_prefix("-skip-testing:"))
        {
            if plain_value(target) {
                continue;
            }
            return Err(format!("{word} names an unusual test."));
        }
        if XCODE_VALUED.contains(&word) {
            let value = words.next().map(String::as_str).unwrap_or_default();
            let fine = match word {
                "-project" => checkout_path(value) && value.ends_with(".xcodeproj"),
                "-workspace" => checkout_path(value) && value.ends_with(".xcworkspace"),
                "-configuration" => matches!(value, "Debug" | "Release"),
                "-sdk" => matches!(value, "iphonesimulator" | "iphoneos" | "macosx"),
                "-destination" => {
                    !value.is_empty()
                        && !value.starts_with('-')
                        && value.chars().all(|c| {
                            c.is_ascii_alphanumeric()
                                || matches!(c, ' ' | '=' | ',' | '.' | '_' | '-' | ':' | '(' | ')')
                        })
                }
                _ => plain_value(value),
            };
            if !fine {
                return Err(format!("{word} doesn't take that value."));
            }
            continue;
        }
        if let Some((key, value)) = word.split_once('=')
            && let Some((_, values)) = XCODE_SETTINGS.iter().find(|(name, _)| *name == key)
        {
            if values.contains(&value) {
                continue;
            }
            return Err(format!("{key} may only be {}.", values.join(" or ")));
        }
        return Err(format!(
            "xcodebuild jobs don't take {word}. They take the actions {}, the options {}, \
             -only-testing:, -skip-testing:, {}, and the settings {}.",
            XCODE_ACTIONS.join(", "),
            XCODE_VALUED.join(", "),
            XCODE_SWITCHES.join(", "),
            XCODE_SETTINGS
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if actions == 0 {
        return Err("Name an xcodebuild action, such as build or test.".into());
    }
    Ok(())
}
