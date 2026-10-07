use psionic_runtime::{
    tassadar_preemptive_job_runtime_bundle_path, write_tassadar_preemptive_job_runtime_bundle,
};

fn main() {
    let path = tassadar_preemptive_job_runtime_bundle_path();
    let bundle = write_tassadar_preemptive_job_runtime_bundle(&path)
        .expect("preemptive-job runtime bundle should write");
    println!(
        "wrote preemptive-job runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
