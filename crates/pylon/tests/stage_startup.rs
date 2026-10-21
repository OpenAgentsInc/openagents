#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
//! `scripts/pylon/v41-stage-startup.sh` in its local mode (#12398): the
//! metadata, the bucket, `gcloud`, and `nvidia-smi` are fakes in a temporary
//! directory. A stage machine gets only its own shards, the tokenizer files,
//! and the group manifest, and installs a worker built for its GPU.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const WORKER: &str = "bin/psionic-v41-stage/abc123/sm_120/psionic-v41-stage";

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/pylon/v41-stage-startup.sh")
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

struct Machine {
    dir: tempfile::TempDir,
    manifest: String,
}

impl Machine {
    /// A bucket with a four-shard checkpoint, a worker, and a manifest, and
    /// metadata for a stage of layers 0-10 with the embeddings.
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
        executable(
            &bucket.join(WORKER),
            "#!/bin/sh\necho worker \"$LD_LIBRARY_PATH\" \"$@\"\n",
        );
        let manifest_bytes = br#"{"schema":"psionic.deepseek_v41.group_manifest.v1"}"#;
        let manifest = format!("{:x}", Sha256::digest(manifest_bytes));
        let worker_dir = Path::new(WORKER).parent().unwrap();
        std::fs::create_dir_all(bucket.join(worker_dir).join("manifests")).unwrap();
        for lib in ["libcudart.so.13", "libcublas.so.13"] {
            write(&bucket.join(worker_dir).join("lib").join(lib), lib);
        }
        std::fs::write(
            bucket
                .join(worker_dir)
                .join("manifests")
                .join(format!("{manifest}.json")),
            manifest_bytes,
        )
        .unwrap();
        let metadata = root.join("metadata");
        for (key, value) in [
            ("oa-weights", "gs://models/w/".to_string()),
            ("oa-layers", "0-10".into()),
            ("oa-embeddings", "1".into()),
            ("oa-head", "0".into()),
            ("oa-worker", format!("gs://models/{WORKER}")),
            ("oa-manifest", manifest.clone()),
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
        Self { dir, manifest }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    fn boot(&self) -> Output {
        Command::new("sh")
            .arg(script())
            .env("OA_STAGE_METADATA", self.path("metadata"))
            .env("OA_STAGE_ROOT", self.path("stage"))
            .env("OA_STAGE_OPT", self.path("opt"))
            .env("OA_GCLOUD", self.path("gcloud"))
            .env("OA_NVIDIA_SMI", self.path("nvidia-smi"))
            .output()
            .unwrap()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path("stage/startup.log")).unwrap_or_default()
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
    let worker = machine.path("opt/stage-worker");
    assert_eq!(
        std::fs::metadata(&worker).unwrap().permissions().mode() & 0o111,
        0o111
    );
    assert!(machine.path("stage/ready").is_file());
    assert!(machine.path("opt/lib/libcudart.so.13").is_file());
    assert!(machine.path("opt/lib/libcublas.so.13").is_file());
    // stage-run starts the worker on these weights with the head's arguments.
    let run = Command::new(machine.path("opt/stage-run"))
        .args(["--layers", "0-10"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&run.stdout).trim(),
        format!(
            "worker {} --weights {} --layers 0-10",
            machine.path("opt/lib").display(),
            weights.display()
        )
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
    assert!(!machine.path("stage/ready").exists());
}

#[test]
fn a_manifest_that_does_not_match_its_digest_is_refused() {
    let machine = Machine::new("12.0");
    write(&machine.path("metadata/oa-manifest"), &"0".repeat(64));
    let manifests = machine
        .path("bucket")
        .join(Path::new(WORKER).parent().unwrap())
        .join("manifests");
    std::fs::copy(
        manifests.join(format!("{}.json", machine.manifest)),
        manifests.join(format!("{}.json", "0".repeat(64))),
    )
    .unwrap();
    assert!(!machine.boot().status.success());
    assert!(
        machine.log().contains("the group manifest hashes to"),
        "{}",
        machine.log()
    );
    assert!(!machine.path("stage/ready").exists());
}
