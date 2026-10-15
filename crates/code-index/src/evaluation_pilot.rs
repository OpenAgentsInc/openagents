//! Reproducible small coding cases. These measure fixture tasks, not repo-wide gains.

use crate::{builder::*, demand::Partition, evaluation::*, recipes::*};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn prepare_pilot(scratch: &Path, strong: &str, cheap: &str) -> Result<Plan, String> {
    let mut cases = Vec::new();
    let mut build_cost = Some(0.0);
    for id in ["registry", "new-module"] {
        let root = scratch.join("cases").join(id);
        if root.exists() {
            return Err(
                "Use a new pilot directory so earlier trials cannot affect the plan.".into(),
            );
        }
        crate::private::write(
            &root,
            "Cargo.toml",
            b"[package]\nname = \"pilot-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )?;
        std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
        let files = if id == "registry" {
            vec![
                ("src/lib.rs", "pub mod kind;\npub mod registry;\n"),
                ("src/kind.rs", "pub enum Builtin { Box }\n"),
                (
                    "src/registry.rs",
                    "use crate::kind::Builtin;\npub fn model_label(kind: &Builtin) -> &'static str { match kind { Builtin::Box => \"box\" } }\n",
                ),
            ]
        } else {
            vec![
                ("src/lib.rs", "pub mod registry;\n"),
                (
                    "src/registry.rs",
                    "pub fn known_shapes() -> Vec<&'static str> { vec![\"box\"] }\n",
                ),
            ]
        };
        for (path, text) in &files {
            std::fs::write(root.join(path), text).map_err(|e| e.to_string())?;
        }
        super::recipes::git(&root, &["init", "-q"])?;
        for (key, value) in [
            ("user.name", "Code index evaluation"),
            ("user.email", "code-index@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            super::recipes::git(&root, &["config", key, value])?;
        }
        super::recipes::git(&root, &["add", "."])?;
        super::recipes::git(&root, &["commit", "-qm", "Frozen coding fixture"])?;
        let context = Context::open(&root, "HEAD", "evaluation")?;
        let anchors = if id == "registry" {
            vec![
                SourceRequest {
                    path: "src/kind.rs".into(),
                    symbol: Some("pilot_fixture::kind::Builtin".into()),
                    start: 0,
                    end: 0,
                },
                SourceRequest {
                    path: "src/registry.rs".into(),
                    symbol: Some("pilot_fixture::registry::model_label".into()),
                    start: 0,
                    end: 0,
                },
            ]
        } else {
            vec![
                SourceRequest {
                    path: "src/lib.rs".into(),
                    symbol: None,
                    start: 1,
                    end: 1,
                },
                SourceRequest {
                    path: "src/registry.rs".into(),
                    symbol: Some("pilot_fixture::registry::known_shapes".into()),
                    start: 0,
                    end: 0,
                },
            ]
        };
        let references = anchors
            .iter()
            .map(|a| context.reference(&a.path, a.symbol.as_deref(), a.start, a.end))
            .collect::<Result<Vec<_>, _>>()?;
        let reviewed=context.verify(&Artifact{schema:"openagents.task-index.v1".into(),snapshot:context.snapshot.clone(),recipes:vec![Recipe{
            id:id.into(),capability:"3d-model-registry".into(),operation:if id=="registry"{"register-model-kind"}else{"add-model-module"}.into(),
            description:if id=="registry"{"Add a built-in model variant and update its exhaustive label registration."}else{"Add a public model module, export it, and include it in the known-shape registry."}.into(),
            aliases:vec!["add a model".into(),"register a new model".into()],exclusions:vec!["inference model".into()],conditions:vec![],
            roles:references.iter().enumerate().map(|(i,r)|Role{id:format!("role-{i}"),purpose:if i==0{"Change the declaration or module export."}else{"Update the companion registry and preserve existing entries."}.into(),required:true,when:None,references:vec![r.clone()]}).collect(),
            examples:vec![references[1].clone()],new_files:if id=="registry"{vec![]}else{vec![NewFile{directory:"src".into(),purpose:"Add the requested public model module and export it from lib.rs.".into(),integration:references[0].clone()}]},
            checks:vec![Check{package:"pilot-fixture".into(),manifest:"Cargo.toml".into(),filter:None,establishes:"The independent acceptance checks exercise the new case and the existing one.".into()}],evidence:EvidenceClass::SourceReviewed,limitations:vec!["A small fixture pattern; no production quality claim.".into()],dependency_scopes:vec!["src".into()]
        }]})?;
        let mut teacher_anchors = anchors;
        teacher_anchors.push(SourceRequest {
            path: "Cargo.toml".into(),
            symbol: None,
            start: 1,
            end: 4,
        });
        let task = if id == "registry" {
            "Build a reusable recipe for adding a new Builtin model variant and its label registration. Preserve existing variants. Identify all required source roles and the Cargo package to check."
        } else {
            "Build a reusable recipe for adding a public model implementation module, exporting it, and registering its name. Preserve existing shapes. Identify required companion roles, new-file placement, and the Cargo package to check."
        };
        let request = crate::builder::prepare(
            &context,
            task,
            strong,
            &teacher_anchors,
            Budget {
                calls: 2,
                ..Default::default()
            },
        )?;
        let output = scratch.join("builds").join(id);
        let mut teacher = ClaudeTeacher {
            root: root.clone(),
            output: output.clone(),
            sequence: 0,
        };
        let built = crate::builder::build(&context, request, &mut teacher)?;
        crate::private::write(
            &output,
            "build.json",
            &serde_json::to_vec(&built).map_err(|e| e.to_string())?,
        )?;
        build_cost = build_cost.zip(built.cost_usd).map(|(a, b)| a + b);
        let (task, acceptance, companions, new_paths) = if id == "registry" {
            (
                "Add a Capsule variant to Builtin in src/kind.rs. Update model_label in src/registry.rs so Capsule maps to capsule and Box still maps to box.",
                "use pilot_fixture::{kind::Builtin,registry::model_label}; #[test] fn both_models(){assert_eq!(model_label(&Builtin::Capsule),\"capsule\");assert_eq!(model_label(&Builtin::Box),\"box\");}",
                vec!["src/kind.rs".into(), "src/registry.rs".into()],
                vec![],
            )
        } else {
            (
                "Add src/capsule.rs with a public radius() -> f32 function that returns 0.5. Export it as a public capsule module from src/lib.rs. Include capsule in known_shapes in src/registry.rs while preserving box.",
                "use pilot_fixture::{capsule,registry::known_shapes}; #[test] fn public_and_registered(){assert_eq!(capsule::radius(),0.5);assert!(known_shapes().contains(&\"capsule\"));assert!(known_shapes().contains(&\"box\"));}",
                vec!["src/lib.rs".into(), "src/registry.rs".into()],
                vec!["src/capsule.rs".into()],
            )
        };
        cases.push(Case {
            id: id.into(),
            family: id.into(),
            partition: Partition::Evaluation,
            available_ms: 0,
            repository: root,
            base: context.snapshot.commit,
            task: task.into(),
            index: built.artifact,
            reviewed_index: Some(reviewed),
            acceptance: BTreeMap::from([("tests/acceptance.rs".into(), acceptance.into())]),
            companion_paths: companions,
            new_paths,
        });
    }
    let cutoff = atif::now_ms();
    for case in &mut cases {
        case.available_ms = cutoff;
    }
    let plan=Plan{purpose:Purpose::Evaluation,selectors:BTreeMap::new(),selector_minimum_gain:None,selector_dataset_digest:None,schema:"openagents.task-index-eval-plan.v1".into(),frozen_ms:cutoff,index_cutoff_ms:cutoff,training_families:BTreeSet::new(),cases,
        arms:vec![Arm{id:"strong-current".into(),model:strong.into(),context:ContextKind::Current},Arm{id:"cheap-current".into(),model:cheap.into(),context:ContextKind::Current},Arm{id:"cheap-summaries".into(),model:cheap.into(),context:ContextKind::Summary},Arm{id:"cheap-capability".into(),model:cheap.into(),context:ContextKind::Capability},Arm{id:"cheap-recipes".into(),model:cheap.into(),context:ContextKind::Recipes},Arm{id:"cheap-reviewed".into(),model:cheap.into(),context:ContextKind::Reviewed}],
        repetitions:1,timeout_secs:90,max_output_bytes:2*1024*1024,build_cost_usd:build_cost,refresh_cost_usd:None,check_cost_usd:None,
        baseline:"cheap-current".into(),candidate:"cheap-recipes".into(),min_families:4,allowed_quality_loss:0.0,max_cost_ratio:0.9,
        scope:"Two small Rust fixture families using restricted Claude file tools. Current arms receive a file list, not the full production Coder toolset. Reviewed guides were reviewed by the implementing agent, not a human. Model aliases and provider-reported cost are recorded; machine/check costs are unknown. This pilot alone cannot promote the production feature.".into()};
    validate(&plan)?;
    crate::private::write(
        scratch,
        "plan.json",
        &serde_json::to_vec(&plan).map_err(|e| e.to_string())?,
    )?;
    Ok(plan)
}
