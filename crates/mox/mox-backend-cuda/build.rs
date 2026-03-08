use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=src/kernels/quantized_matvec.cu");
    println!("cargo:rerun-if-changed=src/kernels/quantized_matvec_stub.c");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    if let Some(nvcc) = find_nvcc() {
        compile_cuda_kernels(&nvcc, &out_dir);
    } else {
        cc::Build::new()
            .file("src/kernels/quantized_matvec_stub.c")
            .compile("mox_cuda_quantized_kernels");
    }
}

fn find_nvcc() -> Option<PathBuf> {
    if let Ok(path) = env::var("NVCC") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let candidates = [
        PathBuf::from("/opt/cuda/bin/nvcc"),
        PathBuf::from("/usr/local/cuda/bin/nvcc"),
        PathBuf::from("nvcc"),
    ];
    candidates
        .into_iter()
        .find(|candidate| Command::new(candidate).arg("--version").output().is_ok())
}

fn compile_cuda_kernels(nvcc: &Path, out_dir: &Path) {
    let object = out_dir.join("quantized_matvec.o");
    let status = Command::new(nvcc)
        .args([
            "-std=c++17",
            "-Xcompiler",
            "-fPIC",
            "-c",
            "src/kernels/quantized_matvec.cu",
            "-o",
        ])
        .arg(&object)
        .status()
        .expect("failed to spawn nvcc");
    assert!(
        status.success(),
        "nvcc failed to compile quantized CUDA kernels"
    );

    cc::Build::new()
        .cpp(true)
        .object(&object)
        .compile("mox_cuda_quantized_kernels");

    println!("cargo:rustc-link-lib=cudart");
    println!("cargo:rustc-link-search=native=/opt/cuda/lib64");
    println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
}
