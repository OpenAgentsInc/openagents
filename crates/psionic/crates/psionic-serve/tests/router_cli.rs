use psionic_inference::EngineKind;
use psionic_serve::small_models::RouterServerOptions;

fn parse(args: &[&str]) -> Result<Option<RouterServerOptions>, String> {
    RouterServerOptions::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>())
}

#[test]
fn preserves_legacy_arguments_and_requires_explicit_router_engine() {
    assert!(
        parse(&["-m", "model.gguf", "--backend", "cuda"])
            .unwrap()
            .is_none()
    );
    assert!(parse(&["--router-model-dir", "models"]).is_err());
    let options = parse(&[
        "--router-model-dir",
        "models",
        "--router-engine",
        "native",
        "--port",
        "0",
    ])
    .unwrap()
    .unwrap();
    assert_eq!(options.engine, EngineKind::Native);
    assert_eq!(options.address.port(), 0);
}

#[test]
fn refuses_missing_ort_library_and_ambiguous_mixed_model_options() {
    assert!(parse(&["--router-model-dir", "models", "--router-engine", "ort"]).is_err());
    assert!(
        parse(&[
            "--router-model-dir",
            "models",
            "--router-engine",
            "native",
            "-m",
            "model.gguf"
        ])
        .is_err()
    );
    assert!(
        parse(&[
            "--router-model-dir",
            "models",
            "--router-engine",
            "native",
            "--router-threads",
            "0"
        ])
        .is_err()
    );
}
