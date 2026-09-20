//! What the host found before it chose: the capabilities this machine can
//! reach, the programs it could run, and the sources those programs may
//! look work up in.
//!
//! The two reads belong together because the decision they feed is one
//! decision. A program-selection question's option set is built from
//! programs the host resolved, and a `delegate` step's executor is resolved
//! from what the probe found — so a survey is what a host knows about
//! itself at the moment it starts choosing, and the trace records both
//! reads before any decision call.
//!
//! Both resolve from local files. The relay comes later, and the registry
//! read already records the query it would send.

use std::path::{Path, PathBuf};

use crate::capability::{self, Found, Presence, Trust};
use crate::delegate::Executor;
use crate::program;
use crate::source;
use crate::trace::Recorder;

/// One machine's capabilities, programs, and task sources, read once.
#[derive(Clone, Debug)]
pub struct Survey {
    /// Every declared capability, probed against [`Survey::workspace`].
    pub capabilities: Vec<Found>,
    /// The programs this host would run.
    pub programs: program::Registry,
    /// The sources a `query` step may name. A program names a source and
    /// a host resolves it here, the way a `decide` step names a question
    /// and the wording is resolved from `questions/`.
    pub sources: source::Registry,
    /// The directory the capabilities were probed against. A capability
    /// that refuses one directory may accept another, so a survey is only
    /// true of the workspace it names.
    pub workspace: PathBuf,
}

impl Survey {
    /// Reads the manifests and programs a host can see, and probes each
    /// capability against `workspace` under the operator's trust.
    ///
    /// `repository` is the checkout the host is running in, whose
    /// `capabilities/`, `programs/`, and `sources/` directories are read
    /// before the operator's own. The read is inert and the probes are
    /// gated: a manifest the operator has not approved is recorded
    /// `unprobed` and its argv never runs. `capability-trust approve`
    /// is the approval path.
    #[must_use]
    pub fn read(repository: Option<&Path>, workspace: &Path) -> Self {
        Self::read_with(repository, workspace, &Trust::operator())
    }

    /// The same read, under a trust the caller chose. A host runs this
    /// with [`Trust::operator`]; a test runs it with a trust it can see.
    #[must_use]
    pub fn read_with(repository: Option<&Path>, workspace: &Path, trust: &Trust) -> Self {
        let capabilities =
            capability::Registry::open(&capability::search(repository)).probe_all(workspace, trust);
        let programs = program::Registry::open(&program::search(repository));
        let sources = source::Registry::open(&source::search(repository));
        Survey {
            capabilities,
            programs,
            sources,
            workspace: workspace.to_path_buf(),
        }
    }

    /// The capabilities a host may offer as routes.
    ///
    /// Absent capabilities and present ones that refuse this workspace are
    /// both left out, because neither is a route. An operator without Devin
    /// gets a shorter option set rather than a broken one.
    #[must_use]
    pub fn options(&self) -> Vec<&Found> {
        capability::options(&self.capabilities)
    }

    /// One capability by slug, whatever state it is in.
    #[must_use]
    pub fn capability(&self, slug: &str) -> Option<&Found> {
        self.capabilities
            .iter()
            .find(|found| found.capability() == slug)
    }

    /// The executor for one capability, built from what the probe
    /// resolved.
    ///
    /// `None` unless the capability is present and the manifest says how
    /// to drive it. An absent executor and one refusing this workspace both
    /// answer `None`, because a route that cannot be taken is not a route,
    /// and a manifest with no `invoke` describes something without saying
    /// how to use it.
    #[must_use]
    pub fn executor(&self, slug: &str) -> Option<Executor> {
        executor(self.capability(slug)?)
    }

    /// Writes both reads to the session's trace, probes first.
    ///
    /// This is the order the golden records and the order the work
    /// happens: a host resolves what it can reach, then what it could run,
    /// then asks which program applies.
    pub fn record(&self, recorder: &mut Recorder, operator: Option<&str>) {
        for found in &self.capabilities {
            recorder.check(&found.message(), found.call());
        }
        recorder.check(&self.programs.message(), self.programs.call(operator));
    }
}

/// One probed capability as something a delegation can be handed to.
///
/// This is the seam the fan-out needs: the binary is the absolute path the
/// probe resolved rather than a name in the source, the arguments are the
/// manifest's `invoke`, and the refusals are the ones the manifest
/// declares. A machine without the executor produces no executor, which is
/// how an absent capability drops out of a fan-out instead of failing in
/// one.
#[must_use]
pub fn executor(found: &Found) -> Option<Executor> {
    let Presence::Present { path, .. } = &found.presence else {
        return None;
    };
    let manifest = &found.manifest;
    let (_, arguments) = manifest.invoke.split_first()?;
    let executor = Executor::new(&manifest.slug, path.clone(), arguments.to_vec());
    Some(manifest.refuses.iter().fold(executor, |executor, refusal| {
        executor.refusing(&refusal.name, &refusal.matches)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn a_survey_reads_the_repositorys_capabilities_and_programs() {
        let workspace = tempfile::tempdir().unwrap();
        // An empty trust probes nothing, so the repository's manifest is
        // declared and unprobed rather than run — the read is inert and
        // the answer does not depend on what this operator has approved.
        let survey = Survey::read_with(Some(&repository()), workspace.path(), &Trust::empty());

        assert!(
            survey.capability("devin-local").is_some(),
            "the repository declares devin-local"
        );
        assert_eq!(survey.programs.programs().len(), 4);
        assert!(survey.programs.get("delegate-fan-out").is_some());
        assert!(
            survey.sources.get("work-list").is_some(),
            "the repository declares a task source"
        );
    }

    /// A checkout cannot supply the approval its own manifest needs. The
    /// production read is `Survey::read`, which decides under the
    /// operator's store — a `capability-trust.json` the repository ships,
    /// even a real record copied in, is repository data and nobody's
    /// trust. `Trust::everything` is a test's word; no file becomes it.
    #[test]
    fn a_repositorys_own_trust_file_approves_nothing() {
        let outside = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let repository = tempfile::tempdir().unwrap();
        let capabilities = repository.path().join("capabilities");
        std::fs::create_dir_all(&capabilities).unwrap();
        let marker = workspace.path().join("it-ran");
        std::fs::write(
            capabilities.join("side-effect.json"),
            format!(
                r#"{{"v":1,"slug":"side-effect","name":"A","transport":"subprocess","detect":{{"binary":"sh","version":["sh","-c","touch {}; echo side-effect 1.0.0"]}}}}"#,
                marker.display()
            ),
        )
        .unwrap();

        // A real record, written by an approval outside the repository —
        // then copied into it, where it is just a file the checkout ships.
        let mut approved = Trust::load(&outside.path().join("capability-trust.json")).unwrap();
        approved
            .approve(Some(repository.path()), "side-effect", &[])
            .unwrap();
        std::fs::copy(
            outside.path().join("capability-trust.json"),
            repository.path().join("capability-trust.json"),
        )
        .unwrap();

        let survey = Survey::read(Some(repository.path()), workspace.path());
        let found = survey.capability("side-effect").expect("declared");
        assert!(
            matches!(found.presence, Presence::Unprobed { .. }),
            "the production read consults the operator's store, not the checkout's: {:?}",
            found.presence
        );
        assert!(!marker.exists(), "nothing ran for the unapproved manifest");
    }

    /// A machine with nothing declared surveys cleanly. Absence is not an
    /// error anywhere on this path.
    #[test]
    fn a_machine_with_no_manifests_surveys_to_nothing() {
        let empty = tempfile::tempdir().unwrap();
        let survey = Survey::read_with(Some(empty.path()), empty.path(), &Trust::empty());

        assert!(survey.capabilities.is_empty());
        assert!(survey.options().is_empty());
        assert!(survey.programs.programs().is_empty());
        assert!(survey.sources.slugs().is_empty());
        assert!(
            survey.sources.get("request").is_some(),
            "the work a request carried is a source a machine has without declaring one"
        );
    }

    #[test]
    fn both_reads_reach_the_trace_as_checks() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
        let path = recorder.path().to_path_buf();

        Survey::read_with(Some(&repository()), workspace.path(), &Trust::empty())
            .record(&mut recorder, None);
        recorder.finish(atif::log::ENDED);

        let recording = atif::log::read(&path).expect("the trace reads back");
        let names: Vec<&str> = recording
            .steps
            .iter()
            .filter_map(|step| step.call.as_ref())
            .map(|call| call.name.as_str())
            .collect();
        assert!(names.contains(&capability::PROBE_CALL));
        assert!(names.contains(&program::REGISTRY_CALL));
        assert!(
            recording
                .steps
                .iter()
                .filter_map(|step| step.call.as_ref())
                .all(|call| !call.is_decision()),
            "a probe and a registry read are checks, not decisions"
        );
    }

    /// A manifest whose executor is `/bin/sh`, so a test can build an
    /// executor out of a probe without the real CLI.
    fn shim() -> capability::Manifest {
        capability::Manifest {
            v: 1,
            slug: "shim".to_string(),
            name: "A shim".to_string(),
            summary: String::new(),
            transport: "subprocess".to_string(),
            detect: capability::Detect {
                binary: "sh".to_string(),
                version: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo shim 1.2.3".to_string(),
                ],
                probe: None,
            },
            enforces: vec!["minutes".to_string()],
            cannot_enforce: vec!["tool_set".to_string()],
            sees_repository: true,
            concurrent_max: Some(2),
            cost: "local".to_string(),
            isolation: vec!["directory".to_string()],
            invoke: vec!["sh".to_string(), "-c".to_string()],
            workspace_probe: None,
            refuses: vec![capability::Refusal {
                name: "untrusted_workspace".to_string(),
                matches: "Refusing to run in an untrusted workspace".to_string(),
                explanation: String::new(),
            }],
        }
    }

    #[test]
    fn an_executor_is_built_from_what_the_probe_resolved() {
        let found = shim().probe(Path::new("/"));
        let executor = executor(&found).expect("a present capability drives an executor");

        assert_eq!(executor.capability, "shim");
        assert!(
            executor.binary.is_absolute(),
            "the delegation runs the path the probe resolved, not a name"
        );
        assert_eq!(executor.arguments, ["-c"]);
        assert_eq!(
            executor.refusal("Refusing to run in an untrusted workspace: /tmp"),
            Some("untrusted_workspace".to_string()),
            "the refusal the manifest declares is the one a delegation reads"
        );
    }

    #[test]
    fn a_capability_that_is_not_a_route_drives_nothing() {
        let mut absent = shim();
        let binary = "no-such-executor-openagents".to_string();
        absent.detect.version = vec![binary.clone(), "--version".to_string()];
        absent.detect.binary = binary;
        assert!(executor(&absent.probe(Path::new("/"))).is_none());

        let mut refusing = shim();
        refusing.workspace_probe = Some(capability::WorkspaceProbe {
            argv: vec![
                "sh".to_string(),
                "-c".to_string(),
                "echo Refusing to run in an untrusted workspace >&2".to_string(),
            ],
            accepts: Vec::new(),
            note: String::new(),
        });
        let found = refusing.probe(Path::new("/"));
        assert_eq!(found.presence.state(), "present_unavailable");
        assert!(
            executor(&found).is_none(),
            "an executor that refuses this directory is not offered for it"
        );
    }

    /// The two calls the `devin-fan-out-six` task grades on, read back by
    /// the grader that reports them missing.
    #[test]
    fn coderbench_reads_both_reads_as_the_checks_the_task_names() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let mut recorder = Recorder::open(dir.path(), "a-model", "stub", "/tmp/repo").unwrap();
        let path = recorder.path().to_path_buf();

        Survey::read_with(Some(&repository()), workspace.path(), &Trust::empty())
            .record(&mut recorder, None);
        recorder.finish(atif::log::ENDED);

        let run = coderbench::observe(&path).expect("the grader reads the trace");
        assert!(
            run.checks
                .iter()
                .any(|check| check.name == "capability_probe")
        );
        assert!(
            run.checks
                .iter()
                .any(|check| check.name == "program_registry")
        );
        assert!(
            run.writes.is_empty(),
            "neither read writes anything the grader would count"
        );
    }
}
