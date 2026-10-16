use super::*;
use coder_environment::artifacts::{
    CleanBase, Compatibility, FreshVerification, SnapshotReadiness, SpaceArtifact,
};
use coder_environment::evidence::{CallIdentity, CallResult, Recorder, StreamName};
use coder_environment_operator::space::{BuildAccepted, BuildState, TemplateClient, template_name};
use computer_driver::{
    Artifact, ArtifactKind, CreateRequest, Driver, Exec, ExecOutput, Outcome, Secret, Shape, State,
};

fn done<T>(outcome: Outcome<T>) -> Result<T> {
    match outcome {
        Outcome::Done { value } => Ok(value),
        Outcome::Failed { reason } | Outcome::Unknown { reason } => Err(reason.into()),
    }
}

fn recorder(state: &Path, job: &BuildJob, segment: &str) -> Result<Recorder> {
    Ok(Recorder::create(
        state
            .join("environment-build/evidence")
            .join(&job.id)
            .join(segment),
        segment,
        Some(run_link(job)),
        Redactor::new(),
        job.inputs.evidence_budget.min(8 << 20),
    )?)
}

fn run_link(job: &BuildJob) -> env::RunLink {
    env::RunLink {
        cloud_job: job.computer.clone(),
        task: Some(job.id.clone()),
    }
}

fn start(
    rec: &mut Recorder,
    job: &BuildJob,
    rootfs: &str,
    computer: &str,
    id: &str,
    tool: &str,
    request: Value,
) -> Result<()> {
    rec.start_call(CallIdentity {
        id: id.into(), parent: None, run: run_link(job), tool: tool.into(),
        request: Some(job.id.clone()), operation: Some(computer.into()),
    }, &json!({"build_job":job.id,"inputs_digest":job.artifact_inputs_digest(),"rootfs_digest":rootfs,"request":request}), now_ms())?;
    Ok(())
}

fn result(rec: &mut Recorder, id: &str, output: &ExecOutput) -> Result<()> {
    rec.output(id, StreamName::Stdout, output.stdout.as_bytes())?;
    rec.output(id, StreamName::Stderr, output.stderr.as_bytes())?;
    rec.close_stream(id, StreamName::Stdout)?;
    rec.close_stream(id, StreamName::Stderr)?;
    rec.result(
        id,
        CallResult::Exited {
            code: Some(output.code),
            success: output.code == 0,
        },
        now_ms(),
    )?;
    if output.code != 0 {
        return Err(format!("The {id} check exited {}.", output.code).into());
    }
    Ok(())
}

async fn check(
    driver: &computer_driver::space::SpaceDriver,
    rec: &mut Recorder,
    job: &BuildJob,
    rootfs: &str,
    computer: &str,
    id: &str,
    tool: &str,
    script: String,
) -> Result<ExecOutput> {
    // The public command API runs as the seed's user account. These operator
    // checks inspect admission files and prepare a fresh project as root.
    let script = format!("sudo -n -H -- sh -c '{}'", script.replace('\'', "'\"'\"'"));
    start(
        rec,
        job,
        rootfs,
        computer,
        id,
        tool,
        json!({"command":script,"timeout_seconds":180}),
    )?;
    let mut exec = Exec::new(script);
    exec.timeout_seconds = 180;
    match driver.exec(computer, &exec).await {
        Outcome::Done { value } => {
            result(rec, id, &value)?;
            Ok(value)
        }
        Outcome::Unknown { reason } | Outcome::Failed { reason } => {
            rec.result(
                id,
                CallResult::Unresolved {
                    reason: reason.clone(),
                },
                now_ms(),
            )?;
            Err(reason.into())
        }
    }
}

/// Qualify an accepted publication with a distinct customer-driver computer.
/// A second invocation cannot silently reuse a partly checked guest.
pub(super) async fn run(state: &Path, args: &[String]) -> Result<()> {
    let job: BuildJob = serde_json::from_slice(&fs::read(
        args.first().ok_or("Choose the frozen build job.")?,
    )?)?;
    let compatibility: Compatibility = serde_json::from_slice(&fs::read(
        args.get(1).ok_or("Choose the measured runtime settings.")?,
    )?)?;
    let frozen_plan: CheckPlan = serde_json::from_slice(&fs::read(
        args.get(2).ok_or("Choose the frozen check plan.")?,
    )?)?;
    job.validate()?;
    let bundle = fs::read(args.get(4).ok_or("Choose the frozen source bundle.")?)?;
    if env::digest(&bundle) != job.inputs.source.digest {
        return Err("The source bundle does not match the build's source pin.".into());
    }
    compatibility.validate()?;
    frozen_plan.validate()?;
    if frozen_plan.checks.len() != 1
        || frozen_plan.checks[0].script != env::digest(check_script().as_bytes())
        || frozen_plan.checks[0].assertions != (Assertions::CargoTest { min_passed: 3 })
    {
        return Err("Use the qualification's frozen build and test check.".into());
    }
    if job.inputs.environment != ENVIRONMENT
        || job.phase != Phase::PublishingSpace
        || now_ms() >= job.deadline_ms
    {
        return Err("Use the current qualification build before its deadline.".into());
    }
    let work: Value = serde_json::from_slice(&fs::read(state.join("publication-work.json"))?)?;
    if work["job"] != job.id || work["inputs_digest"] != job.artifact_inputs_digest() {
        return Err("The publication belongs to different build inputs.".into());
    }
    let native_id = work["accepted"]["job_id"]
        .as_str()
        .ok_or("The publication has no accepted build.")?;
    let rootfs_hash = work["rootfs"]
        .as_str()
        .ok_or("The publication has no rootfs digest.")?;
    let rootfs = rootfs_hash
        .strip_prefix("sha256:")
        .ok_or("The rootfs digest is invalid.")?;
    let template = Artifact {
        kind: ArtifactKind::SpaceTemplate,
        reference: format!("{}@{rootfs_hash}", template_name(&job)),
        provider_identity: Some(rootfs_hash.into()),
        disk_gib: None,
    };
    template.validate()?;
    let client = TemplateClient::new(
        &std::env::var("SPACE_API_URL")?,
        Secret::new(std::env::var("SPACE_API_TOKEN")?),
    )?;
    let mut publication = recorder(state, &job, "space-publication")?;
    start(
        &mut publication,
        &job,
        rootfs,
        native_id,
        "build",
        "environment.space.build",
        json!({"accepted_build":native_id,"observation":"Read the accepted build after publication."}),
    )?;
    let status = done(
        client
            .status(&BuildAccepted {
                job_id: native_id.into(),
                template_name: template_name(&job),
            })
            .await,
    )?;
    let build_ok = matches!(status.state, BuildState::Succeeded)
        && status.content_hash.as_deref() == Some(rootfs_hash);
    result(
        &mut publication,
        "build",
        &ExecOutput {
            code: if build_ok { 0 } else { 1 },
            stdout: serde_json::to_string(
                &json!({"job_id":status.job_id,"template_name":status.template_name,"rootfs":status.content_hash,"succeeded":build_ok}),
            )?,
            stderr: String::new(),
        },
    )?;
    start(
        &mut publication,
        &job,
        rootfs,
        native_id,
        "publish",
        "environment.space.publish",
        json!({"template":template.reference,"observation":"Read the published template and clean base."}),
    )?;
    let published = done(client.template(&job).await)?;
    let base = published["snapshots"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row["vcpu"] == 2 && row["memory_mib"] == 4096)
        })
        .ok_or("The published image has no ready small boot image.")?;
    let snapshot = base["snapshot_hash"]
        .as_str()
        .ok_or("The boot image has no digest.")?
        .to_owned();
    let published_ok = published["content_hash"].as_str() == Some(rootfs_hash);
    result(
        &mut publication,
        "publish",
        &ExecOutput {
            code: if published_ok { 0 } else { 1 },
            stdout: serde_json::to_string(&published)?,
            stderr: String::new(),
        },
    )?;
    let publication_digest = publication.finish(now_ms())?.digest;

    let raw_privacy: Value = serde_json::from_slice(&fs::read(
        args.get(3)
            .ok_or("Inspect the published disk and base memory first.")?,
    )?)?;
    if raw_privacy["job"] != job.id
        || raw_privacy["inputs_digest"] != job.artifact_inputs_digest()
        || raw_privacy["rootfs_digest"] != rootfs
        || raw_privacy["snapshot_hash"] != snapshot
        || raw_privacy["passed"] != true
        || raw_privacy["canary_matches"] != 0
        || !raw_privacy["files"]
            .as_array()
            .is_some_and(|rows| rows.len() >= 3)
    {
        return Err(
            "The disk and base memory privacy check does not match this publication.".into(),
        );
    }

    let admission = &raw_privacy["admission"];
    let base_digest = work["base"]["provider_identity"]
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .ok_or("The SPACE base pin is missing.")?;
    let root_key = admission["root_authorized_keys_sha256"]
        .as_str()
        .filter(|value| env::valid_digest(value))
        .ok_or("The provider's root public key was not checked.")?;
    let user_key = admission["user_authorized_keys_sha256"]
        .as_str()
        .filter(|value| env::valid_digest(value))
        .ok_or("The provider's user public key was not checked.")?;
    if admission["base_rootfs_digest"] != base_digest {
        return Err("The public keys belong to a different provider base.".into());
    }

    let driver = computer_driver::space::SpaceDriver::new(
        &std::env::var("SPACE_API_URL")?,
        &std::env::var("SPACE_DATA_URL")?,
        Secret::new(std::env::var("SPACE_API_TOKEN")?),
        &template.reference,
    )?;
    let marker = state.join("fresh-verifier.json");
    if marker.exists() {
        return Err("A fresh verifier was already requested. Inspect and delete it before another qualification.".into());
    }
    let request = CreateRequest {
        operation: format!(
            "{}-fresh-space{}",
            job.id,
            args.get(5)
                .map(|attempt| format!("-{attempt}"))
                .unwrap_or_default()
        ),
        size: Some("small".into()),
        image: Some(template.reference.clone()),
        ttl_seconds: Some(900),
        labels: BTreeMap::new(),
    };
    fs::write(
        &marker,
        serde_json::to_vec_pretty(&json!({"request":request,"issued_ms":now_ms()}))?,
    )?;
    let computer = done(driver.create(&request).await)?;
    fs::write(
        &marker,
        serde_json::to_vec_pretty(
            &json!({"request":request,"computer":computer,"created_ms":now_ms()}),
        )?,
    )?;
    // Deletion runs after every check result, including a failed assertion.
    let checked = async {
        let mut privacy = recorder(state, &job, "space-privacy")?;
        check(&driver, &mut privacy, &job, rootfs, &computer, "privacy", "environment.space.privacy", format!(r#"set -eu
test ! -e /workspace
test ! -e /opt/oa-environment
for home in /root /home/*; do
  for file in .config/gh/hosts.yml .claude/.credentials.json .codex/auth.json .bash_history .zsh_history .npmrc .cargo/credentials .cargo/credentials.toml; do
    test ! -e "$home/$file"
  done
  if [ -d "$home/.ssh" ]; then
    test -z "$(find "$home/.ssh" -mindepth 1 ! -name authorized_keys -print)"
    test ! -L "$home/.ssh/authorized_keys"
  fi
done
printf '%s\n' '{root_key}  /root/.ssh/authorized_keys' '{user_key}  /home/user/.ssh/authorized_keys' | sha256sum --check
test -x /usr/local/bin/spaced-svc
test -s /etc/spaced/version
echo 'Fresh image has no checkout or account files; guest services are present.'
"#)).await?;
        start(&mut privacy,&job,rootfs,"qualification-host-11811","raw-privacy","environment.space.raw-privacy",json!({"observation":"Read the host's published disk and clean base memory inspection receipt."}))?;
        result(&mut privacy,"raw-privacy",&ExecOutput {code:0,stdout:serde_json::to_string(&raw_privacy)?,stderr:String::new()})?;
        let privacy_digest = privacy.finish(now_ms())?.digest;
        let mut verification = recorder(state, &job, "space-verify")?;
        let runtime = format!(r#"set -eu
export PATH=/usr/local/bin:$PATH
test "$(uname -sm)" = 'Linux x86_64'
test "$(coder --version)" = 'coder 1.0.0 ({RELEASE} clean)'
test "$(cat /etc/spaced/version)" = '0.8.0'
printf '%s\n' \
 '6bedc8cc2ea7626838b51ec3c12979383237528bc7ba0862e184679c38f0861d  /usr/local/bin/coder' \
 '675fdfac5ef5f8b6d343576fbd967c559cc9ebd9b8ef46f50d3432f8c90b7c64  /usr/local/bin/openagents' \
 'b05ecb338c980295f74b7ac3cb793cdaab1d461b42bb15c39ca55022854a9e43  /usr/local/bin/microcoder' \
 '{daemon}  /usr/local/bin/spaced-svc' | sha256sum --check
rustc --version
cargo --version
command -v git
command -v cc
command -v pkg-config
test -s /etc/ssl/certs/ca-certificates.crt
for file in /usr/local/bin/rustc /usr/local/bin/cargo /usr/bin/git /usr/bin/cc /etc/ssl/certs/ca-certificates.crt; do
  printf 'oa-tool '
  sha256sum "$file"
done
"#, daemon=compatibility.guest_daemon.digest);
        let runtime = check(&driver, &mut verification, &job, rootfs, &computer, "runtime", "environment.space.runtime", runtime).await?;
        let mut inventory = vec![];
        for line in runtime.stdout.lines().filter_map(|line| line.strip_prefix("oa-tool ")) {
            let (digest,path) = line.split_once("  ").ok_or("The tool hash output is invalid.")?;
            if !env::valid_digest(digest) || !path.starts_with('/') { return Err("The tool hash output is invalid.".into()); }
            inventory.push(env::NamedArtifact {name:path.into(),digest:digest.into()});
        }
        if inventory.len() != 5 { return Err("The fresh tool inventory is incomplete.".into()); }
        check(&driver, &mut verification, &job, rootfs, &computer, "shape", "environment.space.shape", "set -eu\ntest \"$(getconf _NPROCESSORS_ONLN)\" = 2\nawk '/^MemTotal:/ {print; if ($2 < 3800000 || $2 > 4194304) exit 1}' /proc/meminfo\n".into()).await?;
        let bundle_path = "/home/user/environment-11811-source.bundle";
        start(&mut verification,&job,rootfs,&computer,"source-upload","environment.space.source-upload",json!({"path":bundle_path,"digest":job.inputs.source.digest,"bytes":bundle.len()}))?;
        done(driver.put_file(&computer,bundle_path,&bundle).await)?;
        result(&mut verification,"source-upload",&ExecOutput {code:0,stdout:serde_json::to_string(&json!({"path":bundle_path,"digest":job.inputs.source.digest,"bytes":bundle.len()}))?,stderr:String::new()})?;
        let materialize = format!("printf '%s\\n' '{}  {bundle_path}' | sha256sum --check\ngit init -q .\ngit bundle verify {bundle_path}\ngit fetch -q --no-tags {bundle_path} {}\ngit checkout -q --detach {}\n",job.inputs.source.digest,job.inputs.source.revision,job.inputs.source.revision);
        let verify = coder_environment_operator::space::source_command(&job.inputs.source, coder_environment_setup::source::Mode::Verify)?;
        let source = check(&driver, &mut verification, &job, rootfs, &computer, "source", "environment.space.source", format!("set -eu\nmkdir -p /workspace\ncd /workspace\n(\n{materialize}\n)\n{verify}")).await?;
        if !coder_environment_setup::source::Report::parse(&source.stdout).is_some_and(|report| report.verified(&job.inputs.source)) { return Err("The fresh checkout did not match its source pin.".into()); }
        let check = check(&driver, &mut verification, &job, rootfs, &computer, "build-test", "environment.space.build-test", format!("set -eu\ncd /workspace\n{}", check_script())).await?;
        let mut tally = coder_environment_verify::plan::Tally::default();
        let assertions = Assertions::CargoTest { min_passed:3 };
        tally.feed(assertions, check.stdout.as_bytes()); tally.finish(assertions); tally.verdict(assertions)?;
        let digest = verification.finish(now_ms())?.digest;
        Ok::<_,Box<dyn Error>>((privacy_digest,digest,tally.passed,inventory))
    }.await;
    let mut cleanup = recorder(state, &job, "space-cleanup")?;
    start(
        &mut cleanup,
        &job,
        rootfs,
        &computer,
        "delete",
        "environment.space.cleanup",
        json!({"computer":computer}),
    )?;
    done(driver.delete(&computer).await)?;
    let until = now_ms() + 180_000;
    loop {
        if done(driver.observe(&computer).await)?.state == State::Gone {
            break;
        }
        if now_ms() >= until {
            return Err("The fresh verifier deletion is not confirmed.".into());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    result(
        &mut cleanup,
        "delete",
        &ExecOutput {
            code: 0,
            stdout: serde_json::to_string(&json!({"computer":computer,"state":"gone"}))?,
            stderr: String::new(),
        },
    )?;
    let cleanup_digest = cleanup.finish(now_ms())?.digest;
    let (privacy_digest, verification_digest, passed_tests, mut inventory) = checked?;
    inventory.extend([
        env::NamedArtifact {
            name: "space-base".into(),
            digest: base_digest.into(),
        },
        env::NamedArtifact {
            name: "provider-root-authorized-keys".into(),
            digest: root_key.into(),
        },
        env::NamedArtifact {
            name: "provider-user-authorized-keys".into(),
            digest: user_key.into(),
        },
        env::NamedArtifact {
            name: "rust-1.97.1-x86_64-unknown-linux-gnu".into(),
            digest: "88f28fa9af20594179f85d6df67078dfd6fa93e2f6da5e1e9b0ac4997988ca4f".into(),
        },
        env::NamedArtifact {
            name: "coder-1.0.0".into(),
            digest: "6bedc8cc2ea7626838b51ec3c12979383237528bc7ba0862e184679c38f0861d".into(),
        },
    ]);
    let shape = Shape {
        vcpus: 2,
        memory_mib: 4096,
        min_disk_gib: None,
    };
    let artifact = SpaceArtifact {
        template,
        rootfs_digest: rootfs.into(),
        inputs_digest: job.artifact_inputs_digest(),
        builder: native_id.into(),
        runtime: job.inputs.runtime.clone(),
        coder_version: "1.0.0".into(),
        harness: env::ArtifactPin {
            revision: "space-environment-qualify-v1".into(),
            digest: env::digest(&fs::read(std::env::current_exe()?)?),
        },
        inventory,
        compatibility: compatibility.clone(),
        snapshots: [(
            "small".into(),
            SnapshotReadiness::Ready {
                base: CleanBase {
                    shape: shape.clone(),
                    snapshot: env::NamedArtifact {
                        name: snapshot.clone(),
                        digest: snapshot.strip_prefix("sha256:").unwrap_or(&snapshot).into(),
                    },
                    compatibility_digest: compatibility.digest(),
                    privacy_evidence_digest: privacy_digest.clone(),
                },
            },
        )]
        .into(),
        verification: FreshVerification {
            computer: computer.clone(),
            size: "small".into(),
            actual_shape: shape,
            compatibility_digest: compatibility.digest(),
            rootfs_digest: rootfs.into(),
            inputs_digest: job.artifact_inputs_digest(),
            plan_digest: env::digest(&serde_json::to_vec(&frozen_plan)?),
            evidence_digest: verification_digest,
            passed_tests,
            cleanup_evidence_digest: cleanup_digest,
        },
        publication_evidence_digest: publication_digest,
        privacy_evidence_digest: privacy_digest,
    };
    artifact.validate()?;
    fs::write(
        state.join("space-artifact.json"),
        serde_json::to_vec_pretty(&artifact)?,
    )?;
    println!(
        "{}",
        json!({"job":job.id,"computer":computer,"passed_tests":passed_tests,"deleted":true,"artifact_digest":artifact.digest()})
    );
    Ok(())
}
