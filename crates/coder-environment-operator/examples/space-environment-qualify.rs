//! Disposable, real-provider qualification of the existing environment owners.

use coder_environment as env;
use coder_environment::evidence::Redactor;
use coder_environment_build::{BuildJob, Phase, service::BuildRequest};
use coder_environment_operator::{Config, Owners, ProviderKind, now_ms};
use coder_environment_verify::plan::{
    Assertions, Check, CheckKind, CheckPlan, Idempotence, SourceStep, Startup,
};
use coder_working_computer::Principal;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[path = "space-environment-qualify/publish.rs"]
mod publication;
#[path = "space-environment-qualify/verify.rs"]
mod space_verification;

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const ENVIRONMENT: &str = "environment-11811";
const RELEASE: &str = "3230ded51181aa438656825da8245cea6c8e717c";
const RUNTIME: &str = "07d30ec2a1b8fafa4a802e4de0615808bcfd428903473e916a9f8a227abc79f4";

fn owner() -> Principal {
    Principal {
        workspace: "qualification-11811".into(),
        principal: "synthetic-fixture".into(),
    }
}

fn install_script() -> String {
    let tools = include_str!("../../../deploy/space-host/install-environment-tools.sh");
    let body = format!(
        r#"set -euo pipefail
export PATH=/usr/local/bin:$PATH
export CARGO_TARGET_DIR="$PWD/target"
export OA_ENVIRONMENT_BUILD_GUEST=1
export OA_ENVIRONMENT_TOOLS_WORK="$HOME/.cache/oa-environment-tools-11811"
if [ ! -f /usr/local/share/oa-environment-fixture-ready ]; then
    mkdir -p "$HOME/.config/gh"
    printf %s oa-qualification-private-canary-11811 > "$HOME/.config/gh/hosts.yml"
fi
(
{tools}
)
rm -rf -- "$OA_ENVIRONMENT_TOOLS_WORK"
cargo build --release --locked --offline
cargo test --locked --offline
if [ "$(id -u)" = 0 ]; then
    touch /usr/local/share/oa-environment-fixture-ready
else
    sudo -n touch /usr/local/share/oa-environment-fixture-ready
fi
"#
    );
    format!("exec bash -c '{}'", body.replace('\'', "'\"'\"'"))
}

fn check_script() -> &'static str {
    r#"set -eu
export PATH=/usr/local/bin:$PATH
export CARGO_TARGET_DIR="$PWD/target"
test ! -e "$HOME/.config/gh/hosts.yml"
test "$(coder --version)" = 'coder 1.0.0 (3230ded51181aa438656825da8245cea6c8e717c clean)'
rm -rf target
cargo test --locked --offline
cargo build --release --locked --offline
test -s target/release/libenvironment_fixture.rlib
"#
}

fn plan(check: String) -> CheckPlan {
    CheckPlan {
        schema: coder_environment_verify::plan::PLAN_SCHEMA.into(),
        profile: "rust-library".into(),
        source: SourceStep::Contained,
        offline: true,
        startup: Startup::NotApplicable {
            reason: "This repository is a Rust library.".into(),
        },
        checks: vec![Check {
            name: "fresh-rust-build-and-tests".into(),
            kind: CheckKind::Behavior,
            script: check,
            cwd: ".".into(),
            timeout_seconds: 120,
            assertions: Assertions::CargoTest { min_passed: 3 },
        }],
        idempotence: Idempotence {
            inventory: vec![
                "Cargo.lock".into(),
                "/usr/local/bin/coder".into(),
                "/usr/local/bin/rustc".into(),
            ],
        },
    }
}

async fn open(
    state: &Path,
    base: Option<String>,
) -> Result<Owners<coder_working_computer::boat::BoatProvider>> {
    let config = Config {
        schema: coder_environment_operator::SCHEMA.into(),
        provider: ProviderKind::Boat,
        gce: None,
        workdir: "/var/tmp/oa-environment-11811".into(),
        template: base,
        credential_names: BTreeSet::new(),
        tick_seconds: 1,
    };
    let providers = coder_environment_operator::boat::providers(&config).await?;
    Ok(Owners::open(
        state,
        providers,
        Arc::new(|_| Ok(Redactor::new())),
    )?)
}

async fn until_published(
    owners: &Owners<coder_working_computer::boat::BoatProvider>,
    mut job: BuildJob,
) -> Result<BuildJob> {
    while !job.phase.terminal() && job.phase != Phase::PublishingSpace {
        tokio::time::sleep(Duration::from_millis(500)).await;
        job = owners.builder.advance(&job.id, now_ms()).await?;
    }
    println!(
        "{}",
        json!({"job":job.id,"phase":job.phase,"build":job.inputs.build_id,"image":job.image,"reason":job.reason})
    );
    if matches!(job.phase, Phase::Failed | Phase::Cancelled) {
        return Err("The environment build did not finish.".into());
    }
    Ok(job)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let action = args
        .get(1)
        .ok_or("Choose prepare, status, revise, publish, apply, verify, or cancel.")?;
    let state = PathBuf::from(
        args.get(2)
            .ok_or("Choose a private qualification directory.")?,
    );
    let hostname = String::from_utf8(std::process::Command::new("hostname").output()?.stdout)?;
    if !(hostname.starts_with("oa-lnd-builder") || hostname.starts_with("oa-space-spike-11811"))
        || !state.is_absolute()
        || !state.to_string_lossy().contains("11811")
    {
        return Err("Use an isolated qualification host and directory.".into());
    }
    if action == "publish" {
        return publication::run(&state, &args[3..]).await;
    }
    if action == "space-verify" {
        return space_verification::run(&state, &args[3..]).await;
    }
    if action == "space-cleanup" {
        return publication::cleanup(&state, &args[3..]).await;
    }
    if action == "retry-publication" {
        return publication::retry_failed(&state, &args[3..]).await;
    }
    let origin = std::env::var("BOAT_API_BASE")?;
    if !origin.starts_with("https://oa-boat-") || std::env::var("BOAT_API_KEY")?.is_empty() {
        return Err("Configure this project's Boat service for the qualification.".into());
    }
    let store = env::store::Store::under(state.join("environments"));
    let existing = match store.read(ENVIRONMENT) {
        Ok(env) => Some(env),
        Err(env::store::StoreError::NotFound) => None,
        Err(error) => return Err(error.into()),
    };
    let advertised = if existing.is_none() {
        let value = boat::Client::from_env().await?.capabilities().await?;
        let caps: computer_driver::Capabilities = serde_json::from_value(value)?;
        let artifact = caps
            .default_artifact
            .ok_or("Boat has no resolved base image.")?;
        artifact.validate()?;
        if artifact.kind != computer_driver::ArtifactKind::GceImage
            || artifact.provider_identity.is_none()
        {
            return Err("Boat must identify its exact GCE base image.".into());
        }
        Some(artifact)
    } else {
        None
    };
    let base = match &existing {
        Some(env) => env.draft().recipe.base.image_id.clone(),
        None => advertised.as_ref().unwrap().reference.clone(),
    };
    let owners = open(&state, Some(base.clone())).await?;
    if action == "prepare" && existing.is_none() {
        let source: Value = serde_json::from_slice(&fs::read(
            args.get(3).ok_or("Choose the synthetic source receipt.")?,
        )?)?;
        let source = env::SourcePin {
            repository: source["repository"].as_str().map(str::to_owned),
            revision: source["revision"]
                .as_str()
                .ok_or("The source commit is missing.")?
                .into(),
            digest: source["bundle_sha256"]
                .as_str()
                .ok_or("The source digest is missing.")?
                .into(),
        };
        let install = install_script();
        let check = owners.seal_artifact(check_script().as_bytes())?;
        let plan = plan(check);
        let plan_digest = owners.seal_artifact(&serde_json::to_vec(&plan)?)?;
        let base_record = advertised.as_ref().unwrap();
        fs::write(
            state.join("base-artifact.json"),
            serde_json::to_vec_pretty(base_record)?,
        )?;
        let recipe = env::Recipe {
            schema: env::RECIPE_SCHEMA.into(),
            base: env::ImagePin {
                provider: env::Provider::Boat,
                image_id: base,
                digest: env::digest(&serde_json::to_vec(&base_record)?),
            },
            runtime: env::ArtifactPin {
                revision: RELEASE.into(),
                digest: RUNTIME.into(),
            },
            platform: env::Platform {
                os: "linux".into(),
                architecture: "x86_64".into(),
            },
            install: env::Script {
                cwd: ".".into(),
                digest: env::digest(install.as_bytes()),
            },
            start: Default::default(),
            inputs: env::Inputs {
                toolchain: Some(
                    "88f28fa9af20594179f85d6df67078dfd6fa93e2f6da5e1e9b0ac4997988ca4f".into(),
                ),
                locks: BTreeMap::from([(
                    "Cargo.lock".into(),
                    env::digest(include_bytes!(
                        "../../../deploy/space-host/fixtures/environment/Cargo.lock"
                    )),
                )]),
            },
            credential_names: BTreeSet::new(),
            qualification: env::Qualification {
                profile: "rust-library".into(),
                plan_digest,
            },
            limits: env::Limits {
                deadline_seconds: 3600,
                concurrent_machines: 2,
                total_machine_allocations: 6,
                output_bytes: 32 * 1024 * 1024,
            },
            capture: env::capture::Capture {
                required: BTreeSet::from(["target/release/libenvironment_fixture.rlib".into()]),
                ..Default::default()
            },
        };
        let environment = env::Environment::new(
            ENVIRONMENT,
            env::ProjectLink {
                workspace: owner().workspace,
                project: "fixture".into(),
            },
            source,
            recipe,
            now_ms(),
        )?;
        owners.setup.environments.create(&environment)?;
        fs::write(
            owners
                .layout
                .setup_blobs()
                .join(env::digest(install.as_bytes())),
            &install,
        )?;
    }
    let mut environment = owners.setup.environments.read(ENVIRONMENT)?;
    if action == "revise" {
        let install = install_script();
        let mut recipe = environment.draft().recipe.clone();
        recipe.limits.total_machine_allocations = 8;
        recipe.install.digest = env::digest(install.as_bytes());
        let check = owners.seal_artifact(check_script().as_bytes())?;
        recipe.qualification.plan_digest =
            owners.seal_artifact(&serde_json::to_vec(&plan(check))?)?;
        fs::write(
            owners.layout.setup_blobs().join(&recipe.install.digest),
            &install,
        )?;
        owners.setup.environments.apply(
            ENVIRONMENT,
            &env::Command::UpdateRecipe {
                expected_draft_revision: environment.draft_revision,
                recipe,
            },
            now_ms(),
        )?;
        println!("{}", json!({"revised":true}));
        return Ok(());
    }
    environment = owners.setup.environments.read(ENVIRONMENT)?;
    let request = BuildRequest {
        request_id: format!(
            "qualification-build-11811-r{}{}",
            environment.draft_revision,
            args.get(4)
                .map(|suffix| format!("-{suffix}"))
                .unwrap_or_default()
        ),
        environment: ENVIRONMENT.into(),
        owner: owner(),
        expected_draft_revision: environment.draft_revision,
        size: "small".into(),
        space: true,
    };
    let selected_request = if action == "prepare" {
        request.request_id.as_str()
    } else {
        environment
            .builds
            .last()
            .ok_or("Start a qualification build first.")?
            .request_id
            .as_str()
    };
    let job_id = format!(
        "bj-{}",
        &env::digest(format!("{}\0{}", request.environment, selected_request).as_bytes())[..24]
    );
    match action.as_str() {
        "prepare" => {
            until_published(&owners, owners.build(&request, now_ms()).await?).await?;
        }
        "status" => println!(
            "{}",
            serde_json::to_string(&owners.builder.jobs.read(&job_id)?)?
        ),
        "apply" => {
            let artifact: env::artifacts::SpaceArtifact = serde_json::from_slice(&fs::read(
                args.get(3).ok_or("Choose the verified SPACE artifact.")?,
            )?)?;
            let job = owners
                .builder
                .publish_space(&job_id, artifact, now_ms())
                .await?;
            until_published(&owners, job).await?;
        }
        "verify" => {
            let build = owners.builder.jobs.read(&job_id)?;
            let mut job = owners
                .verify(
                    &coder_environment_verify::service::VerifyRequest {
                        request_id: format!(
                            "qualification-verify-11811{}",
                            args.get(3)
                                .map(|attempt| format!("-{attempt}"))
                                .unwrap_or_default()
                        ),
                        environment: ENVIRONMENT.into(),
                        build_id: build.inputs.build_id,
                        owner: owner(),
                        plan_digest: environment.draft().recipe.qualification.plan_digest.clone(),
                        size: "small".into(),
                    },
                    now_ms(),
                )
                .await?;
            while job.phase != coder_environment_verify::Phase::Done {
                tokio::time::sleep(Duration::from_millis(500)).await;
                job = owners.verifier.advance(&job.id, now_ms()).await?;
            }
            println!("{}", serde_json::to_string(&job)?);
            if !matches!(job.verdict, Some(coder_environment_verify::Verdict::Passed)) {
                return Err("The fresh Boat verification did not pass.".into());
            }
        }
        "cancel" => {
            let job = owners
                .builder
                .cancel(&job_id, "Disposable qualification cleanup", now_ms())
                .await?;
            println!("{}", serde_json::to_string(&job)?);
        }
        "save" => {
            let verification = environment
                .verifications
                .last()
                .ok_or("Verify the build first.")?;
            let candidate = environment.propose(&verification.id)?;
            fs::write(
                state.join("reviewed-candidate.json"),
                serde_json::to_vec_pretty(&candidate)?,
            )?;
            let now = now_ms();
            owners.setup.environments.apply(
                ENVIRONMENT,
                &env::Command::Promote {
                    request_id: "qualification-save-11811".into(),
                    expected_selection_revision: environment.selection.revision,
                    review: env::promotion::Review {
                        id: "qualification-review-11811".into(),
                        actor: owner().principal,
                        candidate,
                        granted_ms: now,
                        expires_ms: now + 60_000,
                    },
                },
                now,
            )?;
            let saved = owners.setup.environments.read(ENVIRONMENT)?;
            let version = saved.active().ok_or("The checked version was not saved.")?;
            fs::write(
                state.join("saved-version.json"),
                serde_json::to_vec_pretty(version)?,
            )?;
            println!("{}", serde_json::to_string(version)?);
        }
        "teardown" => {
            use coder_working_computer::provider::{Images, Outcome};
            let job = owners.builder.jobs.read(&job_id)?;
            if !job.phase.terminal() {
                return Err(
                    "Finish or cancel this qualification before deleting its image.".into(),
                );
            }
            owners
                .setup
                .environments
                .apply(ENVIRONMENT, &env::Command::Retire, now_ms())?;
            let until = now_ms() + 180_000;
            loop {
                let mut gone = true;
                for entry in fs::read_dir(owners.layout.computers())? {
                    let path = entry?.path();
                    if path
                        .extension()
                        .is_some_and(|extension| extension == "json")
                    {
                        let computer: coder_working_computer::Computer =
                            serde_json::from_slice(&fs::read(path)?)?;
                        if computer.owner != owner() {
                            return Err("This directory contains another owner's computer.".into());
                        }
                        if !matches!(computer.phase, coder_working_computer::Phase::Deleted) {
                            let settled = owners
                                .builder
                                .computers
                                .delete(&computer.id, now_ms())
                                .await?;
                            gone &= matches!(
                                settled.computer().phase,
                                coder_working_computer::Phase::Deleted
                            );
                        }
                    }
                }
                if gone {
                    break;
                }
                if now_ms() >= until {
                    return Err("The qualification computers are not all deleted.".into());
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            if let Some(image) = &job.image {
                let expected = job
                    .capture
                    .as_ref()
                    .and_then(|capture| capture.record.as_ref())
                    .ok_or("The owned image record is missing.")?;
                match owners
                    .builder
                    .computers
                    .provider
                    .read_image(&image.image_id)
                    .await
                {
                    Outcome::Done { value: None } => {}
                    Outcome::Done {
                        value: Some(actual),
                    } if actual.name == expected.name
                        && actual.source == expected.source
                        && actual.snapshot == expected.snapshot =>
                    {
                        match owners
                            .builder
                            .computers
                            .provider
                            .delete_image(&image.image_id)
                            .await
                        {
                            Outcome::Done { .. } => {}
                            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                                return Err(reason.into());
                            }
                        }
                    }
                    _ => {
                        return Err(
                            "The image's ownership could not be checked before deletion.".into(),
                        );
                    }
                }
                if !matches!(
                    owners
                        .builder
                        .computers
                        .provider
                        .read_image(&image.image_id)
                        .await,
                    Outcome::Done { value: None }
                ) {
                    return Err("The image deletion is not confirmed.".into());
                }
            }
            let receipt = json!({"environment":ENVIRONMENT,"retired":true,"working_computers_deleted":true,"image":job.image,"image_deleted":true});
            fs::write(
                state.join("qualification-teardown.json"),
                serde_json::to_vec_pretty(&receipt)?,
            )?;
            println!("{receipt}");
        }
        _ => {
            return Err(
                "Choose prepare, status, revise, publish, apply, verify, or cancel.".into(),
            );
        }
    }
    Ok(())
}
