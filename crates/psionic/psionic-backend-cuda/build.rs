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
            .compile("psionic_cuda_quantized_kernels");
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
    let mut command = Command::new(nvcc);
    command.args([
        "-std=c++17",
        "-O3",
        "--use_fast_math",
        "-Xcompiler",
        "-fPIC",
        "-c",
        "src/kernels/quantized_matvec.cu",
    ]);
    if let Some(arch) = find_cuda_arch() {
        command.arg(format!("-arch={arch}"));
    }
    let status = command
        .arg("-o")
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
        .compile("psionic_cuda_quantized_kernels");

    println!("cargo:rustc-link-lib=cudart");
    println!("cargo:rustc-link-search=native=/opt/cuda/lib64");
    println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
}

fn find_cuda_arch() -> Option<String> {
    env::var("CUDAARCHS")
        .ok()
        .and_then(|value| normalize_cuda_arch(value.as_str()))
        .or_else(|| {
            env::var("PSI_CUDA_ARCH")
                .ok()
                .and_then(|value| normalize_cuda_arch(value.as_str()))
        })
        .or_else(|| {
            Command::new("nvidia-smi")
                .args(["--query-gpu=compute_cap", "--format=csv,noheader,nounits"])
                .output()
                .ok()
                .and_then(|output| {
                    if !output.status.success() {
                        return None;
                    }
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    stdout
                        .lines()
                        .next()
                        .map(str::trim)
                        .and_then(normalize_cuda_arch)
                })
        })
}

fn normalize_cuda_arch(raw: &str) -> Option<String> {
    let digits = raw.chars().filter(char::is_ascii_digit).collect::<String>();
    if digits.len() < 2 {
        return None;
    }
    Some(format!("sm_{digits}"))
}
