use psionic_inference::{
    DeviceChoice, ModelEngine, Tensor, TensorData, Tensors,
    ort_engine::{OrtEngine, OrtExecution, OrtOptions},
};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let options = OrtOptions {
        library: args[1].clone().into(),
        library_sha256: psionic_inference::artifacts::file_sha256(std::path::Path::new(&args[1]))
            .unwrap(),
        threads: 4,
    };
    let profile = std::env::var_os("PSIONIC_PROBE_PROFILE").map(std::path::PathBuf::from);
    let mut engine = OrtEngine::load_with_execution(
        std::path::Path::new(&args[2]),
        &options,
        &OrtExecution {
            device: DeviceChoice::Cuda,
            allow_cpu_fallback: true,
            profile,
            ..Default::default()
        },
    )
    .unwrap();
    let input = Tensors::from([(
        "Input3".into(),
        Tensor {
            shape: vec![1, 1, 28, 28],
            data: TensorData::F32(vec![0.0; 784]),
        },
    )]);
    for i in 0..1000 {
        engine.run(&input).unwrap();
        if i == 99 {
            engine.finish_profiling().unwrap();
        }
    }
    drop(engine);
    println!("All runs and session destruction completed");
}
