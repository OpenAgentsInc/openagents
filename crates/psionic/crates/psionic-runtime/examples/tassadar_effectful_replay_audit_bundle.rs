use psionic_runtime::{
    tassadar_effectful_replay_audit_bundle_path, write_tassadar_effectful_replay_audit_bundle,
};

fn main() {
    let path = tassadar_effectful_replay_audit_bundle_path();
    let bundle = write_tassadar_effectful_replay_audit_bundle(&path)
        .expect("effectful replay audit bundle should write");
    println!(
        "wrote effectful replay audit bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
