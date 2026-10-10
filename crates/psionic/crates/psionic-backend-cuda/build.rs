use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=src/kernels/quantized_matvec.cu");
    println!("cargo:rerun-if-changed=src/kernels/qwen38_quant_tables.cuh");
    println!("cargo:rerun-if-changed=src/kernels/quantized_matvec_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/clef_prefill.cu");
    println!("cargo:rerun-if-changed=src/kernels/clef_prefill_stub.c");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    if let Some(nvcc) = find_nvcc() {
        compile_cuda_kernels(&nvcc, &out_dir)?;
    } else {
        cc::Build::new()
            .file("src/kernels/quantized_matvec_stub.c")
            .file("src/kernels/clef_prefill_stub.c")
            .compile("psionic_cuda_quantized_kernels");
    }
    Ok(())
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

fn compile_cuda_object(
    nvcc: &Path,
    source: &str,
    object: &Path,
    fast_math: bool,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(nvcc);
    command.args(["-std=c++17", "-O3"]);
    if fast_math {
        command.arg("--use_fast_math");
    }
    command.args(["-Xcompiler", "-fPIC", "-c", source]);
    if let Some(arch) = find_cuda_arch() {
        command.arg(format!("-arch={arch}"));
    }
    let status = command.arg("-o").arg(object).status()?;
    if !status.success() {
        return Err(format!("nvcc failed to compile {source}").into());
    }
    Ok(())
}

fn compile_cuda_kernels(nvcc: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let object = out_dir.join("quantized_matvec.o");
    compile_cuda_object(nvcc, "src/kernels/quantized_matvec.cu", &object, true)?;
    // The Clef prefill kernels keep IEEE expf/division: decisions compare
    // against f32 references.
    let clef_object = out_dir.join("clef_prefill.o");
    compile_cuda_object(nvcc, "src/kernels/clef_prefill.cu", &clef_object, false)?;

    cc::Build::new()
        .cpp(true)
        .object(&object)
        .object(&clef_object)
        .compile("psionic_cuda_quantized_kernels");

    println!("cargo:rustc-link-lib=cudart");
    println!("cargo:rustc-link-search=native=/opt/cuda/lib64");
    println!("cargo:rustc-link-search=native=/usr/local/cuda/lib64");
    Ok(())
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
