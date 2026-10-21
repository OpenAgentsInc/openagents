#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `scripts/pylon/v41-stage-startup.sh` in its local mode (#12398): the
//! metadata, the bucket, `gcloud`, `nvidia-smi`, and the boot ID are fakes in
//! a temporary directory. A stage machine gets only its own shards, the
//! tokenizer files, and the group manifest, and installs a worker built for
//! its GPU only when the worker and its libraries match the build's
//! `BUILD.json` (#12407). Repeated boots never leave an earlier boot's ready
//! mark for `stage-run` or the head's ssh probe to trust.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const WORKER: &str = "bin/psionic-v41-stage/abc123/sm_120/psionic-v41-stage";
const PYLON: &str = "bin/pylon/abc123/pylon";
const LIBS: [&str; 2] = ["libcudart.so.13", "libcublas.so.13"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn script() -> PathBuf {
    repo().join("scripts/pylon/v41-stage-startup.sh")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> String {
    sha256(&std::fs::read(path).unwrap())
}

struct Machine {
    dir: tempfile::TempDir,
    manifest: String,
}

impl Machine {
    /// A bucket with a four-shard checkpoint, a worker build, and a
    /// manifest, and metadata for a stage of layers 0-10 with the
    /// embeddings.
    fn new(compute_cap: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let bucket = root.join("bucket");
        let index = serde_json::json!({"weight_map": {
            "embed.weight": "s1.safetensors",
            "layers.0.attn.wkv.weight": "s1.safetensors",
            "layers.9.attn.wkv.weight": "s2.safetensors",
            "layers.12.attn.wkv.weight": "s3.safetensors",
            "head.weight": "s4.safetensors",
        }});
        write(
            &bucket.join("w/model.safetensors.index.json"),
            &index.to_string(),
        );
        for file in [
            "config.json",
            "tokenizer.json",
            "tokenizer_config.json",
            "s1.safetensors",
            "s2.safetensors",
            "s3.safetensors",
            "s4.safetensors",
        ] {
            write(&bucket.join("w").join(file), file);
        }
        let manifest_bytes = br#"{"schema":"psionic.deepseek_v41.group_manifest.v1"}"#;
        let manifest = sha256(manifest_bytes);
        let worker_dir = bucket.join(Path::new(WORKER).parent().unwrap());
        std::fs::create_dir_all(worker_dir.join("manifests")).unwrap();
        std::fs::write(
            worker_dir
                .join("manifests")
                .join(format!("{manifest}.json")),
            manifest_bytes,
        )
        .unwrap();
        let root = root.to_path_buf();
        let machine = Self { dir, manifest };
        machine.upload_build("worker", &[]);
        let metadata = root.join("metadata");
        for (key, value) in [
            ("oa-weights", "gs://models/w/".to_string()),
            ("oa-layers", "0-10".into()),
            ("oa-embeddings", "1".into()),
            ("oa-head", "0".into()),
            ("oa-worker", format!("gs://models/{WORKER}")),
            ("oa-manifest", machine.manifest.clone()),
        ] {
            write(&metadata.join(key), &value);
        }
        // `gcloud storage cp [-I] SRC... DST` over the bucket directory.
        executable(
            &root.join("gcloud"),
            &format!(
                r#"#!/bin/sh
[ "$1 $2" = "storage cp" ] || exit 2
shift 2
bucket={bucket}
if [ "$1" = -I ]; then
  while read -r url; do cp "$bucket/${{url#gs://models/}}" "$2" || exit 1; done
  exit 0
fi
while [ "$#" -gt 1 ]; do
  # Unquoted, so a bucket wildcard matches as gcloud's does.
  cp $bucket/${{1#gs://models/}} "$(eval echo \"\${{$#}}\")" || exit 1
  shift
done
"#,
                bucket = bucket.display()
            ),
        );
        executable(
            &root.join("nvidia-smi"),
            &format!("#!/bin/sh\necho {compute_cap}\n"),
        );
        machine.reboot("boot-1");
        machine
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    fn bucket(&self, rel: &str) -> PathBuf {
        self.path("bucket").join(rel)
    }

    fn worker_dir(&self) -> PathBuf {
        self.bucket(Path::new(WORKER).parent().unwrap().to_str().unwrap())
    }

    /// Uploads a worker that prints `name` and a matching v2 `BUILD.json`
    /// listing the bucket's manifest and `extra_manifests`.
    fn upload_build(&self, name: &str, extra_manifests: &[&str]) -> String {
        let dir = self.worker_dir();
        let worker = format!("#!/bin/sh\necho {name} \"$LD_LIBRARY_PATH\" \"$@\"\n");
        executable(&dir.join("psionic-v41-stage"), &worker);
        let mut libraries = serde_json::Map::new();
        for lib in LIBS {
            let text = format!("{lib} for {name}");
            write(&dir.join("lib").join(lib), &text);
            libraries.insert(lib.into(), sha256(text.as_bytes()).into());
        }
        let mut manifests = serde_json::Map::new();
        for m in std::iter::once(self.manifest.as_str()).chain(extra_manifests.iter().copied()) {
            manifests.insert(
                m.into(),
                "crates/pylon/placements/deepseek-v41/x.json".into(),
            );
        }
        let build = serde_json::json!({
            "schema": "openagents.psionic_v41_stage.build.v2",
            "arch": "sm_120",
            "sha256": sha256(worker.as_bytes()),
            "libraries": libraries,
            "manifests": manifests,
            "profile": "cuda-device-sm_120",
        });
        let text = build.to_string();
        write(&dir.join("BUILD.json"), &text);
        sha256(text.as_bytes())
    }

    fn reboot(&self, id: &str) {
        write(&self.path("boot_id"), &format!("{id}\n"));
    }

    fn boot(&self) -> Output {
        Command::new("sh")
            .arg(script())
            .env("OA_STAGE_METADATA", self.path("metadata"))
            .env("OA_STAGE_ROOT", self.path("stage"))
            .env("OA_STAGE_OPT", self.path("opt"))
            .env("OA_GCLOUD", self.path("gcloud"))
            .env("OA_NVIDIA_SMI", self.path("nvidia-smi"))
            .env("OA_BOOT_ID", self.path("boot_id"))
            .output()
            .unwrap()
    }

    /// What `stage-run` prints, or `None` when it refuses at once.
    fn run_stage(&self) -> Option<String> {
        let run = Command::new(self.path("opt/stage-run"))
            .args(["--layers", "0-10"])
            .env("OA_STAGE_WAIT", "0")
            .output()
            .unwrap();
        run.status
            .success()
            .then(|| String::from_utf8_lossy(&run.stdout).trim().to_owned())
    }

    /// Whether the head's ssh probe finds this machine ready, through a
    /// fake `gcloud compute ssh` that runs the remote command here.
    fn head_probe_passes(&self) -> bool {
        executable(
            &self.path("ssh-gcloud"),
            r#"#!/bin/sh
while [ "$1" != -- ]; do shift; done
shift
while :; do
  case $1 in
    -o) shift 2 ;;
    -T) shift ;;
    *) break ;;
  esac
done
exec sh -c "$*" </dev/null
"#,
        );
        let out = Command::new(repo().join("scripts/pylon/v41-head-ssh.sh"))
            .args(["m", "z", "p"])
            .env("OA_GCLOUD", self.path("ssh-gcloud"))
            .env("OA_HEAD_SSH_WAIT", "1")
            .env("OA_STAGE_ROOT", self.path("stage"))
            .env("OA_STAGE_OPT", self.path("opt"))
            .env("OA_BOOT_ID", self.path("boot_id"))
            .output()
            .unwrap();
        // Past the probe, the session runs head-run, whose fake pylon prints
        // its arguments.
        String::from_utf8_lossy(&out.stdout).contains("cloud head")
    }

    /// Makes this the group head's machine, with a pylon build in the
    /// bucket.
    fn head_here(&self) -> String {
        let pylon = "#!/bin/sh\necho pylon \"$@\"\n";
        executable(&self.bucket(PYLON), pylon);
        let build = serde_json::json!({
            "schema": "openagents.pylon.build.v1",
            "sha256": sha256(pylon.as_bytes()),
        })
        .to_string();
        write(
            &self
                .bucket(Path::new(PYLON).parent().unwrap().to_str().unwrap())
                .join("BUILD.json"),
            &build,
        );
        write(&self.path("metadata/oa-group-head"), "1");
        write(
            &self.path("metadata/oa-pylon"),
            &format!("gs://models/{PYLON}"),
        );
        write(&self.path("metadata/oa-stage-ssh"), "#!/bin/sh\n");
        sha256(build.as_bytes())
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("stage/startup.log")).unwrap_or_default()
    }

    fn ready(&self) -> bool {
        self.path("stage/ready").exists()
    }

    fn build(&self) -> String {
        sha256_file(&self.worker_dir().join("BUILD.json"))
    }

    fn expected_run(&self, name: &str, build: &str) -> String {
        format!(
            "{name} {} --weights {} --layers 0-10",
            self.path(&format!("opt/builds/{build}/lib")).display(),
            self.path("stage/weights").display()
        )
    }
}

#[test]
fn a_stage_machine_gets_its_shards_tokenizer_manifest_and_worker() {
    let machine = Machine::new("12.0");
    let out = machine.boot();
    assert!(out.status.success(), "{}", machine.log());
    let weights = machine.path("stage/weights");
    for file in [
        "model.safetensors.index.json",
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "s1.safetensors",
        "s2.safetensors",
    ] {
        assert!(weights.join(file).is_file(), "{file}: {}", machine.log());
    }
    // Layers 12 and the head belong to other stages.
    assert!(!weights.join("s3.safetensors").exists());
    assert!(!weights.join("s4.safetensors").exists());
    assert!(
        weights
            .join(format!("manifests/{}.json", machine.manifest))
            .is_file()
    );
    let build = machine.build();
    let installed = machine.path(&format!("opt/builds/{build}"));
    assert_eq!(
        std::fs::metadata(installed.join("psionic-v41-stage"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0o111
    );
    for lib in LIBS {
        assert!(installed.join("lib").join(lib).is_file());
    }
    // The ready mark names this boot, the manifest, and the build.
    assert_eq!(
        std::fs::read_to_string(machine.path("stage/ready")).unwrap(),
        format!("boot boot-1\nmanifest {}\nbuild {build}", machine.manifest)
    );
    assert!(!machine.path("opt/build.staging").exists());
    // stage-run starts the worker on these weights with the head's arguments.
    assert_eq!(
        machine.run_stage().unwrap(),
        machine.expected_run("worker", &build)
    );
}

#[test]
fn a_worker_built_for_another_gpu_is_refused_before_any_weights_copy() {
    let machine = Machine::new("8.9");
    assert!(!machine.boot().status.success());
    assert!(
        machine
            .log()
            .contains("built for sm_120, but this GPU is sm_89"),
        "{}",
        machine.log()
    );
    assert!(!machine.path("stage/weights/s1.safetensors").exists());
    assert!(!machine.ready());
}

#[test]
fn a_manifest_that_does_not_match_its_digest_is_refused() {
    let machine = Machine::new("12.0");
    let zeros = "0".repeat(64);
    machine.upload_build("worker", &[&zeros]);
    write(&machine.path("metadata/oa-manifest"), &zeros);
    let manifests = machine.worker_dir().join("manifests");
    std::fs::copy(
        manifests.join(format!("{}.json", machine.manifest)),
        manifests.join(format!("{zeros}.json")),
    )
    .unwrap();
    assert!(!machine.boot().status.success());
    assert!(
        machine.log().contains("the group manifest hashes to"),
        "{}",
        machine.log()
    );
    assert!(!machine.ready());
}

#[test]
fn a_manifest_the_build_was_not_made_with_is_refused() {
    let machine = Machine::new("12.0");
    write(&machine.path("metadata/oa-manifest"), &"1".repeat(64));
    assert!(!machine.boot().status.success());
    assert!(
        machine.log().contains("not made with group manifest"),
        "{}",
        machine.log()
    );
    assert!(!machine.ready());
}

/// F2 of the 2026-10-21 audit: the worker, a library, or the build itself
/// replaced in the bucket is refused before anything is installed.
#[test]
fn a_replaced_worker_library_or_build_is_refused() {
    type Tamper = fn(&Machine);
    let cases: [(&str, Tamper, &str); 5] = [
        (
            "worker",
            |m| {
                executable(
                    &m.worker_dir().join("psionic-v41-stage"),
                    "#!/bin/sh\necho replacement-worker\n",
                );
            },
            "the worker hashes to",
        ),
        (
            "library",
            |m| write(&m.worker_dir().join("lib/libcublas.so.13"), "replaced"),
            "lib/libcublas.so.13 hashes to",
        ),
        (
            "extra library",
            |m| write(&m.worker_dir().join("lib/libcublasLt.so.13"), "extra"),
            "but BUILD.json lists",
        ),
        (
            "pinned build",
            |m| write(&m.path("metadata/oa-build"), &"2".repeat(64)),
            "but this group runs",
        ),
        (
            "old schema",
            |m| {
                write(
                    &m.worker_dir().join("BUILD.json"),
                    r#"{"schema":"openagents.psionic_v41_stage.build.v1","sha256":"x"}"#,
                );
            },
            "rebuild with",
        ),
    ];
    for (name, tamper, message) in cases {
        let machine = Machine::new("12.0");
        tamper(&machine);
        assert!(!machine.boot().status.success(), "{name}");
        assert!(machine.log().contains(message), "{name}: {}", machine.log());
        assert!(!machine.ready(), "{name}");
        assert!(!machine.path("opt/stage-run").exists(), "{name}");
        assert!(
            !machine.path("stage/weights/s1.safetensors").exists(),
            "{name}"
        );
    }
}

#[test]
fn a_pinned_build_that_matches_installs() {
    let machine = Machine::new("12.0");
    write(&machine.path("metadata/oa-build"), &machine.build());
    assert!(machine.boot().status.success(), "{}", machine.log());
    assert!(machine.run_stage().is_some());
}

/// F5 of the 2026-10-21 audit: a second boot that fails leaves nothing that
/// `stage-run` or the head's probe trusts, and a reboot invalidates the mark
/// before the startup script runs again.
#[test]
fn repeated_boots_never_leave_a_stale_ready_mark() {
    type Second = fn(&Machine);
    let failures: [(&str, Second); 4] = [
        ("failed download", |m| {
            std::fs::remove_file(m.bucket("w/s2.safetensors")).unwrap();
        }),
        ("bad manifest", |m| {
            let zeros = "0".repeat(64);
            m.upload_build("worker", &[&zeros]);
            write(&m.path("metadata/oa-manifest"), &zeros);
            let manifests = m.worker_dir().join("manifests");
            std::fs::copy(
                manifests.join(format!("{}.json", m.manifest)),
                manifests.join(format!("{zeros}.json")),
            )
            .unwrap();
        }),
        ("replaced worker", |m| {
            executable(
                &m.worker_dir().join("psionic-v41-stage"),
                "#!/bin/sh\necho replacement-worker\n",
            );
        }),
        ("interrupted install", |m| {
            std::fs::remove_dir_all(m.worker_dir().join("lib")).unwrap();
        }),
    ];
    for (name, second) in failures {
        let machine = Machine::new("12.0");
        machine.head_here();
        assert!(machine.boot().status.success(), "{name}: {}", machine.log());
        assert!(machine.run_stage().is_some(), "{name}");
        assert!(machine.head_probe_passes(), "{name}: first boot");
        second(&machine);
        assert!(!machine.boot().status.success(), "{name}");
        assert!(!machine.ready(), "{name}");
        assert_eq!(machine.run_stage(), None, "{name}");
        assert!(!machine.head_probe_passes(), "{name}: after a failed boot");
    }

    // A reboot: the old mark names another boot, so neither trusts it
    // before this boot's startup script runs.
    let machine = Machine::new("12.0");
    machine.head_here();
    assert!(machine.boot().status.success(), "{}", machine.log());
    machine.reboot("boot-2");
    assert!(machine.ready());
    assert_eq!(machine.run_stage(), None);
    assert!(!machine.head_probe_passes());
    assert!(machine.boot().status.success(), "{}", machine.log());
    assert!(machine.run_stage().is_some());
    assert!(machine.head_probe_passes());
}

#[test]
fn a_changed_build_replaces_the_installed_one_whole() {
    let machine = Machine::new("12.0");
    assert!(machine.boot().status.success(), "{}", machine.log());
    let first = machine.build();
    assert_eq!(
        machine.run_stage().unwrap(),
        machine.expected_run("worker", &first)
    );
    let second = machine.upload_build("worker-2", &[]);
    assert_ne!(first, second);
    assert!(machine.boot().status.success(), "{}", machine.log());
    assert_eq!(
        machine.run_stage().unwrap(),
        machine.expected_run("worker-2", &second)
    );
}

#[test]
fn stage_run_refuses_an_installed_build_changed_after_startup() {
    let machine = Machine::new("12.0");
    assert!(machine.boot().status.success(), "{}", machine.log());
    let build = machine.build();
    executable(
        &machine.path(&format!("opt/builds/{build}/psionic-v41-stage")),
        "#!/bin/sh\necho replacement-worker\n",
    );
    assert_eq!(machine.run_stage(), None);
}

#[test]
fn the_head_installs_only_a_pylon_that_matches_its_build() {
    let machine = Machine::new("12.0");
    let pylon_build = machine.head_here();
    write(&machine.path("metadata/oa-pylon-build"), &pylon_build);
    assert!(machine.boot().status.success(), "{}", machine.log());
    assert!(machine.head_probe_passes());

    let machine = Machine::new("12.0");
    machine.head_here();
    executable(&machine.bucket(PYLON), "#!/bin/sh\necho other\n");
    assert!(!machine.boot().status.success());
    assert!(
        machine.log().contains("does not match its BUILD.json"),
        "{}",
        machine.log()
    );
    assert!(!machine.path("opt/head-run").exists());
    assert!(!machine.ready());

    let machine = Machine::new("12.0");
    machine.head_here();
    write(&machine.path("metadata/oa-pylon-build"), &"3".repeat(64));
    assert!(!machine.boot().status.success());
    assert!(!machine.path("opt/head-run").exists());
}
