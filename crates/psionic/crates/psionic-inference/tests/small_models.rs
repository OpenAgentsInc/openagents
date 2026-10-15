use psionic_inference::{
    EngineKind, Tensors,
    models::{self, ModelFamily},
};
use serde::Deserialize;
#[cfg(feature = "ort")]
use std::path::PathBuf;

#[derive(Deserialize)]
struct Reference {
    model: String,
    revision: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    inputs: Tensors,
    outputs: Tensors,
    #[allow(dead_code)]
    #[serde(default)]
    texts: Vec<String>,
}

fn reference(family: ModelFamily) -> Reference {
    serde_json::from_str(match family {
        ModelFamily::Potion => include_str!("../fixtures/potion-reference.json"),
        ModelFamily::Mnist => include_str!("../fixtures/mnist-reference.json"),
    })
    .unwrap()
}
#[cfg(feature = "ort")]
fn argmax(v: &[f32]) -> usize {
    let mut i = 0;
    for j in 1..v.len() {
        if v[j] > v[i] {
            i = j;
        }
    }
    i
}

#[test]
fn fixtures_match_the_pinned_sources() {
    for family in [ModelFamily::Potion, ModelFamily::Mnist] {
        let spec = models::manifest(family).unwrap();
        let reference = reference(family);
        assert_eq!(spec.model, reference.model);
        assert_eq!(spec.revision, reference.revision);
        assert_eq!(reference.cases.len(), 100);
        for case in reference.cases {
            for tensor in case.inputs.values().chain(case.outputs.values()) {
                tensor.validate().unwrap();
            }
        }
        assert!(
            models::load_engine(
                family,
                EngineKind::Native,
                std::path::Path::new("/missing-model"),
                1,
                None,
                None
            )
            .is_err()
        );
    }
}

#[test]
#[cfg(feature = "ort")]
#[ignore = "requires pinned small models and ORT; set PSIONIC_SMALL_MODELS_DIR and PSIONIC_ORT_LIBRARY"]
fn both_engines_match_independent_embedding_and_vision_references() {
    use psionic_inference::models::{MnistModel, PotionModel};
    let root =
        PathBuf::from(std::env::var_os("PSIONIC_SMALL_MODELS_DIR").expect("models directory"));
    let library = PathBuf::from(std::env::var_os("PSIONIC_ORT_LIBRARY").expect("ORT library"));
    let hash = psionic_inference::artifacts::file_sha256(&library).unwrap();
    for family in [ModelFamily::Potion, ModelFamily::Mnist] {
        let dir = root.join(match family {
            ModelFamily::Potion => "potion",
            ModelFamily::Mnist => "mnist",
        });
        let reference = reference(family);
        for kind in [EngineKind::Native, EngineKind::Ort] {
            let mut engine =
                models::load_engine(family, kind, &dir, 4, Some(&library), Some(&hash)).unwrap();
            let mut distance = 0_f32;
            for (i, case) in reference.cases.iter().enumerate() {
                let output = engine.run(&case.inputs).unwrap();
                assert_eq!(output.len(), case.outputs.len());
                for (name, expected) in &case.outputs {
                    assert_eq!(output[name].shape, expected.shape);
                    let a = output[name].floats().unwrap();
                    let b = expected.floats().unwrap();
                    for (a, b) in a.iter().zip(b) {
                        distance = distance.max((a - b).abs());
                    }
                    match family {
                        ModelFamily::Potion => {
                            for (a, b) in a.chunks(64).zip(b.chunks(64)) {
                                if b.iter().all(|v| *v == 0.0) {
                                    assert!(a.iter().all(|v| *v == 0.0));
                                } else {
                                    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
                                    assert!(dot > 0.99999, "case {i}: cosine {dot}");
                                }
                            }
                        }
                        ModelFamily::Mnist => assert_eq!(argmax(a), argmax(b), "case {i}"),
                    }
                }
            }
            let tolerance = match family {
                ModelFamily::Potion => 1e-5,
                ModelFamily::Mnist => 1e-3,
            };
            eprintln!("{family:?}/{kind:?}: max absolute distance {distance}");
            assert!(distance < tolerance);
            if let ModelFamily::Potion = family {
                let mut model =
                    PotionModel::load(&dir, kind, 4, Some(&library), Some(&hash)).unwrap();
                for case in &reference.cases {
                    let texts: Vec<&str> = case.texts.iter().map(String::as_str).collect();
                    assert_eq!(model.prepare(&texts).unwrap(), case.inputs);
                }
                assert!(model.encode(&[]).unwrap().is_empty());
                let candidates = [
                    "A simple pasta recipe.",
                    "Rust memory safety and borrowing",
                    "A summer trip to Paris.",
                ];
                let rank = model
                    .rank("Rust memory safety and borrowing", &candidates)
                    .unwrap();
                assert_eq!(rank[0].index, 1);
                assert!(
                    model
                        .prepare(&["x".repeat(128 * 1024 + 1).as_str()])
                        .is_err()
                );
            } else {
                let mut model =
                    MnistModel::load(&dir, kind, 4, Some(&library), Some(&hash)).unwrap();
                let pixel_values = reference.cases[2].inputs["Input3"].floats().unwrap();
                let pixels: Vec<u8> = pixel_values
                    .iter()
                    .map(|v| (v * 255.0).round() as u8)
                    .collect();
                let prediction = model.classify(&pixels).unwrap();
                assert_eq!(
                    prediction.digit,
                    argmax(
                        reference.cases[2].outputs["Plus214_Output_0"]
                            .floats()
                            .unwrap()
                    )
                );
                assert!((prediction.probabilities.iter().sum::<f32>() - 1.0).abs() < 1e-6);
            }
        }
    }
}
