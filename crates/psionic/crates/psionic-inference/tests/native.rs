use std::path::PathBuf;

use psionic_inference::{
    EngineKind, ModelEngine, Tensors,
    modernbert::ModernBertEngine,
    router::{answer, pinned_manifest, verify_native_router},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    requests: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    inputs: Tensors,
    outputs: Tensors,
}

#[test]
fn invalid_native_configuration_is_an_error() {
    assert!(ModernBertEngine::load(std::path::Path::new("/missing-model"), 0).is_err());
    assert!(ModernBertEngine::load(std::path::Path::new("/missing-model"), 4).is_err());
}

#[test]
#[ignore = "requires converted pinned model; set PSIONIC_ROUTER_MODEL_DIR"]
fn native_matches_all_ort_reference_cases() {
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("model directory"));
    let manifest = pinned_manifest().unwrap();
    manifest.verify(&root).unwrap();
    verify_native_router(&root).unwrap();
    let mut engine = ModernBertEngine::load(&root, 4).unwrap();
    assert_eq!(engine.identity().engine, EngineKind::Native);
    assert!(engine.run(&Tensors::new()).is_err());
    let fixture: Fixture =
        serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap();
    let mut max_distance = 0.0_f32;
    for case in fixture.requests {
        let actual = engine.run(&case.inputs).unwrap();
        let a = answer(&actual, &manifest).unwrap();
        let b = answer(&case.outputs, &manifest).unwrap();
        assert_eq!(
            [a.category.label, a.difficulty.label, a.stakes.label],
            [b.category.label, b.difficulty.label, b.stakes.label],
            "case {}",
            case.id
        );
        for name in a.flags.keys() {
            assert_eq!(
                a.flags[name] >= 0.5,
                b.flags[name] >= 0.5,
                "case {} flag {name}",
                case.id
            );
        }
        for (name, expected) in &case.outputs {
            let x = actual[name].floats().unwrap();
            let y = expected.floats().unwrap();
            let mx = x.iter().sum::<f32>() / x.len() as f32;
            let my = y.iter().sum::<f32>() / y.len() as f32;
            for (x, y) in x.iter().zip(y) {
                let distance = ((x - mx) - (y - my)).abs() / manifest.temperatures[name];
                max_distance = max_distance.max(distance);
                assert!(
                    distance < 0.01,
                    "case {} head {name}: centered distance {distance}, raw {x} vs {y}",
                    case.id
                );
            }
        }
    }
    eprintln!("Maximum centered logit distance: {max_distance}");
}
