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
    println!("cargo:rerun-if-changed=src/kernels/clef_hopper.cu");
    println!("cargo:rerun-if-changed=src/kernels/clef_hopper_stub.c");

    println!("cargo:rerun-if-changed=src/kernels/encoder.cu");
    println!("cargo:rerun-if-changed=src/kernels/encoder_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/digit.cu");
    println!("cargo:rerun-if-changed=src/kernels/digit_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/v41_block_matvec.cu");
    println!("cargo:rerun-if-changed=src/kernels/v41_block_matvec_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/v41_mix.cu");
    println!("cargo:rerun-if-changed=src/kernels/v41_mix_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/v41_attention.cu");
    println!("cargo:rerun-if-changed=src/kernels/v41_attention_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/v41_stage.cu");
    println!("cargo:rerun-if-changed=src/kernels/v41_stage_stub.c");
    println!("cargo:rerun-if-changed=src/kernels/v41_expert_cache.cu");
    println!("cargo:rerun-if-changed=src/kernels/v41_expert_cache_stub.c");
    println!("cargo:rerun-if-env-changed=NVCC");
    println!("cargo:rerun-if-env-changed=PSIONIC_CUDA");
    // The target architecture is read at run time below, so Cargo only
    // recompiles the kernels for a new one when told to watch it (#12407).
    println!("cargo:rerun-if-env-changed=CUDAARCHS");
    println!("cargo:rerun-if-env-changed=PSI_CUDA_ARCH");
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    // `PSIONIC_CUDA=off` links the stubs even where nvcc is installed: the
    // CPU-only engine build (`scripts/release/psionic.sh`) must not need
    // the CUDA runtime.
    let cuda = !matches!(env::var("PSIONIC_CUDA").as_deref(), Ok("off"));
    if let Some(nvcc) = find_nvcc().filter(|_| cuda) {
        compile_cuda_kernels(&nvcc, &out_dir)?;
    } else {
        cc::Build::new()
            .file("src/kernels/quantized_matvec_stub.c")
            .file("src/kernels/clef_prefill_stub.c")
            .file("src/kernels/encoder_stub.c")
            .file("src/kernels/digit_stub.c")
            .file("src/kernels/v41_block_matvec_stub.c")
            .file("src/kernels/v41_mix_stub.c")
            .file("src/kernels/v41_attention_stub.c")
            .file("src/kernels/v41_stage_stub.c")
            .file("src/kernels/v41_expert_cache_stub.c")
            .file("src/kernels/clef_hopper_stub.c")
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
    compile_cuda_object_for(nvcc, source, object, fast_math, find_cuda_arch())
}

fn compile_cuda_object_for(
    nvcc: &Path,
    source: &str,
    object: &Path,
    fast_math: bool,
    arch: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new(nvcc);
    command.args(["-std=c++17", "-O3"]);
    if fast_math {
        command.arg("--use_fast_math");
    }
    command.args(["-Xcompiler", "-fPIC", "-c", source]);
    if let Some(arch) = arch {
        match arch.strip_prefix("sm_") {
            // Architecture-specific features (`sm_90a`: wgmma, setmaxnreg)
            // need the matching `compute_90a` virtual architecture and run
            // only on that device.
            Some(digits) if digits.ends_with('a') => {
                command.arg(format!("-gencode=arch=compute_{digits},code={arch}"));
            }
            _ => {
                command.arg(format!("-arch={arch}"));
            }
        }
    }
    let status = command.arg("-o").arg(object).status()?;
    if !status.success() {
        return Err(format!("nvcc failed to compile {source}").into());
    }
    Ok(())
}

fn compile_cuda_kernels(nvcc: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    // The crate reports the architecture it was compiled for
    // (`COMPILED_CUDA_ARCH`), so a binary can say what it holds.
    match find_cuda_arch() {
        Some(arch) => println!("cargo:rustc-env=PSIONIC_CUDA_COMPILED_ARCH={arch}"),
        None => println!("cargo:rustc-env=PSIONIC_CUDA_COMPILED_ARCH=nvcc-default"),
    }
    if env::var("CUDAARCHS").is_err() && env::var("PSI_CUDA_ARCH").is_err() {
        println!(
            "cargo:warning=psionic-backend-cuda: no CUDAARCHS or PSI_CUDA_ARCH; the kernels target {}, and changing the GPU alone won't rebuild them",
            find_cuda_arch().unwrap_or_else(|| "nvcc's default".into())
        );
    }
    let object = out_dir.join("quantized_matvec.o");
    compile_cuda_object(nvcc, "src/kernels/quantized_matvec.cu", &object, true)?;
    // The Clef prefill kernels keep IEEE expf/division: decisions compare
    // against f32 references.
    let clef_object = out_dir.join("clef_prefill.o");
    compile_cuda_object(nvcc, "src/kernels/clef_prefill.cu", &clef_object, false)?;

    let encoder_object = out_dir.join("encoder.o");
    compile_cuda_object(nvcc, "src/kernels/encoder.cu", &encoder_object, false)?;
    let digit_object = out_dir.join("digit.o");
    compile_cuda_object(nvcc, "src/kernels/digit.cu", &digit_object, false)?;
    // V4.1 decoding relies on float32 subnormals: no fast math.
    let v41_object = out_dir.join("v41_block_matvec.o");
    compile_cuda_object(nvcc, "src/kernels/v41_block_matvec.cu", &v41_object, false)?;
    // The mixing, Engram, and routing kernels round with explicit intrinsics
    // and keep IEEE division and square roots: no fast math either.
    let v41_mix_object = out_dir.join("v41_mix.o");
    compile_cuda_object(nvcc, "src/kernels/v41_mix.cu", &v41_mix_object, false)?;
    // The attention kernels follow the same rules.
    let v41_attention_object = out_dir.join("v41_attention.o");
    compile_cuda_object(
        nvcc,
        "src/kernels/v41_attention.cu",
        &v41_attention_object,
        false,
    )?;
    // The stage kernels around them too.
    let v41_stage_object = out_dir.join("v41_stage.o");
    compile_cuda_object(nvcc, "src/kernels/v41_stage.cu", &v41_stage_object, false)?;
    // The expert cache's host-lane mailbox kernels (#12463).
    let v41_cache_object = out_dir.join("v41_expert_cache.o");
    compile_cuda_object(
        nvcc,
        "src/kernels/v41_expert_cache.cu",
        &v41_cache_object,
        false,
    )?;
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .object(&object)
        .object(&clef_object)
        .object(&encoder_object)
        .object(&digit_object)
        .object(&v41_object)
        .object(&v41_mix_object)
        .object(&v41_attention_object)
        .object(&v41_stage_object)
        .object(&v41_cache_object);
    // The Hopper projection GEMM (#11443) is wgmma/TMA code for sm_90a;
    // other targets link its stub and keep the fused and cuBLAS paths.
    if find_cuda_arch().is_some_and(|arch| arch == "sm_90" || arch == "sm_90a") {
        let hopper_object = out_dir.join("clef_hopper.o");
        compile_cuda_object_for(
            nvcc,
            "src/kernels/clef_hopper.cu",
            &hopper_object,
            false,
            Some(String::from("sm_90a")),
        )?;
        build.object(&hopper_object);
    } else {
        cc::Build::new()
            .file("src/kernels/clef_hopper_stub.c")
            .compile("psionic_cuda_hopper_stub");
    }
    build.compile("psionic_cuda_quantized_kernels");

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
    let suffix = if raw.trim().ends_with('a') { "a" } else { "" };
    Some(format!("sm_{digits}{suffix}"))
}
