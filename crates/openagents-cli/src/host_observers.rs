//! Explicit native owner adapters for `openagents host serve`.

use std::path::PathBuf;

pub struct Options {
    pub arguments: Vec<String>,
    pub projects: Option<PathBuf>,
    pub cloud: Option<PathBuf>,
    /// `--environment-owners CONFIG`: the setup, build, and verify owners
    /// packaged beside the cloud operator (ENV-08).
    pub environment: Option<PathBuf>,
    pub state: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub policy: coder_access::RelayPolicy,
}
impl Options {
    pub fn take(arguments: &[String]) -> Result<Self, String> {
        let mut remaining = Vec::new();
        let mut projects = None;
        let mut cloud = None;
        let mut environment = None;
        let mut index = 0;
        while index < arguments.len() {
            let target = match arguments[index].as_str() {
                "--project-observer" => Some(&mut projects),
                "--cloud-operator" => Some(&mut cloud),
                "--environment-owners" => Some(&mut environment),
                _ => None,
            };
            if let Some(target) = target {
                if target.is_some() {
                    return Err("an owner adapter flag is repeated".into());
                }
                let path = arguments
                    .get(index + 1)
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .ok_or("an owner adapter requires an absolute configuration path")?;
                *target = Some(path);
                index += 2;
            } else {
                remaining.push(arguments[index].clone());
                index += 1;
            }
        }
        if (projects.is_some() || cloud.is_some())
            && (remaining.first().is_none_or(|argument| argument != "serve") || cfg!(not(unix)))
        {
            return Err("owner adapters apply only to a Unix resident host serve command".into());
        }
        let explicit = |flag| {
            remaining
                .windows(2)
                .find(|pair| pair[0] == flag)
                .map(|pair| PathBuf::from(&pair[1]))
        };
        if environment.is_some() && cloud.is_none() {
            return Err("environment owners run only beside a --cloud-operator".into());
        }
        let state = explicit("--state");
        let root = explicit("--root");
        if cloud.is_some()
            && (!state.as_ref().is_some_and(|path| path.is_absolute())
                || !root.as_ref().is_some_and(|path| path.is_absolute()))
        {
            return Err(
                "operator cloud requires explicit absolute --state and --root directories".into(),
            );
        }
        let policy = if remaining
            .iter()
            .any(|argument| argument == "--loopback-test")
        {
            coder_access::RelayPolicy::LoopbackTest
        } else {
            coder_access::RelayPolicy::Production
        };
        Ok(Self {
            arguments: remaining,
            projects,
            cloud,
            environment,
            state,
            root,
            policy,
        })
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn explicit_adapters_preserve_native_arguments_without_ambient_defaults() {
        let options = Options::take(&args(&[
            "serve",
            "--project-observer",
            "/private/projects.json",
            "--cloud-operator",
            "/private/cloud.json",
            "--state",
            "/private/access",
            "--root",
            "/private/host",
            "--loopback-test",
        ]))
        .unwrap();
        assert_eq!(
            options.projects,
            Some(PathBuf::from("/private/projects.json"))
        );
        assert_eq!(options.cloud, Some(PathBuf::from("/private/cloud.json")));
        assert_eq!(
            options.arguments,
            args(&[
                "serve",
                "--state",
                "/private/access",
                "--root",
                "/private/host",
                "--loopback-test"
            ])
        );
        assert!(
            Options::take(&args(&["serve", "--cloud-operator", "/private/cloud.json"])).is_err()
        );
        assert!(Options::take(&args(&["serve", "--project-observer", "relative.json"])).is_err());
        // Environment owners run only beside an explicit cloud operator.
        let packaged = Options::take(&args(&[
            "serve",
            "--cloud-operator",
            "/private/cloud.json",
            "--environment-owners",
            "/private/environment.json",
            "--state",
            "/private/access",
            "--root",
            "/private/host",
        ]))
        .unwrap();
        assert_eq!(
            packaged.environment,
            Some(PathBuf::from("/private/environment.json"))
        );
        assert!(
            Options::take(&args(&[
                "serve",
                "--environment-owners",
                "/private/environment.json"
            ]))
            .is_err()
        );
        assert!(
            Options::take(&args(&[
                "init",
                "--project-observer",
                "/private/projects.json"
            ]))
            .is_err()
        );
        assert!(
            Options::take(&args(&[
                "serve",
                "--project-observer",
                "/a",
                "--project-observer",
                "/b"
            ]))
            .is_err()
        );
    }
}
