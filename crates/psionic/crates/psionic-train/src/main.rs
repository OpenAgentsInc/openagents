use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::Digest;

use psionic_train::{
    PSION_APPLE_WINDOWED_TRAINING_LANE_ID, PSION_CS336_A1_DEMO_LANE_ID,
    PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION,
    PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION,
    PSIONIC_TRAIN_PACKAGED_RUNTIME_DIRTY_TREE_ADMISSION,
    PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE, PSIONIC_TRAIN_PACKAGED_RUNTIME_SELECTED_REF,
    PSIONIC_TRAIN_RUNTIME_ROOT_ENV, PsionicTrainArtifactSurfaceRefs,
    PsionicTrainCapabilityProjection, PsionicTrainCheckpointHandoffError,
    PsionicTrainCheckpointManifest, PsionicTrainCheckpointPointer, PsionicTrainCheckpointSurface,
    PsionicTrainGroupedReplicaCheckpointError, PsionicTrainGroupedReplicaRecoverySourceKind,
    PsionicTrainGroupedReplicaTransportError, PsionicTrainInvocationManifest,
    PsionicTrainMembershipRevisionReceipt, PsionicTrainOperation, PsionicTrainOutcomeKind,
    PsionicTrainRefusalClass, PsionicTrainRunStatusPacket, PsionicTrainRuntimeAttestation,
    PsionicTrainRuntimeContractError, PsionicTrainStatusPacket,
    PsionicTrainValidatorArtifactOutputs, PsionicTrainValidatorReplayError,
    PsionicTrainWindowArtifactInputRefs, PsionicTrainWindowArtifactOutputs,
    PsionicTrainWindowStatusPacket, admitted_environment_ref_for_lane,
    admitted_release_id_for_lane, build_psionic_train_checkpoint_handoff_receipt,
    execute_psionic_train_validator_replay, inspect_psionic_train_checkpoint_surface,
    materialize_psionic_train_checkpoint_handoff,
    persist_psionic_train_grouped_stage_recovery_receipt_from_surface,
    persist_psionic_train_window_artifacts, retain_psionic_train_checkpoint_handoff_receipt,
    run_psion_cs336_a1_demo_cli, run_psion_cs336_a1_demo_manifest, run_psionic_legal_dpo_cli,
    run_psionic_legal_ft_cli, run_psionic_legal_grpo_cli, run_psionic_legal_merge_lora_cli,
    run_psionic_legal_sft_cli, run_qwen_legal_full_artifact_promotion_cli,
    run_qwen_legal_hillclimb_cli, run_qwen_legal_rl_rollout_cli, runtime_build_digest,
    validate_psionic_train_grouped_stage_input_transport,
};
use psionic_train::{
    PSIONIC_TRAIN_CPU_BUDGET_ENV, psionic_train_cpu_budget_banner, resolve_psionic_train_cpu_budget,
};

#[allow(dead_code)]
#[path = "../examples/psion_actual_pretraining_operator.rs"]
mod psion_actual_pretraining_operator;

#[derive(Clone, Debug)]
struct MachineRuntimeIdentity {
    attestation: PsionicTrainRuntimeAttestation,
    capability_projection: PsionicTrainCapabilityProjection,
}

#[derive(Clone, Debug)]
struct MachineRuntimeSource {
    repo_root: PathBuf,
    packaged_revision: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct MachineStatusSurfacePaths {
    run_status_packet_path: Option<String>,
    window_status_packet_path: Option<String>,
}

#[derive(Clone, Debug)]
struct RetainedCheckpointSurface {
    path: String,
    surface: PsionicTrainCheckpointSurface,
}

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h" | "help") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    // Machine `manifest` mode reserves stdout and stderr for single JSON
    // status packets, so the budget is enforced silently there.
    let print_cpu_budget_banner = args[0].as_str() != "manifest";
    if let Err(exit_code) = apply_bounded_cpu_budget(print_cpu_budget_banner) {
        return exit_code;
    }

    match args[0].as_str() {
        "manifest" => run_manifest_mode(&args[1..]),
        "sft" => run_sft_passthrough(&args[1..]),
        "dpo" => run_dpo_passthrough(&args[1..]),
        "grpo" => run_grpo_passthrough(&args[1..]),
        "qwen-legal-rl-rollouts" => run_qwen_legal_rl_rollouts_passthrough(&args[1..]),
        "qwen-legal-artifact-promotion" => {
            run_qwen_legal_artifact_promotion_passthrough(&args[1..])
        }
        "qwen-legal-hillclimb" => run_qwen_legal_hillclimb_passthrough(&args[1..]),
        "merge-lora" => run_merge_lora_passthrough(&args[1..]),
        "legal" => run_legal_ft_passthrough(&args[1..]),
        "actual-pretraining" => run_actual_pretraining_passthrough(&args[1..]),
        "cs336-a1-demo" => run_cs336_a1_demo_passthrough(&args[1..]),
        other => {
            eprintln!(
                "error: unsupported psionic-train subcommand `{other}`\n\nsupported subcommands: manifest, sft, dpo, grpo, qwen-legal-rl-rollouts, qwen-legal-artifact-promotion, qwen-legal-hillclimb, merge-lora, legal, actual-pretraining, cs336-a1-demo"
            );
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

/// Applies the bounded CPU budget before any training work begins
/// (psionic#1123). The bounded single-core default is binding unless the
/// owner set `PSIONIC_TRAIN_CPU_BUDGET` as an explicit opt-in; an invalid
/// value is refused instead of silently widening or narrowing the budget.
fn apply_bounded_cpu_budget(print_banner: bool) -> Result<(), ExitCode> {
    let env_value = env::var(PSIONIC_TRAIN_CPU_BUDGET_ENV).ok();
    let resolution = resolve_psionic_train_cpu_budget(env_value.as_deref()).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
    })?;
    for (name, value) in resolution.budget.thread_cap_env_assignments() {
        // SAFETY: called at process start, before any threads are spawned,
        // so mutating the process environment cannot race other threads.
        unsafe {
            env::set_var(name, value);
        }
    }
    if print_banner {
        eprintln!("{}", psionic_train_cpu_budget_banner(&resolution));
    }
    Ok(())
}

fn run_legal_ft_passthrough(args: &[String]) -> ExitCode {
    match run_psionic_legal_ft_cli(args) {
        Ok(output) => match serde_json::to_string_pretty(&output.machine_receipt) {
            Ok(json) => {
                println!("{}", output.human_summary);
                println!("{json}");
                if output.integrity_valid {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
                }
            }
            Err(error) => {
                eprintln!("error: failed to serialize legal ft receipt: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_sft_passthrough(args: &[String]) -> ExitCode {
    match run_psionic_legal_sft_cli(args) {
        Ok(receipt) => match serde_json::to_string_pretty(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize SFT receipt: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_dpo_passthrough(args: &[String]) -> ExitCode {
    match run_psionic_legal_dpo_cli(args) {
        Ok(receipt) => match serde_json::to_string_pretty(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize DPO receipt: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_grpo_passthrough(args: &[String]) -> ExitCode {
    match run_psionic_legal_grpo_cli(args) {
        Ok(receipt) => match serde_json::to_string_pretty(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize GRPO receipt: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_qwen_legal_rl_rollouts_passthrough(args: &[String]) -> ExitCode {
    match run_qwen_legal_rl_rollout_cli(args) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize Qwen legal RL rollout batch report: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_qwen_legal_artifact_promotion_passthrough(args: &[String]) -> ExitCode {
    match run_qwen_legal_full_artifact_promotion_cli(args) {
        Ok(report) => match serde_json::to_string_pretty(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!(
                    "error: failed to serialize Qwen legal artifact promotion report: {error}"
                );
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_qwen_legal_hillclimb_passthrough(args: &[String]) -> ExitCode {
    match run_qwen_legal_hillclimb_cli(args) {
        Ok(output) => match serde_json::to_string_pretty(&output) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize Qwen legal hillclimb output: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_merge_lora_passthrough(args: &[String]) -> ExitCode {
    match run_psionic_legal_merge_lora_cli(args) {
        Ok(receipt) => match serde_json::to_string_pretty(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: failed to serialize merge-lora receipt: {error}");
                ExitCode::from(PsionicTrainRefusalClass::InternalError.exit_code())
            }
        },
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code())
        }
    }
}

fn run_manifest_mode(args: &[String]) -> ExitCode {
    let manifest_path = match parse_manifest_path(args) {
        Ok(path) => path,
        Err(error) => {
            emit_refusal_packet(PsionicTrainStatusPacket::refusal(
                None,
                PsionicTrainRefusalClass::BadConfig,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                error,
            ));
            return ExitCode::from(PsionicTrainRefusalClass::BadConfig.exit_code());
        }
    };

    let manifest = match load_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(packet) => {
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };

    let runtime_identity = match resolve_runtime_identity(&manifest, &manifest_path) {
        Ok(identity) => identity,
        Err(mut packet) => {
            let resolved_run_root = manifest_run_root(&manifest);
            let status_paths = write_status_surfaces_for_packet(
                &manifest,
                manifest_path.display().to_string(),
                resolved_run_root.as_deref(),
                None,
                &packet,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };

    let run_root = manifest_run_root(&manifest);
    if let Err(error) = validate_psionic_train_grouped_stage_input_transport(&manifest) {
        let summary = run_root.as_deref().and_then(machine_run_surface_summary);
        let mut packet = PsionicTrainStatusPacket::refusal(
            Some(&manifest),
            refusal_for_grouped_stage_transport_error(&error),
            Some(manifest_path.display().to_string()),
            Some(runtime_identity.attestation.clone()),
            Some(runtime_identity.capability_projection.clone()),
            summary
                .as_ref()
                .and_then(|value| value.run_id.clone())
                .or_else(|| manifest.run_id.clone()),
            run_root.as_ref().map(|value| value.display().to_string()),
            None,
            None,
            format!("failed to validate grouped stage input transport: {error}"),
        );
        let status_paths = write_status_surfaces_for_packet(
            &manifest,
            manifest_path.display().to_string(),
            run_root.as_deref(),
            summary.as_ref(),
            &packet,
        )
        .unwrap_or_default();
        packet.run_status_packet_path = status_paths.run_status_packet_path;
        packet.window_status_packet_path = status_paths.window_status_packet_path;
        emit_refusal_packet(packet.clone());
        return ExitCode::from(packet.exit_code);
    }
    if manifest.operation == PsionicTrainOperation::ServeCheckpoint {
        return run_checkpoint_handoff_manifest(
            &manifest,
            &manifest_path,
            &runtime_identity,
            run_root.as_deref(),
        );
    }
    if manifest.operation == PsionicTrainOperation::ValidateContribution {
        return run_validator_replay_manifest(
            &manifest,
            &manifest_path,
            &runtime_identity,
            run_root.as_deref(),
        );
    }
    if manifest.operation == PsionicTrainOperation::Resume {
        if let (Some(run_root), Some(peer_receipt_binding)) = (
            run_root.as_deref(),
            manifest.peer_checkpoint_handoff_receipt.as_ref(),
        ) {
            if let Err(error) = materialize_psionic_train_checkpoint_handoff(
                run_root,
                peer_receipt_binding,
                manifest
                    .coordination
                    .node_pubkey
                    .as_deref()
                    .expect("validated manifests always carry coordination.node_pubkey"),
            ) {
                return emit_checkpoint_handoff_refusal(
                    &manifest,
                    &manifest_path,
                    run_root,
                    &runtime_identity,
                    &error,
                    format!(
                        "failed to materialize peer checkpoint handoff `{}`",
                        peer_receipt_binding.artifact_ref.artifact_id
                    ),
                );
            }
        }
        if let Some(run_root) = run_root.as_deref() {
            if let Err(error) = persist_grouped_stage_resume_recovery(
                &manifest,
                run_root,
                if manifest.peer_checkpoint_handoff_receipt.is_some() {
                    PsionicTrainGroupedReplicaRecoverySourceKind::PeerCheckpointHandoff
                } else {
                    PsionicTrainGroupedReplicaRecoverySourceKind::RetainedCheckpoint
                },
            ) {
                let summary = machine_run_surface_summary(run_root);
                let mut packet = PsionicTrainStatusPacket::refusal(
                    Some(&manifest),
                    refusal_for_grouped_stage_checkpoint_error(&error),
                    Some(manifest_path.display().to_string()),
                    Some(runtime_identity.attestation.clone()),
                    Some(runtime_identity.capability_projection.clone()),
                    summary
                        .as_ref()
                        .and_then(|value| value.run_id.clone())
                        .or_else(|| manifest.run_id.clone()),
                    Some(run_root.display().to_string()),
                    None,
                    None,
                    format!("failed to materialize grouped stage resume recovery: {error}"),
                );
                let status_paths = write_status_surfaces_for_packet(
                    &manifest,
                    manifest_path.display().to_string(),
                    Some(run_root),
                    summary.as_ref(),
                    &packet,
                )
                .unwrap_or_default();
                packet.run_status_packet_path = status_paths.run_status_packet_path;
                packet.window_status_packet_path = status_paths.window_status_packet_path;
                emit_refusal_packet(packet.clone());
                return ExitCode::from(packet.exit_code);
            }
        }
    }
    if manifest.lane_id == PSION_APPLE_WINDOWED_TRAINING_LANE_ID {
        return run_apple_windowed_manifest(
            &manifest,
            &manifest_path,
            &runtime_identity,
            run_root.as_deref(),
        );
    }
    if manifest.lane_id == PSION_CS336_A1_DEMO_LANE_ID {
        return run_cs336_a1_demo_manifest_mode(
            &manifest,
            &manifest_path,
            &runtime_identity,
            run_root.as_deref(),
        );
    }

    let operator_args = match operator_args_from_manifest(&manifest) {
        Ok(args) => args,
        Err(mut packet) => {
            let status_paths = write_status_surfaces_for_packet(
                &manifest,
                manifest_path.display().to_string(),
                run_root.as_deref(),
                None,
                &packet,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };

    let manifest_path_text = Some(manifest_path.display().to_string());
    let initial_summary = run_root.as_deref().and_then(machine_run_surface_summary);

    psion_actual_pretraining_operator::set_human_output_enabled(false);
    let execution = psion_actual_pretraining_operator::run_with_args(operator_args);
    psion_actual_pretraining_operator::set_human_output_enabled(true);

    match execution {
        Ok(()) => {
            let summary = run_root
                .as_deref()
                .and_then(machine_run_surface_summary)
                .or(initial_summary);
            let detail = success_detail(&manifest, summary.as_ref());
            let status_paths = match write_status_surfaces(
                &manifest,
                manifest_path.display().to_string(),
                run_root.as_deref(),
                summary.as_ref(),
                None,
                PsionicTrainOutcomeKind::Succeeded,
                0,
                false,
                psionic_train::PsionicTrainAuthorityOwner::Pylon,
                None,
                detail.as_str(),
                &runtime_identity,
            ) {
                Ok(paths) => paths,
                Err(error) => {
                    let packet = PsionicTrainStatusPacket::refusal(
                        Some(&manifest),
                        PsionicTrainRefusalClass::InternalError,
                        Some(manifest_path.display().to_string()),
                        Some(runtime_identity.attestation.clone()),
                        Some(runtime_identity.capability_projection.clone()),
                        summary
                            .as_ref()
                            .and_then(|value| value.run_id.clone())
                            .or_else(|| manifest.run_id.clone()),
                        run_root.as_ref().map(|value| value.display().to_string()),
                        None,
                        None,
                        format!("failed to persist machine status packets: {error}"),
                    );
                    emit_refusal_packet(packet.clone());
                    return ExitCode::from(packet.exit_code);
                }
            };
            let packet = PsionicTrainStatusPacket::success(
                &manifest,
                manifest_path_text,
                runtime_identity.attestation.clone(),
                runtime_identity.capability_projection.clone(),
                summary
                    .as_ref()
                    .and_then(|value| value.run_id.clone())
                    .or_else(|| manifest.run_id.clone()),
                run_root.as_ref().map(|value| value.display().to_string()),
                status_paths.run_status_packet_path,
                status_paths.window_status_packet_path,
                summary
                    .as_ref()
                    .and_then(|value| value.current_status_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.retained_summary_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.latest_checkpoint_pointer_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.launcher_log_path.clone()),
                detail,
            );
            emit_success_packet(&packet);
            ExitCode::SUCCESS
        }
        Err(error) => {
            let refusal_summary = run_root.as_deref().and_then(machine_run_surface_summary);
            let mut packet = refusal_packet_for_error(
                Some(&manifest),
                manifest_path.display().to_string(),
                run_root.as_deref(),
                Some(&runtime_identity),
                error.as_ref(),
            );
            let status_paths = write_status_surfaces_for_packet(
                &manifest,
                manifest_path.display().to_string(),
                run_root.as_deref(),
                refusal_summary.as_ref(),
                &packet,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            let code = packet.exit_code;
            emit_refusal_packet(packet);
            ExitCode::from(code)
        }
    }
}

fn run_actual_pretraining_passthrough(args: &[String]) -> ExitCode {
    match psion_actual_pretraining_operator::run_with_args(args.to_vec()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_cs336_a1_demo_passthrough(args: &[String]) -> ExitCode {
    match run_psion_cs336_a1_demo_cli(args.to_vec()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_cs336_a1_demo_manifest_mode(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    run_root: Option<&Path>,
) -> ExitCode {
    let Some(run_root) = run_root else {
        let packet = PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            Some(runtime_identity.attestation.clone()),
            Some(runtime_identity.capability_projection.clone()),
            manifest.run_id.clone(),
            None,
            None,
            None,
            "bounded CS336 A1 demo lane requires one resolved output_root",
        );
        emit_refusal_packet(packet.clone());
        return ExitCode::from(packet.exit_code);
    };

    let initial_summary = machine_run_surface_summary(run_root);
    match run_psion_cs336_a1_demo_manifest(manifest) {
        Ok(()) => {
            let summary = machine_run_surface_summary(run_root).or(initial_summary);
            let detail = success_detail(manifest, summary.as_ref());
            let status_paths = match write_status_surfaces(
                manifest,
                manifest_path.display().to_string(),
                Some(run_root),
                summary.as_ref(),
                None,
                PsionicTrainOutcomeKind::Succeeded,
                0,
                false,
                psionic_train::PsionicTrainAuthorityOwner::Pylon,
                None,
                detail.as_str(),
                runtime_identity,
            ) {
                Ok(paths) => paths,
                Err(error) => {
                    let packet = PsionicTrainStatusPacket::refusal(
                        Some(manifest),
                        PsionicTrainRefusalClass::InternalError,
                        Some(manifest_path.display().to_string()),
                        Some(runtime_identity.attestation.clone()),
                        Some(runtime_identity.capability_projection.clone()),
                        summary
                            .as_ref()
                            .and_then(|value| value.run_id.clone())
                            .or_else(|| manifest.run_id.clone()),
                        Some(run_root.display().to_string()),
                        None,
                        None,
                        format!("failed to persist machine status packets: {error}"),
                    );
                    emit_refusal_packet(packet.clone());
                    return ExitCode::from(packet.exit_code);
                }
            };
            let packet = PsionicTrainStatusPacket::success(
                manifest,
                Some(manifest_path.display().to_string()),
                runtime_identity.attestation.clone(),
                runtime_identity.capability_projection.clone(),
                summary
                    .as_ref()
                    .and_then(|value| value.run_id.clone())
                    .or_else(|| manifest.run_id.clone()),
                Some(run_root.display().to_string()),
                status_paths.run_status_packet_path,
                status_paths.window_status_packet_path,
                summary
                    .as_ref()
                    .and_then(|value| value.current_status_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.retained_summary_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.latest_checkpoint_pointer_path.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.launcher_log_path.clone()),
                detail,
            );
            emit_success_packet(&packet);
            ExitCode::SUCCESS
        }
        Err(error) => {
            let summary = machine_run_surface_summary(run_root);
            let mut packet = refusal_packet_for_error(
                Some(manifest),
                manifest_path.display().to_string(),
                Some(run_root),
                Some(runtime_identity),
                &error,
            );
            let status_paths = write_status_surfaces_for_packet(
                manifest,
                manifest_path.display().to_string(),
                Some(run_root),
                summary.as_ref(),
                &packet,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            let code = packet.exit_code;
            emit_refusal_packet(packet);
            ExitCode::from(code)
        }
    }
}

fn run_checkpoint_handoff_manifest(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    run_root: Option<&Path>,
) -> ExitCode {
    let Some(run_root) = run_root else {
        let packet = PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            Some(runtime_identity.attestation.clone()),
            Some(runtime_identity.capability_projection.clone()),
            manifest.run_id.clone(),
            None,
            None,
            None,
            "serve-checkpoint requires one retained run_root",
        );
        emit_refusal_packet(packet.clone());
        return ExitCode::from(packet.exit_code);
    };
    let peer_node_pubkey = manifest
        .peer_node_pubkey
        .as_deref()
        .expect("validated serve-checkpoint manifests always carry peer_node_pubkey");
    let receipt = match build_psionic_train_checkpoint_handoff_receipt(
        run_root,
        manifest
            .coordination
            .node_pubkey
            .as_deref()
            .expect("validated manifests always carry coordination.node_pubkey"),
        peer_node_pubkey,
    ) {
        Ok(receipt) => receipt,
        Err(error) => {
            return emit_checkpoint_handoff_refusal(
                manifest,
                manifest_path,
                run_root,
                runtime_identity,
                &error,
                format!("failed to build peer checkpoint handoff for peer `{peer_node_pubkey}`"),
            );
        }
    };
    if let Err(error) = retain_psionic_train_checkpoint_handoff_receipt(run_root, &receipt) {
        return emit_checkpoint_handoff_refusal(
            manifest,
            manifest_path,
            run_root,
            runtime_identity,
            &error,
            String::from("failed to retain peer checkpoint handoff receipt"),
        );
    }

    let summary = machine_run_surface_summary(run_root);
    let detail = format!(
        "psionic-train completed serve-checkpoint for lane `{}` targeting peer `{peer_node_pubkey}`",
        manifest.lane_id
    );
    let status_paths = match write_status_surfaces(
        manifest,
        manifest_path.display().to_string(),
        Some(run_root),
        summary.as_ref(),
        None,
        PsionicTrainOutcomeKind::Succeeded,
        0,
        false,
        psionic_train::PsionicTrainAuthorityOwner::Pylon,
        None,
        detail.as_str(),
        runtime_identity,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            let packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::InternalError,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                summary
                    .as_ref()
                    .and_then(|value| value.run_id.clone())
                    .or_else(|| manifest.run_id.clone()),
                Some(run_root.display().to_string()),
                None,
                None,
                format!("failed to persist machine status packets: {error}"),
            );
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };
    let packet = PsionicTrainStatusPacket::success(
        manifest,
        Some(manifest_path.display().to_string()),
        runtime_identity.attestation.clone(),
        runtime_identity.capability_projection.clone(),
        summary
            .as_ref()
            .and_then(|value| value.run_id.clone())
            .or_else(|| manifest.run_id.clone()),
        Some(run_root.display().to_string()),
        status_paths.run_status_packet_path,
        status_paths.window_status_packet_path,
        summary
            .as_ref()
            .and_then(|value| value.current_status_path.clone()),
        summary
            .as_ref()
            .and_then(|value| value.retained_summary_path.clone()),
        summary
            .as_ref()
            .and_then(|value| value.latest_checkpoint_pointer_path.clone()),
        summary
            .as_ref()
            .and_then(|value| value.launcher_log_path.clone()),
        detail,
    );
    emit_success_packet(&packet);
    ExitCode::SUCCESS
}

fn run_validator_replay_manifest(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    run_root: Option<&Path>,
) -> ExitCode {
    let Some(run_root) = run_root else {
        let packet = PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            Some(runtime_identity.attestation.clone()),
            Some(runtime_identity.capability_projection.clone()),
            manifest.run_id.clone(),
            None,
            None,
            None,
            "validate-contribution requires one retained run_root",
        );
        emit_refusal_packet(packet.clone());
        return ExitCode::from(packet.exit_code);
    };

    let execution = match execute_psionic_train_validator_replay(manifest, run_root) {
        Ok(execution) => execution,
        Err(error) => {
            let refusal_class = refusal_for_validator_replay_error(&error);
            let mut packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                refusal_class,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                manifest.run_id.clone(),
                Some(run_root.display().to_string()),
                None,
                None,
                error.to_string(),
            );
            let status_paths = write_status_surfaces(
                manifest,
                manifest_path.display().to_string(),
                Some(run_root),
                None,
                None,
                packet.outcome,
                packet.exit_code,
                packet.retryable,
                packet.authority_owner,
                packet.refusal_class,
                packet.detail.as_str(),
                runtime_identity,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };

    let status_paths = match write_status_surfaces(
        manifest,
        manifest_path.display().to_string(),
        Some(run_root),
        None,
        Some(&execution.artifacts),
        PsionicTrainOutcomeKind::Succeeded,
        0,
        false,
        psionic_train::PsionicTrainAuthorityOwner::Pylon,
        None,
        execution.detail.as_str(),
        runtime_identity,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            let packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::InternalError,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                manifest.run_id.clone(),
                Some(run_root.display().to_string()),
                None,
                None,
                format!("failed to persist machine status packets: {error}"),
            );
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };
    let packet = PsionicTrainStatusPacket::success(
        manifest,
        Some(manifest_path.display().to_string()),
        runtime_identity.attestation.clone(),
        runtime_identity.capability_projection.clone(),
        manifest.run_id.clone(),
        Some(run_root.display().to_string()),
        status_paths.run_status_packet_path,
        status_paths.window_status_packet_path,
        None,
        None,
        None,
        None,
        format!(
            "{}; validator verdict `{}` at score {} bps",
            execution.detail,
            validator_disposition_label(execution.score_receipt.disposition),
            execution.score_receipt.score_bps
        ),
    );
    emit_success_packet(&packet);
    ExitCode::SUCCESS
}

fn run_apple_windowed_manifest(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    run_root: Option<&Path>,
) -> ExitCode {
    let Some(run_root) = run_root else {
        let packet = PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            Some(runtime_identity.attestation.clone()),
            Some(runtime_identity.capability_projection.clone()),
            manifest.run_id.clone(),
            None,
            None,
            None,
            "apple machine lane requires one resolved run root",
        );
        emit_refusal_packet(packet.clone());
        return ExitCode::from(packet.exit_code);
    };

    if manifest.operation == PsionicTrainOperation::RecordCheckpoint {
        if let Err(error) = write_generic_machine_checkpoint_artifacts(manifest, run_root) {
            let packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::InternalError,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                manifest.run_id.clone(),
                Some(run_root.display().to_string()),
                None,
                None,
                error,
            );
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    }

    if manifest.operation == PsionicTrainOperation::Resume {
        let has_checkpoint_surface = match inspect_psionic_train_checkpoint_surface(
            run_root,
            manifest.role,
            manifest.operation,
        ) {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(error) => {
                let mut packet = PsionicTrainStatusPacket::refusal(
                    Some(manifest),
                    PsionicTrainRefusalClass::CheckpointDigestMismatch,
                    Some(manifest_path.display().to_string()),
                    Some(runtime_identity.attestation.clone()),
                    Some(runtime_identity.capability_projection.clone()),
                    manifest.run_id.clone().or_else(|| {
                        run_root
                            .file_name()
                            .map(|value| value.to_string_lossy().to_string())
                    }),
                    Some(run_root.display().to_string()),
                    None,
                    None,
                    format!("failed to inspect retained Apple checkpoint state: {error}"),
                );
                let status_paths = write_status_surfaces(
                    manifest,
                    manifest_path.display().to_string(),
                    Some(run_root),
                    None,
                    None,
                    packet.outcome,
                    packet.exit_code,
                    packet.retryable,
                    packet.authority_owner,
                    packet.refusal_class,
                    packet.detail.as_str(),
                    runtime_identity,
                )
                .unwrap_or_default();
                packet.run_status_packet_path = status_paths.run_status_packet_path;
                packet.window_status_packet_path = status_paths.window_status_packet_path;
                emit_refusal_packet(packet.clone());
                return ExitCode::from(packet.exit_code);
            }
        };
        if !has_checkpoint_surface {
            let mut packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::CheckpointMissing,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                manifest.run_id.clone().or_else(|| {
                    run_root
                        .file_name()
                        .map(|value| value.to_string_lossy().to_string())
                }),
                Some(run_root.display().to_string()),
                None,
                None,
                String::from(
                    "apple machine lane resume requires one retained admitted checkpoint before rejoin",
                ),
            );
            let status_paths = write_status_surfaces(
                manifest,
                manifest_path.display().to_string(),
                Some(run_root),
                None,
                None,
                packet.outcome,
                packet.exit_code,
                packet.retryable,
                packet.authority_owner,
                packet.refusal_class,
                packet.detail.as_str(),
                runtime_identity,
            )
            .unwrap_or_default();
            packet.run_status_packet_path = status_paths.run_status_packet_path;
            packet.window_status_packet_path = status_paths.window_status_packet_path;
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    }

    let detail = apple_machine_success_detail(manifest);
    let status_paths = match write_status_surfaces(
        manifest,
        manifest_path.display().to_string(),
        Some(run_root),
        None,
        None,
        PsionicTrainOutcomeKind::Succeeded,
        0,
        false,
        psionic_train::PsionicTrainAuthorityOwner::Pylon,
        None,
        detail.as_str(),
        runtime_identity,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            let packet = PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::InternalError,
                Some(manifest_path.display().to_string()),
                Some(runtime_identity.attestation.clone()),
                Some(runtime_identity.capability_projection.clone()),
                manifest.run_id.clone(),
                Some(run_root.display().to_string()),
                None,
                None,
                format!("failed to persist machine status packets: {error}"),
            );
            emit_refusal_packet(packet.clone());
            return ExitCode::from(packet.exit_code);
        }
    };
    let checkpoint_pointer_path =
        run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
    let packet = PsionicTrainStatusPacket::success(
        manifest,
        Some(manifest_path.display().to_string()),
        runtime_identity.attestation.clone(),
        runtime_identity.capability_projection.clone(),
        manifest.run_id.clone().or_else(|| {
            run_root
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        }),
        Some(run_root.display().to_string()),
        status_paths.run_status_packet_path,
        status_paths.window_status_packet_path,
        None,
        None,
        checkpoint_pointer_path
            .is_file()
            .then(|| checkpoint_pointer_path.display().to_string()),
        None,
        detail,
    );
    emit_success_packet(&packet);
    ExitCode::SUCCESS
}

fn refusal_for_validator_replay_error(
    error: &PsionicTrainValidatorReplayError,
) -> PsionicTrainRefusalClass {
    match error {
        PsionicTrainValidatorReplayError::Read { .. }
        | PsionicTrainValidatorReplayError::Write { .. }
        | PsionicTrainValidatorReplayError::ArtifactIncomplete { .. } => {
            PsionicTrainRefusalClass::ArtifactIncomplete
        }
        PsionicTrainValidatorReplayError::Parse { .. }
        | PsionicTrainValidatorReplayError::ArtifactDigestMismatch { .. } => {
            PsionicTrainRefusalClass::ArtifactDigestMismatch
        }
        PsionicTrainValidatorReplayError::StaleAssignment { .. } => {
            PsionicTrainRefusalClass::StaleAssignment
        }
        PsionicTrainValidatorReplayError::CheckpointMissing { .. } => {
            PsionicTrainRefusalClass::CheckpointMissing
        }
    }
}

fn validator_disposition_label(
    disposition: psionic_train::TrainingExecutionValidatorDisposition,
) -> &'static str {
    match disposition {
        psionic_train::TrainingExecutionValidatorDisposition::Accepted => "accepted",
        psionic_train::TrainingExecutionValidatorDisposition::Quarantined => "quarantined",
        psionic_train::TrainingExecutionValidatorDisposition::Rejected => "rejected",
        psionic_train::TrainingExecutionValidatorDisposition::ReplayRequired => "replay_required",
    }
}

fn machine_runtime_source() -> Result<MachineRuntimeSource, String> {
    machine_runtime_source_from_candidates(machine_runtime_source_candidates())
}

fn machine_runtime_source_candidates() -> Vec<(String, PathBuf)> {
    let mut candidates = Vec::new();
    if let Ok(value) = env::var(PSIONIC_TRAIN_RUNTIME_ROOT_ENV) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            candidates.push((
                PSIONIC_TRAIN_RUNTIME_ROOT_ENV.to_string(),
                PathBuf::from(trimmed),
            ));
        }
    }

    if let Ok(current_exe) = env::current_exe() {
        for ancestor in current_exe.ancestors().skip(1) {
            candidates.push(("current_exe_ancestor".to_string(), ancestor.to_path_buf()));
        }
    }

    candidates.push((
        "compile_time_manifest_dir".to_string(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    ));
    candidates
}

fn machine_runtime_source_from_candidates(
    candidates: Vec<(String, PathBuf)>,
) -> Result<MachineRuntimeSource, String> {
    let mut errors = Vec::new();
    for (source, candidate) in candidates {
        let repo_root = match candidate.canonicalize() {
            Ok(repo_root) => repo_root,
            Err(error) => {
                errors.push(format!("{} {}: {error}", source, candidate.display()));
                continue;
            }
        };
        match packaged_revision_for_runtime_root(repo_root.as_path()) {
            Ok(Some(packaged_revision)) => {
                return Ok(MachineRuntimeSource {
                    repo_root,
                    packaged_revision: Some(packaged_revision),
                });
            }
            Ok(None) => {}
            Err(error) => {
                errors.push(format!("{} {}: {error}", source, repo_root.display()));
                continue;
            }
        }
        if repo_root.join("Cargo.toml").is_file()
            && repo_root.join("crates/psionic-train/src/main.rs").is_file()
            && repo_root
                .join("crates/psionic-train/src/train_runtime.rs")
                .is_file()
        {
            return Ok(MachineRuntimeSource {
                repo_root,
                packaged_revision: None,
            });
        }
        errors.push(format!(
            "{} {}: not a Psionic checkout or packaged runtime",
            source,
            repo_root.display()
        ));
    }
    Err(format!(
        "failed to resolve psionic runtime source: {}",
        errors.join("; ")
    ))
}

fn packaged_revision_for_runtime_root(repo_root: &Path) -> Result<Option<String>, String> {
    let revision_path = repo_root.join(PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE);
    if !revision_path.exists() {
        return Ok(None);
    }
    let revision = fs::read_to_string(revision_path.as_path()).map_err(|error| {
        format!(
            "failed to read packaged runtime revision marker {}: {error}",
            revision_path.display()
        )
    })?;
    let revision = revision.trim();
    if revision.is_empty() {
        return Err(format!(
            "packaged runtime revision marker {} is empty",
            revision_path.display()
        ));
    }
    Ok(Some(revision.to_string()))
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|error| {
            format!(
                "failed to execute git -C {} {}: {error}",
                repo_root.display(),
                args.join(" ")
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "git command failed: git -C {} {}",
            repo_root.display(),
            args.join(" ")
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("git output was not valid UTF-8: {error}"))
}

fn dirty_tree_posture(
    repo_root: &Path,
    allow_dirty_tree: bool,
) -> Result<(String, Option<String>), String> {
    let porcelain = git_output(repo_root, &["status", "--porcelain"])?;
    if porcelain.is_empty() {
        return Ok((String::from("refuse_by_default"), None));
    }
    if !allow_dirty_tree {
        return Err(String::from(
            "dirty working trees are refused by default; rerun with --allow-dirty-tree to override",
        ));
    }
    let status_snapshot = git_output(repo_root, &["status", "--short", "--branch"])?;
    Ok((
        String::from("allowed_by_operator_override"),
        Some(sha256_hex(status_snapshot.as_bytes())),
    ))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn resolve_runtime_identity(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
) -> Result<MachineRuntimeIdentity, PsionicTrainStatusPacket> {
    let runtime_source = machine_runtime_source().map_err(|detail| {
        PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::InternalError,
            Some(manifest_path.display().to_string()),
            None,
            None,
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            format!("failed to resolve machine runtime identity: {detail}"),
        )
    })?;
    let selected_git_ref = manifest.selected_git_ref.as_deref().ok_or_else(|| {
        PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            None,
            None,
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            "machine runtime manifest is missing selected_git_ref",
        )
    })?;
    let (git_commit_sha, dirty_tree_admission, workspace_status_sha256) = if let Some(
        packaged_revision,
    ) =
        runtime_source.packaged_revision.clone()
    {
        if selected_git_ref != PSIONIC_TRAIN_PACKAGED_RUNTIME_SELECTED_REF {
            return Err(PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                format!(
                    "packaged machine runtime requires selected_git_ref `{PSIONIC_TRAIN_PACKAGED_RUNTIME_SELECTED_REF}` but found `{selected_git_ref}`"
                ),
            ));
        }
        if manifest.allow_dirty_tree {
            return Err(PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                "packaged machine runtime cannot enable dirty-tree override",
            ));
        }
        (
            packaged_revision,
            PSIONIC_TRAIN_PACKAGED_RUNTIME_DIRTY_TREE_ADMISSION.to_string(),
            None,
        )
    } else {
        let git_commit_sha = git_output(
            runtime_source.repo_root.as_path(),
            &["rev-parse", selected_git_ref],
        )
        .map_err(|detail| {
            PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                detail,
            )
        })?;
        let (dirty_tree_admission, workspace_status_sha256) = dirty_tree_posture(
            runtime_source.repo_root.as_path(),
            manifest.allow_dirty_tree,
        )
        .map_err(|detail| {
            PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                detail,
            )
        })?;
        (
            git_commit_sha,
            dirty_tree_admission,
            workspace_status_sha256,
        )
    };
    let expected_release_id =
        admitted_release_id_for_lane(manifest.lane_id.as_str()).map_err(|error| {
            PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                error.to_string(),
            )
        })?;
    let expected_environment_ref = admitted_environment_ref_for_lane(manifest.lane_id.as_str())
        .map_err(|error| {
            PsionicTrainStatusPacket::refusal(
                Some(manifest),
                PsionicTrainRefusalClass::BadConfig,
                Some(manifest_path.display().to_string()),
                None,
                None,
                manifest.run_id.clone(),
                manifest_run_root(manifest).map(|value| value.display().to_string()),
                None,
                None,
                error.to_string(),
            )
        })?;
    let resolved_build_digest = runtime_build_digest(
        expected_release_id,
        psionic_train::PSIONIC_TRAIN_RUNTIME_SURFACE_ID,
        manifest.lane_id.as_str(),
        git_commit_sha.as_str(),
        dirty_tree_admission.as_str(),
        workspace_status_sha256.as_deref(),
        expected_environment_ref,
    );
    let attestation = PsionicTrainRuntimeAttestation::new(
        expected_release_id,
        resolved_build_digest.clone(),
        git_commit_sha,
        dirty_tree_admission,
        workspace_status_sha256,
        expected_environment_ref,
    );
    let capability_projection = PsionicTrainCapabilityProjection::for_lane(
        manifest.lane_id.as_str(),
        manifest.role,
        expected_environment_ref,
    )
    .map_err(|error| {
        PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            Some(manifest_path.display().to_string()),
            Some(attestation.clone()),
            None,
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            error.to_string(),
        )
    })?;

    if manifest.admission_identity.release_id != expected_release_id {
        return Err(PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BuildRevoked,
            Some(manifest_path.display().to_string()),
            Some(attestation.clone()),
            Some(capability_projection.clone()),
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            format!(
                "admitted release id `{}` does not match the executing runtime release `{expected_release_id}`",
                manifest.admission_identity.release_id
            ),
        ));
    }
    if manifest.admission_identity.environment_ref != expected_environment_ref {
        return Err(PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::EnvironmentMismatch,
            Some(manifest_path.display().to_string()),
            Some(attestation.clone()),
            Some(capability_projection.clone()),
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            format!(
                "admitted environment ref `{}` does not match the executing runtime environment `{expected_environment_ref}`",
                manifest.admission_identity.environment_ref
            ),
        ));
    }
    if manifest.admission_identity.build_digest != resolved_build_digest {
        return Err(PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BuildRevoked,
            Some(manifest_path.display().to_string()),
            Some(attestation.clone()),
            Some(capability_projection.clone()),
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            format!(
                "admitted build digest `{}` does not match the executing runtime build `{resolved_build_digest}`",
                manifest.admission_identity.build_digest
            ),
        ));
    }

    Ok(MachineRuntimeIdentity {
        attestation,
        capability_projection,
    })
}

fn parse_manifest_path(args: &[String]) -> Result<PathBuf, String> {
    let mut args = args.iter();
    let Some(flag) = args.next() else {
        return Err(String::from(
            "manifest mode requires --manifest <path> to a machine-consumable invocation manifest",
        ));
    };
    if flag != "--manifest" {
        return Err(format!(
            "manifest mode expected `--manifest <path>` but found `{flag}`"
        ));
    }
    let Some(path) = args.next() else {
        return Err(String::from(
            "manifest mode requires one path after --manifest",
        ));
    };
    if args.next().is_some() {
        return Err(String::from(
            "manifest mode accepts only `--manifest <path>` and no trailing arguments",
        ));
    }
    Ok(PathBuf::from(path))
}

fn load_manifest(path: &Path) -> Result<PsionicTrainInvocationManifest, PsionicTrainStatusPacket> {
    let bytes = fs::read(path).map_err(|error| {
        PsionicTrainStatusPacket::refusal(
            None,
            PsionicTrainRefusalClass::BadConfig,
            Some(path.display().to_string()),
            None,
            None,
            None,
            None,
            None,
            None,
            format!(
                "failed to read psionic-train invocation manifest `{}`: {error}",
                path.display()
            ),
        )
    })?;
    let manifest: PsionicTrainInvocationManifest =
        serde_json::from_slice(&bytes).map_err(|error| {
            PsionicTrainStatusPacket::refusal(
                None,
                PsionicTrainRefusalClass::BadConfig,
                Some(path.display().to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                format!(
                    "failed to parse psionic-train invocation manifest `{}`: {error}",
                    path.display()
                ),
            )
        })?;
    manifest.validate_machine_contract().map_err(|error| {
        PsionicTrainStatusPacket::refusal(
            Some(&manifest),
            refusal_for_contract_error(&error),
            Some(path.display().to_string()),
            None,
            None,
            manifest.run_id.clone(),
            manifest_run_root(&manifest).map(|value| value.display().to_string()),
            None,
            None,
            format!(
                "psionic-train invocation manifest `{}` failed validation: {error}",
                path.display()
            ),
        )
    })?;
    Ok(manifest)
}

fn operator_args_from_manifest(
    manifest: &PsionicTrainInvocationManifest,
) -> Result<Vec<String>, PsionicTrainStatusPacket> {
    if matches!(
        manifest.operation,
        PsionicTrainOperation::ServeCheckpoint | PsionicTrainOperation::ValidateContribution
    ) {
        return Err(PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::InternalError,
            None,
            None,
            None,
            manifest.run_id.clone(),
            manifest_run_root(manifest).map(|value| value.display().to_string()),
            None,
            None,
            "serve-checkpoint and validate-contribution are handled directly by manifest mode and must not be forwarded to the actual-lane operator CLI",
        ));
    }
    let mut args = vec![String::from(manifest.operation.cli_subcommand())];
    match manifest.operation {
        PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane => {
            args.push(String::from("--run-id"));
            args.push(manifest.run_id.clone().ok_or_else(|| {
                PsionicTrainStatusPacket::refusal(
                    Some(manifest),
                    PsionicTrainRefusalClass::BadConfig,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    "machine runtime manifest is missing run_id for launch-style operation",
                )
            })?);
            args.push(String::from("--output-root"));
            args.push(manifest.output_root.clone().ok_or_else(|| {
                PsionicTrainStatusPacket::refusal(
                    Some(manifest),
                    PsionicTrainRefusalClass::BadConfig,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    "machine runtime manifest is missing output_root for launch-style operation",
                )
            })?);
        }
        _ => {
            args.push(String::from("--run-root"));
            args.push(manifest.run_root.clone().ok_or_else(|| {
                PsionicTrainStatusPacket::refusal(
                    Some(manifest),
                    PsionicTrainRefusalClass::BadConfig,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    "machine runtime manifest is missing run_root for retained-state operation",
                )
            })?);
        }
    }

    args.push(String::from("--git-ref"));
    args.push(manifest.selected_git_ref.clone().ok_or_else(|| {
        PsionicTrainStatusPacket::refusal(
            Some(manifest),
            PsionicTrainRefusalClass::BadConfig,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            "machine runtime manifest is missing selected_git_ref",
        )
    })?);

    if let Some(path) = manifest.hardware_observation_path.clone() {
        args.push(String::from("--hardware-observation"));
        args.push(path);
    }
    if let Some(path) = manifest.run_shape_observation_path.clone() {
        args.push(String::from("--run-shape-observation"));
        args.push(path);
    }
    if manifest.allow_dirty_tree {
        args.push(String::from("--allow-dirty-tree"));
    }
    if manifest.dry_run {
        args.push(String::from("--dry-run"));
    }
    if let Some(label) = manifest.checkpoint_label.clone() {
        args.push(String::from("--checkpoint-label"));
        args.push(label);
    }
    if let Some(step) = manifest.optimizer_step {
        args.push(String::from("--optimizer-step"));
        args.push(step.to_string());
    }
    if let Some(checkpoint_ref) = manifest.checkpoint_ref.clone() {
        args.push(String::from("--checkpoint-ref"));
        args.push(checkpoint_ref);
    }
    if let Some(digest) = manifest.checkpoint_object_digest.clone() {
        args.push(String::from("--checkpoint-object-digest"));
        args.push(digest);
    }
    if let Some(total_bytes) = manifest.checkpoint_total_bytes {
        args.push(String::from("--checkpoint-total-bytes"));
        args.push(total_bytes.to_string());
    }
    if manifest.inject_failed_upload {
        args.push(String::from("--inject-failed-upload"));
    }
    if manifest.inject_eval_worker_unavailable {
        args.push(String::from("--inject-eval-worker-unavailable"));
    }

    Ok(args)
}

fn manifest_run_root(manifest: &PsionicTrainInvocationManifest) -> Option<PathBuf> {
    match manifest.operation {
        PsionicTrainOperation::Start | PsionicTrainOperation::RehearseBaseLane => {
            manifest.output_root.as_ref().map(PathBuf::from)
        }
        PsionicTrainOperation::Resume
        | PsionicTrainOperation::ServeCheckpoint
        | PsionicTrainOperation::ValidateContribution
        | PsionicTrainOperation::RecordCheckpoint
        | PsionicTrainOperation::Backup
        | PsionicTrainOperation::DecideContinueRestart => {
            manifest.run_root.as_ref().map(PathBuf::from)
        }
    }
}

fn success_detail(
    manifest: &PsionicTrainInvocationManifest,
    summary: Option<&MachineRunSurfaceSummary>,
) -> String {
    match summary.and_then(|value| value.phase.as_deref()) {
        Some(phase) => format!(
            "psionic-train completed {} for lane `{}` with retained phase `{phase}`",
            manifest.operation.cli_subcommand(),
            manifest.lane_id
        ),
        None => format!(
            "psionic-train completed {} for lane `{}`",
            manifest.operation.cli_subcommand(),
            manifest.lane_id
        ),
    }
}

fn refusal_packet_for_error(
    manifest: Option<&PsionicTrainInvocationManifest>,
    manifest_path: String,
    run_root: Option<&Path>,
    runtime_identity: Option<&MachineRuntimeIdentity>,
    error: &dyn Error,
) -> PsionicTrainStatusPacket {
    let detail = error.to_string();
    let refusal_class = classify_operator_error(&detail);
    let summary = run_root.and_then(machine_run_surface_summary);
    PsionicTrainStatusPacket::refusal(
        manifest,
        refusal_class,
        Some(manifest_path),
        runtime_identity.map(|value| value.attestation.clone()),
        runtime_identity.map(|value| value.capability_projection.clone()),
        summary
            .as_ref()
            .and_then(|value| value.run_id.clone())
            .or_else(|| manifest.and_then(|value| value.run_id.clone())),
        run_root.map(|value| value.display().to_string()),
        None,
        None,
        detail,
    )
}

fn classify_operator_error(detail: &str) -> PsionicTrainRefusalClass {
    let lower = detail.to_ascii_lowercase();
    if lower.contains("requires --")
        || lower.contains("missing subcommand")
        || lower.contains("unsupported subcommand")
        || lower.contains("unknown argument")
        || lower.contains("dirty working trees are refused by default")
    {
        PsionicTrainRefusalClass::BadConfig
    } else if lower.contains("launch refused preflight admission") {
        PsionicTrainRefusalClass::UnsupportedTopology
    } else if lower.contains("accepted checkpoint pointer")
        || lower.contains("checkpoint manifest path")
        || lower.contains("no admitted backup receipt was available")
        || lower.contains("latest checkpoint backup receipt is not durable enough")
        || lower.contains("could not resolve an admitted checkpoint pointer")
        || lower.contains("primary pointer could not be resumed")
    {
        PsionicTrainRefusalClass::CheckpointMissing
    } else if lower.contains("drifted from the accepted checkpoint pointer")
        || lower.contains("checkpoint manifest ref drifted")
    {
        PsionicTrainRefusalClass::CheckpointDigestMismatch
    } else if lower.contains("failed-upload drill") || lower.contains("durable backup confirmation")
    {
        PsionicTrainRefusalClass::ArtifactIncomplete
    } else {
        PsionicTrainRefusalClass::InternalError
    }
}

fn refusal_for_contract_error(
    error: &PsionicTrainRuntimeContractError,
) -> PsionicTrainRefusalClass {
    match error {
        PsionicTrainRuntimeContractError::InvalidValue { field, .. }
            if field == "invocation_manifest.lane_id" =>
        {
            PsionicTrainRefusalClass::UnsupportedTopology
        }
        PsionicTrainRuntimeContractError::InvalidValue { field, .. }
            if field.starts_with("invocation_manifest.grouped_stage_assignment") =>
        {
            PsionicTrainRefusalClass::GroupedStageAssignmentInvalid
        }
        PsionicTrainRuntimeContractError::FieldMismatch { field, .. }
            if field.starts_with("invocation_manifest.grouped_stage_assignment") =>
        {
            PsionicTrainRefusalClass::GroupedStageAssignmentInvalid
        }
        PsionicTrainRuntimeContractError::MissingField { field }
            if field.starts_with("invocation_manifest.grouped_stage_assignment") =>
        {
            PsionicTrainRefusalClass::GroupedStageAssignmentInvalid
        }
        PsionicTrainRuntimeContractError::FieldMismatch { field, .. }
            if field == "invocation_manifest.manifest_digest" =>
        {
            PsionicTrainRefusalClass::ArtifactDigestMismatch
        }
        _ => PsionicTrainRefusalClass::BadConfig,
    }
}

fn refusal_for_checkpoint_handoff_error(
    error: &PsionicTrainCheckpointHandoffError,
) -> PsionicTrainRefusalClass {
    match error {
        PsionicTrainCheckpointHandoffError::MissingCheckpoint { .. } => {
            PsionicTrainRefusalClass::CheckpointMissing
        }
        PsionicTrainCheckpointHandoffError::Parse { .. }
        | PsionicTrainCheckpointHandoffError::Invalid { .. } => {
            PsionicTrainRefusalClass::CheckpointDigestMismatch
        }
        PsionicTrainCheckpointHandoffError::Read { .. } => {
            PsionicTrainRefusalClass::CheckpointMissing
        }
        PsionicTrainCheckpointHandoffError::Write { .. } => {
            PsionicTrainRefusalClass::ArtifactIncomplete
        }
    }
}

fn refusal_for_grouped_stage_transport_error(
    error: &PsionicTrainGroupedReplicaTransportError,
) -> PsionicTrainRefusalClass {
    match error {
        PsionicTrainGroupedReplicaTransportError::Read { .. }
        | PsionicTrainGroupedReplicaTransportError::Write { .. } => {
            PsionicTrainRefusalClass::ArtifactIncomplete
        }
        PsionicTrainGroupedReplicaTransportError::Parse { .. }
        | PsionicTrainGroupedReplicaTransportError::ArtifactDigestMismatch { .. } => {
            PsionicTrainRefusalClass::ArtifactDigestMismatch
        }
        PsionicTrainGroupedReplicaTransportError::Invalid { .. } => {
            PsionicTrainRefusalClass::GroupedStageAssignmentInvalid
        }
        PsionicTrainGroupedReplicaTransportError::StaleAssignment { .. } => {
            PsionicTrainRefusalClass::StaleAssignment
        }
    }
}

fn refusal_for_grouped_stage_checkpoint_error(
    error: &PsionicTrainGroupedReplicaCheckpointError,
) -> PsionicTrainRefusalClass {
    match error {
        PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint { .. } => {
            PsionicTrainRefusalClass::CheckpointMissing
        }
        PsionicTrainGroupedReplicaCheckpointError::StaleAssignment { .. } => {
            PsionicTrainRefusalClass::StaleAssignment
        }
        PsionicTrainGroupedReplicaCheckpointError::Read { .. }
        | PsionicTrainGroupedReplicaCheckpointError::Write { .. } => {
            PsionicTrainRefusalClass::ArtifactIncomplete
        }
        PsionicTrainGroupedReplicaCheckpointError::Parse { .. }
        | PsionicTrainGroupedReplicaCheckpointError::Invalid { .. } => {
            PsionicTrainRefusalClass::CheckpointDigestMismatch
        }
    }
}

fn persist_grouped_stage_resume_recovery(
    manifest: &PsionicTrainInvocationManifest,
    run_root: &Path,
    recovery_source_kind: PsionicTrainGroupedReplicaRecoverySourceKind,
) -> Result<(), PsionicTrainGroupedReplicaCheckpointError> {
    if manifest.grouped_stage_assignment.is_none() {
        return Ok(());
    }
    let checkpoint_surface = inspect_psionic_train_checkpoint_surface(
        run_root,
        manifest.role,
        manifest.operation,
    )
    .map_err(|error| PsionicTrainGroupedReplicaCheckpointError::Invalid {
        detail: error.to_string(),
    })?
    .ok_or_else(
        || PsionicTrainGroupedReplicaCheckpointError::MissingCheckpoint {
            detail: String::from(
                "grouped stage resume requires one retained checkpoint surface under the run root",
            ),
        },
    )?;
    persist_psionic_train_grouped_stage_recovery_receipt_from_surface(
        run_root,
        manifest,
        &checkpoint_surface,
        recovery_source_kind,
    )?;
    Ok(())
}

fn emit_checkpoint_handoff_refusal(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: &Path,
    run_root: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    error: &PsionicTrainCheckpointHandoffError,
    detail_prefix: String,
) -> ExitCode {
    let summary = machine_run_surface_summary(run_root);
    let mut packet = PsionicTrainStatusPacket::refusal(
        Some(manifest),
        refusal_for_checkpoint_handoff_error(error),
        Some(manifest_path.display().to_string()),
        Some(runtime_identity.attestation.clone()),
        Some(runtime_identity.capability_projection.clone()),
        summary
            .as_ref()
            .and_then(|value| value.run_id.clone())
            .or_else(|| manifest.run_id.clone()),
        Some(run_root.display().to_string()),
        None,
        None,
        format!("{detail_prefix}: {error}"),
    );
    let status_paths = write_status_surfaces_for_packet(
        manifest,
        manifest_path.display().to_string(),
        Some(run_root),
        summary.as_ref(),
        &packet,
    )
    .unwrap_or_default();
    packet.run_status_packet_path = status_paths.run_status_packet_path;
    packet.window_status_packet_path = status_paths.window_status_packet_path;
    let code = packet.exit_code;
    emit_refusal_packet(packet);
    ExitCode::from(code)
}

#[derive(Clone, Debug)]
struct MachineRunSurfaceSummary {
    run_id: Option<String>,
    phase: Option<String>,
    current_status_path: Option<String>,
    retained_summary_path: Option<String>,
    latest_checkpoint_pointer_path: Option<String>,
    auto_resume_receipt_path: Option<String>,
    closeout_bundle_path: Option<String>,
    launch_manifest_path: Option<String>,
    launcher_log_path: Option<String>,
}

fn machine_run_surface_summary(run_root: &Path) -> Option<MachineRunSurfaceSummary> {
    let current_status_path = run_root.join("status/current_run_status.json");
    let current_status = if current_status_path.is_file() {
        fs::read_to_string(&current_status_path)
            .ok()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
    } else {
        None
    };
    Some(MachineRunSurfaceSummary {
        run_id: current_status
            .as_ref()
            .and_then(|value| value.get("run_id"))
            .and_then(|value| value.as_str())
            .map(String::from),
        phase: current_status
            .as_ref()
            .and_then(|value| value.get("phase"))
            .and_then(|value| value.as_str())
            .map(String::from),
        current_status_path: current_status_path
            .is_file()
            .then(|| current_status_path.display().to_string()),
        retained_summary_path: {
            let path = run_root.join("status/retained_summary.json");
            path.is_file().then(|| path.display().to_string())
        },
        latest_checkpoint_pointer_path: {
            let path = run_root.join("checkpoints/latest_accepted_checkpoint_pointer.json");
            path.is_file().then(|| path.display().to_string())
        },
        auto_resume_receipt_path: {
            let path = run_root.join("checkpoints/auto_resume_receipt.json");
            path.is_file().then(|| path.display().to_string())
        },
        closeout_bundle_path: {
            let path = run_root.join("closeout/closeout_bundle.json");
            path.is_file().then(|| path.display().to_string())
        },
        launch_manifest_path: {
            let start_path = run_root.join("manifests/launch_manifest.json");
            if start_path.is_file() {
                Some(start_path.display().to_string())
            } else {
                let resume_path = run_root.join("manifests/resume_manifest.json");
                resume_path
                    .is_file()
                    .then(|| resume_path.display().to_string())
            }
        },
        launcher_log_path: {
            let path = run_root.join("logs/launcher.log");
            path.is_file().then(|| path.display().to_string())
        },
    })
}

fn write_status_surfaces_for_packet(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: String,
    run_root: Option<&Path>,
    summary: Option<&MachineRunSurfaceSummary>,
    packet: &PsionicTrainStatusPacket,
) -> Result<MachineStatusSurfacePaths, String> {
    let Some(attestation) = packet.runtime_attestation.clone() else {
        return Ok(MachineStatusSurfacePaths::default());
    };
    let Some(capability_projection) = packet.capability_projection.clone() else {
        return Ok(MachineStatusSurfacePaths::default());
    };
    write_status_surfaces(
        manifest,
        manifest_path,
        run_root,
        summary,
        None,
        packet.outcome,
        packet.exit_code,
        packet.retryable,
        packet.authority_owner,
        packet.refusal_class,
        packet.detail.as_str(),
        &MachineRuntimeIdentity {
            attestation,
            capability_projection,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn write_status_surfaces(
    manifest: &PsionicTrainInvocationManifest,
    manifest_path: String,
    run_root: Option<&Path>,
    summary: Option<&MachineRunSurfaceSummary>,
    validator_artifacts: Option<&PsionicTrainValidatorArtifactOutputs>,
    outcome: PsionicTrainOutcomeKind,
    exit_code: u8,
    retryable: bool,
    authority_owner: psionic_train::PsionicTrainAuthorityOwner,
    refusal_class: Option<PsionicTrainRefusalClass>,
    detail: &str,
    runtime_identity: &MachineRuntimeIdentity,
) -> Result<MachineStatusSurfacePaths, String> {
    let Some(run_root) = run_root else {
        return Ok(MachineStatusSurfacePaths::default());
    };
    let status_dir = run_root.join("status");
    fs::create_dir_all(&status_dir).map_err(|error| {
        format!(
            "failed to create status directory `{}`: {error}",
            status_dir.display()
        )
    })?;
    let run_status_path = status_dir.join("psionic_train_run_status_packet.json");
    let window_status_path = status_dir.join("psionic_train_window_status_packet.json");
    let resolved_run_id = summary
        .and_then(|value| value.run_id.clone())
        .or_else(|| manifest.run_id.clone())
        .or_else(|| {
            run_root
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| String::from("unknown_run"));
    let membership_revision_path = write_membership_revision_receipt(
        manifest,
        run_root,
        runtime_identity,
        resolved_run_id.as_str(),
        outcome,
    )?;
    let checkpoint_surface = write_checkpoint_surface(run_root, manifest)?;
    let window_artifact_inputs = build_window_artifact_inputs(
        manifest,
        summary,
        manifest_path.as_str(),
        membership_revision_path.clone(),
        checkpoint_surface.as_ref(),
    );
    let window_artifacts = persist_psionic_train_window_artifacts(
        manifest,
        &runtime_identity.attestation,
        &runtime_identity.capability_projection,
        resolved_run_id.as_str(),
        run_root,
        &window_artifact_inputs,
        outcome,
        exit_code,
        retryable,
        authority_owner,
        refusal_class,
        detail,
    )
    .map_err(|error| format!("failed to persist window contribution artifacts: {error}"))?;
    let artifacts = build_artifact_surface_refs(
        manifest,
        summary,
        membership_revision_path.clone(),
        checkpoint_surface.as_ref(),
        validator_artifacts,
        window_artifacts.as_ref(),
    );
    let run_packet = PsionicTrainRunStatusPacket {
        schema_version: String::from(psionic_train::PSIONIC_TRAIN_RUN_STATUS_PACKET_SCHEMA_VERSION),
        runtime_surface_id: String::from(psionic_train::PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
        lane_id: manifest.lane_id.clone(),
        role: manifest.role,
        operation: manifest.operation,
        work_class: manifest.work_class,
        outcome,
        exit_code,
        retryable,
        authority_owner,
        refusal_class,
        coordination: manifest.coordination.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        validator_target_work_class: manifest.validator_target_work_class,
        manifest_path: Some(manifest_path.clone()),
        manifest_digest: manifest.manifest_digest.clone(),
        run_id: Some(resolved_run_id.clone()),
        run_root: Some(run_root.display().to_string()),
        phase: summary.and_then(|value| value.phase.clone()),
        runtime_attestation: runtime_identity.attestation.clone(),
        capability_projection: runtime_identity.capability_projection.clone(),
        artifacts: artifacts.clone(),
        current_status_path: summary.and_then(|value| value.current_status_path.clone()),
        retained_summary_path: summary.and_then(|value| value.retained_summary_path.clone()),
        launcher_log_path: summary.and_then(|value| value.launcher_log_path.clone()),
        detail: String::from(detail),
    };
    let window_packet = PsionicTrainWindowStatusPacket {
        schema_version: String::from(
            psionic_train::PSIONIC_TRAIN_WINDOW_STATUS_PACKET_SCHEMA_VERSION,
        ),
        runtime_surface_id: String::from(psionic_train::PSIONIC_TRAIN_RUNTIME_SURFACE_ID),
        lane_id: manifest.lane_id.clone(),
        role: manifest.role,
        operation: manifest.operation,
        work_class: manifest.work_class,
        outcome,
        exit_code,
        retryable,
        authority_owner,
        refusal_class,
        coordination: manifest.coordination.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        validator_target_work_class: manifest.validator_target_work_class,
        manifest_digest: manifest.manifest_digest.clone(),
        run_id: Some(resolved_run_id),
        run_root: Some(run_root.display().to_string()),
        window_state: manifest.coordination.window_id.as_ref().map(|_| {
            summary
                .and_then(|value| value.phase.clone())
                .unwrap_or_else(|| String::from("window_context_declared"))
        }),
        runtime_attestation: runtime_identity.attestation.clone(),
        capability_projection: runtime_identity.capability_projection.clone(),
        artifacts,
        detail: if manifest.coordination.window_id.is_some() {
            String::from(detail)
        } else {
            format!(
                "{detail}; this admitted lane does not yet materialize dedicated sealed-window artifacts"
            )
        },
    };
    fs::write(
        &run_status_path,
        serde_json::to_vec_pretty(&run_packet)
            .map_err(|error| format!("failed to serialize run-status packet: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "failed to write run-status packet `{}`: {error}",
            run_status_path.display()
        )
    })?;
    fs::write(
        &window_status_path,
        serde_json::to_vec_pretty(&window_packet)
            .map_err(|error| format!("failed to serialize window-status packet: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "failed to write window-status packet `{}`: {error}",
            window_status_path.display()
        )
    })?;
    Ok(MachineStatusSurfacePaths {
        run_status_packet_path: Some(run_status_path.display().to_string()),
        window_status_packet_path: Some(window_status_path.display().to_string()),
    })
}

fn build_artifact_surface_refs(
    manifest: &PsionicTrainInvocationManifest,
    summary: Option<&MachineRunSurfaceSummary>,
    membership_revision_path: Option<String>,
    checkpoint_surface: Option<&RetainedCheckpointSurface>,
    validator_artifacts: Option<&PsionicTrainValidatorArtifactOutputs>,
    window_artifacts: Option<&PsionicTrainWindowArtifactOutputs>,
) -> PsionicTrainArtifactSurfaceRefs {
    PsionicTrainArtifactSurfaceRefs {
        launch_manifest_path: summary.and_then(|value| value.launch_manifest_path.clone()),
        membership_revision_path,
        window_execution_path: window_artifacts.map(|value| value.window_execution_path.clone()),
        contribution_receipt_path: window_artifacts
            .map(|value| value.contribution_receipt_path.clone()),
        contribution_artifact_manifest_path: window_artifacts
            .map(|value| value.contribution_artifact_manifest_path.clone()),
        grouped_stage_input_transport_path: manifest
            .grouped_stage_input_transport
            .as_ref()
            .and_then(|value| value.materialized_path.clone()),
        grouped_stage_output_transport_path: window_artifacts
            .and_then(|value| value.grouped_stage_output_transport_path.clone()),
        grouped_stage_output_payload_path: window_artifacts
            .and_then(|value| value.grouped_stage_output_payload_path.clone()),
        grouped_stage_execution_summary_path: window_artifacts
            .and_then(|value| value.grouped_stage_execution_summary_path.clone()),
        grouped_stage_replay_evidence_path: validator_artifacts
            .and_then(|value| value.grouped_stage_replay_evidence_path.clone()),
        checkpoint_surface_path: checkpoint_surface.map(|value| value.path.clone()),
        checkpoint_pointer_path: checkpoint_surface
            .and_then(|value| value.surface.artifacts.checkpoint_pointer_path.clone())
            .or_else(|| summary.and_then(|value| value.latest_checkpoint_pointer_path.clone())),
        checkpoint_manifest_path: checkpoint_surface
            .and_then(|value| value.surface.artifacts.checkpoint_manifest_path.clone()),
        checkpoint_backup_receipt_path: checkpoint_surface.and_then(|value| {
            value
                .surface
                .artifacts
                .checkpoint_backup_receipt_path
                .clone()
        }),
        checkpoint_handoff_receipt_path: checkpoint_surface.and_then(|value| {
            value
                .surface
                .artifacts
                .peer_checkpoint_handoff_receipt_path
                .clone()
        }),
        recovery_receipt_path: checkpoint_surface
            .and_then(|value| {
                value
                    .surface
                    .artifacts
                    .grouped_stage_recovery_receipt_path
                    .clone()
                    .or_else(|| value.surface.artifacts.auto_resume_receipt_path.clone())
            })
            .or_else(|| summary.and_then(|value| value.auto_resume_receipt_path.clone())),
        validator_score_receipt_path: validator_artifacts
            .map(|value| value.validator_score_receipt_path.clone()),
        validator_quality_drift_signal_path: validator_artifacts
            .map(|value| value.validator_quality_drift_signal_path.clone()),
        validator_rollback_signal_path: validator_artifacts
            .map(|value| value.validator_rollback_signal_path.clone()),
        weak_device_validation_replay_proof_path: validator_artifacts
            .and_then(|value| value.weak_device_validation_replay_proof_path.clone()),
        sealed_window_bundle_path: window_artifacts
            .map(|value| value.sealed_window_bundle_path.clone()),
        final_closeout_bundle_path: summary.and_then(|value| value.closeout_bundle_path.clone()),
    }
}

fn build_window_artifact_inputs(
    manifest: &PsionicTrainInvocationManifest,
    summary: Option<&MachineRunSurfaceSummary>,
    manifest_path: &str,
    membership_revision_path: Option<String>,
    checkpoint_surface: Option<&RetainedCheckpointSurface>,
) -> PsionicTrainWindowArtifactInputRefs {
    PsionicTrainWindowArtifactInputRefs {
        invocation_manifest_path: String::from(manifest_path),
        launch_manifest_path: summary.and_then(|value| value.launch_manifest_path.clone()),
        membership_revision_path,
        grouped_stage_input_transport_path: manifest
            .grouped_stage_input_transport
            .as_ref()
            .and_then(|value| value.materialized_path.clone()),
        checkpoint_surface_path: checkpoint_surface.map(|value| value.path.clone()),
        checkpoint_pointer_path: checkpoint_surface
            .and_then(|value| value.surface.artifacts.checkpoint_pointer_path.clone())
            .or_else(|| summary.and_then(|value| value.latest_checkpoint_pointer_path.clone())),
        checkpoint_manifest_path: checkpoint_surface
            .and_then(|value| value.surface.artifacts.checkpoint_manifest_path.clone()),
        checkpoint_backup_receipt_path: checkpoint_surface.and_then(|value| {
            value
                .surface
                .artifacts
                .checkpoint_backup_receipt_path
                .clone()
        }),
        checkpoint_handoff_receipt_path: checkpoint_surface.and_then(|value| {
            value
                .surface
                .artifacts
                .peer_checkpoint_handoff_receipt_path
                .clone()
        }),
        recovery_receipt_path: checkpoint_surface
            .and_then(|value| {
                value
                    .surface
                    .artifacts
                    .grouped_stage_recovery_receipt_path
                    .clone()
                    .or_else(|| value.surface.artifacts.auto_resume_receipt_path.clone())
            })
            .or_else(|| summary.and_then(|value| value.auto_resume_receipt_path.clone())),
        current_status_path: summary.and_then(|value| value.current_status_path.clone()),
        retained_summary_path: summary.and_then(|value| value.retained_summary_path.clone()),
        launcher_log_path: summary.and_then(|value| value.launcher_log_path.clone()),
        final_closeout_bundle_path: summary.and_then(|value| value.closeout_bundle_path.clone()),
    }
}

fn emit_success_packet(packet: &PsionicTrainStatusPacket) {
    println!(
        "{}",
        serde_json::to_string_pretty(packet).expect("status packet should serialize")
    );
}

fn emit_refusal_packet(packet: PsionicTrainStatusPacket) {
    eprintln!(
        "{}",
        serde_json::to_string_pretty(&packet).expect("status packet should serialize")
    );
}

fn write_generic_machine_checkpoint_artifacts(
    manifest: &PsionicTrainInvocationManifest,
    run_root: &Path,
) -> Result<(), String> {
    let checkpoint_label = manifest
        .checkpoint_label
        .as_deref()
        .ok_or_else(|| String::from("record-checkpoint requires checkpoint_label"))?;
    let optimizer_step = manifest
        .optimizer_step
        .ok_or_else(|| String::from("record-checkpoint requires optimizer_step"))?;
    let checkpoint_ref = manifest
        .checkpoint_ref
        .as_deref()
        .ok_or_else(|| String::from("record-checkpoint requires checkpoint_ref"))?;
    let run_id = manifest
        .run_id
        .clone()
        .or_else(|| {
            run_root
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| String::from("unknown_run"));
    let checkpoints_dir = run_root.join("checkpoints");
    let manifests_dir = checkpoints_dir.join("manifests");
    fs::create_dir_all(&manifests_dir).map_err(|error| {
        format!(
            "failed to create generic checkpoint manifest directory `{}`: {error}",
            manifests_dir.display()
        )
    })?;

    let relative_manifest_path =
        format!("checkpoints/manifests/checkpoint_manifest_step-{optimizer_step:06}.json");
    let checkpoint_object_digest = manifest
        .checkpoint_object_digest
        .clone()
        .unwrap_or_else(|| stable_generic_checkpoint_object_digest(manifest, run_id.as_str()));
    let checkpoint_total_bytes = manifest.checkpoint_total_bytes.unwrap_or(65_536);
    let mut checkpoint_manifest = PsionicTrainCheckpointManifest {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_MANIFEST_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        run_id: run_id.clone(),
        window_id: manifest.coordination.window_id.clone(),
        assignment_id: manifest.coordination.assignment_id.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        relative_manifest_path: relative_manifest_path.clone(),
        checkpoint_object_digest,
        checkpoint_total_bytes,
        manifest_digest: String::new(),
    };
    checkpoint_manifest.manifest_digest = checkpoint_manifest.stable_manifest_digest();

    let checkpoint_pointer = PsionicTrainCheckpointPointer {
        schema_version: String::from(PSIONIC_TRAIN_CHECKPOINT_POINTER_SCHEMA_VERSION),
        lane_id: manifest.lane_id.clone(),
        run_id,
        window_id: manifest.coordination.window_id.clone(),
        assignment_id: manifest.coordination.assignment_id.clone(),
        grouped_stage_assignment: manifest.grouped_stage_assignment.clone(),
        pointer_state: String::from("accepted"),
        checkpoint_label: String::from(checkpoint_label),
        optimizer_step,
        checkpoint_ref: String::from(checkpoint_ref),
        checkpoint_manifest_relative_path: relative_manifest_path,
        detail: if let Some(stage_assignment) = manifest.grouped_stage_assignment.as_ref() {
            format!(
                "Grouped-replica stage `{}` retained one accepted stage checkpoint pointer under the shared machine checkpoint contract.",
                stage_assignment.stage_id
            )
        } else {
            String::from(
                "Bounded Apple machine lane retained one accepted checkpoint pointer under the shared machine checkpoint contract.",
            )
        },
    };

    let pointer_path = checkpoints_dir.join("latest_accepted_checkpoint_pointer.json");
    let manifest_path = run_root.join(&checkpoint_manifest.relative_manifest_path);
    fs::write(
        &pointer_path,
        serde_json::to_vec_pretty(&checkpoint_pointer)
            .map_err(|error| format!("failed to serialize generic checkpoint pointer: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "failed to write generic checkpoint pointer `{}`: {error}",
            pointer_path.display()
        )
    })?;
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&checkpoint_manifest)
            .map_err(|error| format!("failed to serialize generic checkpoint manifest: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "failed to write generic checkpoint manifest `{}`: {error}",
            manifest_path.display()
        )
    })?;
    Ok(())
}

fn stable_generic_checkpoint_object_digest(
    manifest: &PsionicTrainInvocationManifest,
    run_id: &str,
) -> String {
    let mut digest = sha2::Sha256::new();
    digest.update(b"psionic_train_generic_checkpoint_object|");
    digest.update(manifest.lane_id.as_bytes());
    digest.update(b"|");
    digest.update(run_id.as_bytes());
    digest.update(b"|");
    digest.update(
        manifest
            .checkpoint_label
            .as_deref()
            .unwrap_or("unknown_checkpoint")
            .as_bytes(),
    );
    digest.update(b"|");
    digest.update(
        manifest
            .checkpoint_ref
            .as_deref()
            .unwrap_or("checkpoint://unknown")
            .as_bytes(),
    );
    format!("{:x}", digest.finalize())
}

fn apple_machine_success_detail(manifest: &PsionicTrainInvocationManifest) -> String {
    match manifest.operation {
        PsionicTrainOperation::Start => format!(
            "psionic-train admitted one Apple-homogeneous worker window under lane `{}` using the shared machine manifest and status contract",
            manifest.lane_id
        ),
        PsionicTrainOperation::Resume => format!(
            "psionic-train resumed one Apple-homogeneous worker window under lane `{}` using the shared machine recovery contract",
            manifest.lane_id
        ),
        PsionicTrainOperation::RecordCheckpoint => format!(
            "psionic-train retained one Apple-homogeneous checkpoint pointer and manifest under lane `{}` using the shared machine checkpoint contract",
            manifest.lane_id
        ),
        PsionicTrainOperation::Backup => format!(
            "psionic-train acknowledged bounded Apple backup posture for lane `{}` without widening beyond the shared machine contract",
            manifest.lane_id
        ),
        PsionicTrainOperation::DecideContinueRestart => format!(
            "psionic-train retained bounded Apple continue-restart posture for lane `{}` under the shared machine recovery contract",
            manifest.lane_id
        ),
        PsionicTrainOperation::RehearseBaseLane => format!(
            "psionic-train rehearsed the bounded Apple machine lane `{}` under the shared manifest contract",
            manifest.lane_id
        ),
        other => format!(
            "psionic-train completed Apple machine operation `{}` for lane `{}`",
            other.cli_subcommand(),
            manifest.lane_id
        ),
    }
}

fn write_membership_revision_receipt(
    manifest: &PsionicTrainInvocationManifest,
    run_root: &Path,
    runtime_identity: &MachineRuntimeIdentity,
    resolved_run_id: &str,
    outcome: PsionicTrainOutcomeKind,
) -> Result<Option<String>, String> {
    let Some(_) = manifest.coordination.node_pubkey.as_ref() else {
        return Ok(None);
    };
    let status_dir = run_root.join("status");
    let current_path = status_dir.join("membership_revision_receipt.json");
    let history_dir = status_dir.join("membership_revisions");
    let previous = if current_path.is_file() {
        Some(load_membership_revision_receipt(current_path.as_path())?)
    } else {
        None
    };
    if previous.is_none() && outcome == PsionicTrainOutcomeKind::Refused {
        return Ok(None);
    }
    fs::create_dir_all(&history_dir).map_err(|error| {
        format!(
            "failed to create membership history directory `{}`: {error}",
            history_dir.display()
        )
    })?;
    let observed_at_ms = current_time_ms()?;
    let receipt = PsionicTrainMembershipRevisionReceipt::next_for_manifest(
        manifest,
        &runtime_identity.attestation,
        &runtime_identity.capability_projection,
        resolved_run_id,
        observed_at_ms,
        outcome,
        previous.as_ref(),
    )?;
    let history_path = history_dir.join(format!(
        "revision-{:06}.json",
        receipt.local_membership_revision
    ));
    let encoded = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to serialize membership receipt: {error}"))?;
    fs::write(&history_path, &encoded).map_err(|error| {
        format!(
            "failed to write membership revision receipt `{}`: {error}",
            history_path.display()
        )
    })?;
    fs::write(&current_path, encoded).map_err(|error| {
        format!(
            "failed to write current membership revision receipt `{}`: {error}",
            current_path.display()
        )
    })?;
    Ok(Some(current_path.display().to_string()))
}

fn write_checkpoint_surface(
    run_root: &Path,
    manifest: &PsionicTrainInvocationManifest,
) -> Result<Option<RetainedCheckpointSurface>, String> {
    let Some(surface) =
        inspect_psionic_train_checkpoint_surface(run_root, manifest.role, manifest.operation)
            .map_err(|error| format!("failed to inspect checkpoint surface: {error}"))?
    else {
        return Ok(None);
    };
    let path = run_root.join("status/checkpoint_surface.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&surface)
            .map_err(|error| format!("failed to serialize checkpoint surface: {error}"))?,
    )
    .map_err(|error| {
        format!(
            "failed to write checkpoint surface `{}`: {error}",
            path.display()
        )
    })?;
    Ok(Some(RetainedCheckpointSurface {
        path: path.display().to_string(),
        surface,
    }))
}

fn load_membership_revision_receipt(
    path: &Path,
) -> Result<PsionicTrainMembershipRevisionReceipt, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "failed to read membership revision receipt `{}`: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "failed to parse membership revision receipt `{}`: {error}",
            path.display()
        )
    })
}

fn current_time_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before UNIX_EPOCH: {error}"))?;
    let millis = duration.as_millis();
    u64::try_from(millis)
        .map_err(|error| format!("current time exceeded u64 milliseconds: {error}"))
}

fn print_usage() {
    eprintln!(
        "Usage:\n  psionic-train manifest --manifest <path>\n  psionic-train sft --config <path>\n  psionic-train dpo --config <path>\n  psionic-train grpo --config <path>\n  psionic-train qwen-legal-rl-rollouts [--runs-root <path>] [--out <dir>]\n  psionic-train qwen-legal-artifact-promotion [--out <dir>]\n  psionic-train qwen-legal-hillclimb --plan <plan.json> [--rung <rung-name>] [--registry <registry.json>] [--feed <feed.json>] [--out <dir>]\n  psionic-train actual-pretraining <operator-args>\n  psionic-train cs336-a1-demo <operator-args>\n\nMachine mode requires a `{}` JSON manifest and emits one `{}` packet on completion.",
        psionic_train::PSIONIC_TRAIN_INVOCATION_MANIFEST_SCHEMA_VERSION,
        psionic_train::PSIONIC_TRAIN_STATUS_PACKET_SCHEMA_VERSION
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_runtime_source_reads_revision_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path()
                .join(PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE),
            "0123456789abcdef\n",
        )
        .expect("write revision marker");

        let source = machine_runtime_source_from_candidates(vec![(
            "test_packaged_runtime".to_string(),
            temp.path().to_path_buf(),
        )])
        .expect("packaged runtime source should resolve");

        assert_eq!(source.repo_root, temp.path().canonicalize().unwrap());
        assert_eq!(
            source.packaged_revision.as_deref(),
            Some("0123456789abcdef")
        );
    }

    #[test]
    fn packaged_runtime_source_rejects_empty_revision_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(
            temp.path()
                .join(PSIONIC_TRAIN_PACKAGED_RUNTIME_REVISION_FILE),
            "\n",
        )
        .expect("write revision marker");

        let error = machine_runtime_source_from_candidates(vec![(
            "test_packaged_runtime".to_string(),
            temp.path().to_path_buf(),
        )])
        .expect_err("empty revision marker should be rejected");

        assert!(error.contains("packaged runtime revision marker"));
        assert!(error.contains("is empty"));
    }
}
