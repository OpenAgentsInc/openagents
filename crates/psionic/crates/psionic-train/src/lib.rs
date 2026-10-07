//! Training checkpointing, fixed-budget trainer steps, rollout artifacts, live
//! recovery, elastic membership, and orchestration substrate for Psionic.

#![cfg_attr(
    test,
    allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)
)]

use std::collections::BTreeSet;

use psionic_cluster::{ClusterMembershipStatus, ClusterState, NodeId};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamManifest, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_runtime::{
    RuntimeWorkClass, RuntimeWorkItem, TrainingCheckpointAvailability, TrainingCheckpointReference,
    TrainingElasticMembershipContext, TrainingRecoveryContext, TrainingRecoveryPosture,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

mod a1_minimal_distributed_lm_lane;
mod a1_minimal_distributed_lm_local_update;
mod a1_minimal_distributed_lm_support_artifacts;
mod a1_minimal_distributed_lm_tokenizer_dataset_bundle;
mod a1_minimal_distributed_lm_trusted_aggregation;
mod adapter_aggregation;
mod adapter_artifact_storage;
mod adapter_cluster;
mod adapter_reference_program;
mod adapter_submission_security;
mod adapter_validation;
mod adapter_window;
mod adapter_worker_protocol;
mod apple_adapter;
mod apple_adapter_experiment;
#[cfg(feature = "legacy-apple-toolkit-oracle")]
mod apple_toolkit;
mod artifact_storage;
mod async_checkpoint_writeback;
mod attnres;
mod benchmarking;
mod checkpoint_recovery;
mod compiled_agent_artifact_contract;
mod compiled_agent_decentralized_roles;
mod compiled_agent_external_benchmark;
mod compiled_agent_external_intake;
mod compiled_agent_external_workers;
mod compiled_agent_learning;
mod compiled_agent_phase_six;
mod compiled_agent_receipts;
mod compiled_agent_shadow_governance;
mod compiled_agent_tailnet_pilot;
mod compiled_agent_xtrain;
mod content_addressed_artifact_exchange_contract;
mod contributor_program_lineage;
mod coordinator_candidate_emission;
mod coordinator_conductor;
mod coordinator_eval_verdict_source;
mod coordinator_http_buymode_dispatch;
mod coordinator_live_buymode_dispatch;
mod coordinator_evolution;
mod coordinator_live_training;
mod coordinator_m8_head_to_head;
mod coordinator_shadow_comparison;
mod core_loop;
mod cross_backend_cuda_mlx_dense_mesh;
mod cross_provider_admission_planner;
mod cross_provider_compute_source_contract;
mod cross_provider_launch_contract;
mod cross_provider_program_run_graph;
mod cross_provider_runtime_binder;
mod cross_provider_training_program_manifest;
mod cs336_a1_full_port_conformance;
mod cs336_a1_reference_training;
mod cs336_a2_ddp_bucketed_receipt;
mod cs336_a1_real_gradient_reference;
mod cs336_a3_scaling_reference;
mod cs336_a5_alignment_reference;
mod cs336_a2_ddp_individual_parameters_receipt;
mod cs336_a2_flashattention_fused_cuda_receipt;
mod cs336_a2_flashattention_reference_receipt;
mod cs336_a2_fsdp_after_backward_receipt;
mod cs336_a2_fsdp_full_params_receipt;
mod cs336_a2_fsdp_wrapper_receipt;
mod cs336_a2_full_port_conformance;
mod cs336_a2_profiling;
mod cs336_a2_sharded_optimizer_receipt;
mod curated_decentralized_run_contract;
mod curriculum;
mod decentralized_network_contract;
mod dense_rank_recovery_contract;
mod dense_rank_runtime;
mod dense_topology_revision_contract;
mod distributed_checkpoint_contract;
mod distributed_optimizer;
mod elastic_device_mesh_contract;
mod first_multi_provider_dense_cuda_run;
mod first_same_job_mixed_backend_dense_run;
mod fraud_quarantine_slashing_contract;
mod gemma_e4b_cuda_adapter_sft;
mod gemma_e4b_finetune_eval;
mod gemma_e4b_finetuning_mvp;
mod google_training_binder_projection;
mod grouped_replica_checkpoint;
mod grouped_replica_evidence;
mod grouped_replica_runtime;
mod grouped_replica_transport;
mod hybrid_pretraining_planner;
mod incentivized_decentralized_run_contract;
mod internet_fault_harness_contract;
mod legal_dpo_cli;
mod legal_grpo_cli;
mod legal_sft_cli;
mod live_checkpoint_catchup_contract;
mod local_metric_sink;
mod mixed_backend_checkpoint_contract;
mod mixed_precision;
mod mlx_dense_rank_runtime;
mod model_io;
mod multi_validator_consensus_contract;
mod open_adapter;
mod open_adapter_pgolfish_profile;
mod open_public_decentralized_run_contract;
mod operator_bootstrap_package_contract;
mod optimizer;
mod orchestrator;
mod parameter_golf;
mod parameter_golf_baseline_graph;
mod parameter_golf_benchmark;
mod parameter_golf_campaign;
mod parameter_golf_cuda_coverage;
mod parameter_golf_distributed;
mod parameter_golf_distributed_8xh100_bringup;
mod parameter_golf_distributed_8xh100_runtime_bootstrap;
mod parameter_golf_distributed_8xh100_train_step;
mod parameter_golf_distributed_visualization;
mod parameter_golf_homegolf;
mod parameter_golf_homegolf_accounting;
mod parameter_golf_homegolf_clustered;
mod parameter_golf_homegolf_comparison;
mod parameter_golf_homegolf_competitive_ablation;
mod parameter_golf_homegolf_dense_baseline;
mod parameter_golf_homegolf_manifest;
mod parameter_golf_homegolf_multiseed_package;
mod parameter_golf_homegolf_score_runtime;
mod parameter_golf_homegolf_strict_challenge;
mod parameter_golf_homegolf_visualization;
mod parameter_golf_promoted_promotion;
mod parameter_golf_record_export_surface_contract;
mod parameter_golf_record_folder_compatibility;
mod parameter_golf_record_track;
mod parameter_golf_reference;
mod parameter_golf_same_node_parity;
mod parameter_golf_single_h100_bringup;
mod parameter_golf_single_h100_training;
mod parameter_golf_single_h100_visualization;
mod parameter_golf_submission;
mod parameter_golf_submission_pr;
mod parameter_golf_submission_runtime;
mod parameter_golf_xtrain_visualization;
mod probe_gepa_candidate_manifest;
mod probe_gepa_rollout_coordinator;
mod pylon_launch_promise_gates;
mod psion_acceptance_matrix;
mod psion_actual_pretraining_baseline_tools_bundle;
mod psion_actual_pretraining_checkpoint_evals;
mod psion_actual_pretraining_checkpoint_recovery;
mod psion_actual_pretraining_continuation_alignment;
mod psion_actual_pretraining_continuation_handoff;
mod psion_actual_pretraining_continuation_handoff_rehearsal;
mod psion_actual_pretraining_continue_restart_decisions;
mod psion_actual_pretraining_dashboard;
mod psion_actual_pretraining_data_bundle;
mod psion_actual_pretraining_evidence_contract;
mod psion_actual_pretraining_hardware_qualification;
mod psion_actual_pretraining_lane;
mod psion_actual_pretraining_launcher;
mod psion_actual_pretraining_recipe_bundle;
mod psion_actual_pretraining_run_shape_qualification;
mod psion_actual_pretraining_scaling_bundle;
mod psion_actual_pretraining_status_surface;
mod psion_actual_pretraining_systems_bundle;
mod psion_benchmark_label_generation;
mod psion_benchmark_packages;
mod psion_checkpoint_recovery;
mod psion_cs336_a1_demo_launcher;
mod psion_cs336_a1_demo_operator;
mod psion_decentralized_contribution;
mod psion_executor_4080_decision_grade_run;
mod psion_executor_4080_durable_checkpoint;
mod psion_executor_4080_frequent_eval_attachment;
mod psion_executor_4080_interruption_recovery;
mod psion_executor_4080_remote_launch;
mod psion_executor_4080_smoke_run;
mod psion_executor_admitted_profiles;
mod psion_executor_article_closeout_set;
mod psion_executor_baseline_truth;
mod psion_executor_batch_accumulation_ablation;
mod psion_executor_canonical_mixture;
mod psion_executor_continue_restart_policy;
mod psion_executor_curriculum_boundaries;
mod psion_executor_decision_thresholds;
mod psion_executor_eval_packs;
mod psion_executor_failure_bundle_taxonomy;
mod psion_executor_formatting_audit;
mod psion_executor_hull_cache_benchmark;
mod psion_executor_local_cluster_autoblocks;
mod psion_executor_local_cluster_dashboard;
mod psion_executor_local_cluster_ledger;
mod psion_executor_local_cluster_review_workflow;
mod psion_executor_local_cluster_roundtrip;
mod psion_executor_local_cluster_run_registration;
mod psion_executor_long_run_rehearsal;
mod psion_executor_mac_export_inspection;
mod psion_executor_mandatory_live_metrics;
mod psion_executor_mixture_rollback_policy;
mod psion_executor_mixture_search_cadence;
mod psion_executor_mlx_checkpoint_compatibility;
mod psion_executor_mlx_decision_grade_run;
mod psion_executor_mlx_forward_load_parity;
mod psion_executor_mlx_smoke_run;
mod psion_executor_optimizer_ablation;
mod psion_executor_percepta_closeout_status;
mod psion_executor_phase_two_preflight_checklist;
mod psion_executor_research_branch;
mod psion_executor_scheduler_ablation;
mod psion_executor_source_family_contribution;
mod psion_executor_supervision_density_ablation;
mod psion_executor_tokenizer_architecture_gate;
mod psion_executor_trace_family_weighting_ablation;
mod psion_executor_trace_native_metrics;
mod psion_executor_trained_v1_promotion;
mod psion_executor_trained_v1_replacement_report;
mod psion_executor_unified_throughput_reporting;
mod psion_forge_eval_pack_manifests;
mod psion_google_single_node_visualization;
mod psion_google_two_node_swarm_contract;
mod psion_google_two_node_swarm_runtime;
mod psion_instruct_sft_lane;
mod psion_pilot_pretraining_run;
mod psion_plugin_argument_construction_benchmark;
mod psion_plugin_benchmark_packages;
mod psion_plugin_conditioned_compact_decoder;
mod psion_plugin_conditioned_sft;
mod psion_plugin_discovery_selection_benchmark;
mod psion_plugin_guest_plugin_benchmark;
mod psion_plugin_host_native_accelerated_lane;
mod psion_plugin_host_native_capability_matrix;
mod psion_plugin_host_native_reference_lane;
mod psion_plugin_mixed_capability_matrix;
mod psion_plugin_mixed_reference_lane;
mod psion_plugin_refusal_request_structure_benchmark;
mod psion_plugin_result_interpretation_benchmark;
mod psion_plugin_route_refusal_hardening;
mod psion_plugin_sequencing_benchmark;
mod psion_pretrain_stage;
mod psion_reasoning_sft;
mod psion_reference_pilot;
mod psion_refusal_calibration;
mod psion_rented_cluster_runbook;
mod psion_route_class_evaluation;
mod psion_run_observability;
mod psion_sampling_policy;
mod psion_trusted_cluster_run;
mod public_dataset_authority_contract;
mod public_miner_protocol_contract;
mod public_network_registry_contract;
mod public_run_explorer_contract;
mod public_testnet_readiness_contract;
mod public_work_assignment_contract;
mod qwen_legal_adapter_registry;
mod qwen_legal_adapter_sft;
mod qwen_legal_checkpoint_recovery;
mod qwen_legal_full_artifact_promotion;
mod qwen_legal_ft_command_surface;
mod qwen_legal_hillclimb_controller;
mod qwen_legal_lora_merge;
mod qwen_legal_rl_rollout_batch;
mod qwen36_27b_legal_ft_milestone;
mod qwen36_real_lora_sft;
mod qwen36_real_pylon_rehearsal;
mod qwen38_training_adapter;
mod qwen_legal_distributed_run_milestone;
mod qwen_legal_pylon_network_sft;
mod qwen_legal_pylon_dispatch;
mod qwen_legal_pylon_training_job;
mod qwen_legal_three_task_milestone;
mod quantized_outer_sync_contract;
mod qwen_legal_training_placement;
mod reference_program;
mod reliability;
mod remote_artifact_backend_contract;
mod remote_training_visualization;
mod remote_training_visualization_v2;
mod replay_truth;
mod reward_ledger_contract;
mod rl_artifacts;
mod rl_online_update;
mod rl_run_service;
mod rollout_validation;
mod run_graph;
mod runpod_local_training_binder_projection;
mod sampler_service;
mod scheduling_accounting;
mod security_posture;
mod settlement_publication_contract;
mod shadow_window_ramp;
mod signed_node_identity_contract;
mod sparta_canary;
mod stability;
mod stage_program;
mod studybench_gepa_feedback;
mod swarm_cuda_bringup;
mod swarm_first_closeout;
mod swarm_first_evidence_bundle;
mod swarm_first_live_runtime;
mod swarm_mlx_bringup;
mod swarm_open_adapter;
mod swarm_open_adapter_receipt;
mod swarm_trusted_lan;
mod swarm_trusted_lan_rehearsal;
mod tassadar;
mod tassadar_architecture_bakeoff;
mod tassadar_article_transformer_training;
mod tassadar_article_transformer_weight_production;
mod tassadar_call_frames;
mod tassadar_compiled_distillation;
mod tassadar_conditional_masking_suite;
mod tassadar_cpu_transform_training_receipt;
mod tassadar_default_train_lane;
mod tassadar_default_train_rehearsal;
mod tassadar_error_regime_catalog;
mod tassadar_executor_9x9_promotion;
mod tassadar_executor_9x9_reference_run;
mod tassadar_executor_hull_benchmark;
mod tassadar_executor_hungarian_10x10_article_run;
mod tassadar_executor_hungarian_learned_run;
mod tassadar_executor_postmortem;
mod tassadar_executor_promotion;
mod tassadar_executor_run;
mod tassadar_executor_scale_plan;
mod tassadar_executor_telemetry;
mod tassadar_executor_training;
mod tassadar_executor_windowed_family_comparison;
mod tassadar_learnability_gap;
mod tassadar_learned_call_stack_heap_suite;
mod tassadar_locality_scratchpad_suite;
mod tassadar_memory_abi_v2;
mod tassadar_mixed_trajectory_suite;
mod tassadar_module_state_curriculum;
mod tassadar_module_trace_abi_v2;
mod tassadar_negative_invocation;
mod tassadar_no_hint_self_supervision;
mod tassadar_numeric_encoding_suite;
mod tassadar_pointer_memory_scratchpad;
mod tassadar_program_family_frontier;
mod tassadar_receipt_supervision;
mod tassadar_scratchpad_framework_comparison;
mod tassadar_search_native_executor;
mod tassadar_sequence;
mod tassadar_shared_depth_curriculum;
mod tassadar_shared_primitive_transfer;
mod tassadar_subroutine_supervision;
mod tassadar_supervision_density;
mod tassadar_trace_family_comparison;
mod tassadar_train_launcher;
mod tassadar_verifier_guided_search_trace_family;
mod tassadar_weak_supervision_executor;
mod train_checkpoint;
mod train_checkpoint_handoff;
mod train_membership;
mod train_runtime;
mod train_validator;
mod train_window;
mod training_cpu_budget;
mod training_execution_evidence_bundle;
mod validator_challenge_scoring_contract;
mod validator_promotion_contract;
mod wan_overlay_route_contract;
mod weak_device_accepted_outcome_proof;
mod worker_protocol;
mod xtrain_explorer_artifacts;

pub use a1_minimal_distributed_lm_lane::*;
pub use a1_minimal_distributed_lm_local_update::*;
pub use a1_minimal_distributed_lm_support_artifacts::*;
pub use a1_minimal_distributed_lm_tokenizer_dataset_bundle::*;
pub use a1_minimal_distributed_lm_trusted_aggregation::*;
pub use adapter_aggregation::*;
pub use adapter_artifact_storage::*;
pub use adapter_cluster::*;
pub use adapter_reference_program::*;
pub use adapter_submission_security::*;
pub use adapter_validation::*;
pub use adapter_window::*;
pub use adapter_worker_protocol::*;
pub use apple_adapter::*;
pub use apple_adapter_experiment::*;
#[cfg(feature = "legacy-apple-toolkit-oracle")]
pub use apple_toolkit::*;
pub use artifact_storage::*;
pub use async_checkpoint_writeback::*;
pub use attnres::*;
pub use benchmarking::*;
pub use checkpoint_recovery::*;
pub use compiled_agent_artifact_contract::*;
pub use compiled_agent_decentralized_roles::*;
pub use compiled_agent_external_benchmark::*;
pub use compiled_agent_external_intake::*;
pub use compiled_agent_external_workers::*;
pub use compiled_agent_learning::*;
pub use compiled_agent_phase_six::*;
pub use compiled_agent_receipts::*;
pub use compiled_agent_shadow_governance::*;
pub use compiled_agent_tailnet_pilot::*;
pub use compiled_agent_xtrain::*;
pub use content_addressed_artifact_exchange_contract::*;
pub use contributor_program_lineage::*;
pub use coordinator_candidate_emission::*;
pub use coordinator_conductor::*;
pub use coordinator_eval_verdict_source::*;
pub use coordinator_http_buymode_dispatch::*;
pub use coordinator_live_buymode_dispatch::*;
pub use coordinator_evolution::*;
pub use coordinator_live_training::*;
pub use coordinator_m8_head_to_head::*;
pub use coordinator_shadow_comparison::*;
pub use core_loop::*;
pub use cross_backend_cuda_mlx_dense_mesh::*;
pub use cross_provider_admission_planner::*;
pub use cross_provider_compute_source_contract::*;
pub use cross_provider_launch_contract::*;
pub use cross_provider_program_run_graph::*;
pub use cross_provider_runtime_binder::*;
pub use cross_provider_training_program_manifest::*;
pub use cs336_a1_full_port_conformance::*;
pub use cs336_a1_reference_training::*;
pub use cs336_a1_real_gradient_reference::*;
pub use cs336_a3_scaling_reference::*;
pub use cs336_a5_alignment_reference::*;
pub use cs336_a2_ddp_bucketed_receipt::*;
pub use cs336_a2_ddp_individual_parameters_receipt::*;
pub use cs336_a2_flashattention_fused_cuda_receipt::*;
pub use cs336_a2_flashattention_reference_receipt::*;
pub use cs336_a2_fsdp_after_backward_receipt::*;
pub use cs336_a2_fsdp_full_params_receipt::*;
pub use cs336_a2_fsdp_wrapper_receipt::*;
pub use cs336_a2_full_port_conformance::*;
pub use cs336_a2_profiling::*;
pub use cs336_a2_sharded_optimizer_receipt::*;
pub use curated_decentralized_run_contract::*;
pub use curriculum::*;
pub use decentralized_network_contract::*;
pub use dense_rank_recovery_contract::*;
pub use dense_rank_runtime::*;
pub use dense_topology_revision_contract::*;
pub use distributed_checkpoint_contract::*;
pub use distributed_optimizer::*;
pub use elastic_device_mesh_contract::*;
pub use first_multi_provider_dense_cuda_run::*;
pub use first_same_job_mixed_backend_dense_run::*;
pub use fraud_quarantine_slashing_contract::*;
pub use gemma_e4b_cuda_adapter_sft::*;
pub use gemma_e4b_finetune_eval::*;
pub use gemma_e4b_finetuning_mvp::*;
pub use google_training_binder_projection::*;
pub use grouped_replica_checkpoint::*;
pub use grouped_replica_evidence::*;
pub use grouped_replica_runtime::*;
pub use grouped_replica_transport::*;
pub use hybrid_pretraining_planner::*;
pub use incentivized_decentralized_run_contract::*;
pub use internet_fault_harness_contract::*;
pub use legal_dpo_cli::*;
pub use legal_grpo_cli::*;
pub use legal_sft_cli::*;
pub use live_checkpoint_catchup_contract::*;
pub use local_metric_sink::*;
pub use mixed_backend_checkpoint_contract::*;
pub use mixed_precision::*;
pub use mlx_dense_rank_runtime::*;
pub use model_io::*;
pub use multi_validator_consensus_contract::*;
pub use open_adapter::*;
pub use open_adapter_pgolfish_profile::*;
pub use open_public_decentralized_run_contract::*;
pub use operator_bootstrap_package_contract::*;
pub use optimizer::*;
pub use orchestrator::*;
pub use parameter_golf::*;
pub use parameter_golf_baseline_graph::*;
pub use parameter_golf_benchmark::*;
pub use parameter_golf_campaign::*;
pub use parameter_golf_cuda_coverage::*;
pub use parameter_golf_distributed::*;
pub use parameter_golf_distributed_8xh100_bringup::*;
pub use parameter_golf_distributed_8xh100_runtime_bootstrap::*;
pub use parameter_golf_distributed_8xh100_train_step::*;
pub use parameter_golf_distributed_visualization::*;
pub use parameter_golf_homegolf::*;
pub use parameter_golf_homegolf_accounting::*;
pub use parameter_golf_homegolf_clustered::*;
pub use parameter_golf_homegolf_comparison::*;
pub use parameter_golf_homegolf_competitive_ablation::*;
pub use parameter_golf_homegolf_dense_baseline::*;
pub use parameter_golf_homegolf_manifest::*;
pub use parameter_golf_homegolf_multiseed_package::*;
pub use parameter_golf_homegolf_score_runtime::*;
pub use parameter_golf_homegolf_strict_challenge::*;
pub use parameter_golf_homegolf_visualization::*;
pub use parameter_golf_promoted_promotion::*;
pub use parameter_golf_record_export_surface_contract::*;
pub use parameter_golf_record_folder_compatibility::*;
pub use parameter_golf_record_track::*;
pub use parameter_golf_reference::*;
pub use parameter_golf_same_node_parity::*;
pub use parameter_golf_single_h100_bringup::*;
pub use parameter_golf_single_h100_training::*;
pub use parameter_golf_single_h100_visualization::*;
pub use parameter_golf_submission::*;
pub use parameter_golf_submission_pr::*;
pub use parameter_golf_submission_runtime::*;
pub use parameter_golf_xtrain_visualization::*;
pub use probe_gepa_candidate_manifest::*;
pub use probe_gepa_rollout_coordinator::*;
pub use pylon_launch_promise_gates::*;
pub use psion_acceptance_matrix::*;
pub use psion_actual_pretraining_baseline_tools_bundle::*;
pub use psion_actual_pretraining_checkpoint_evals::*;
pub use psion_actual_pretraining_checkpoint_recovery::*;
pub use psion_actual_pretraining_continuation_alignment::*;
pub use psion_actual_pretraining_continuation_handoff::*;
pub use psion_actual_pretraining_continuation_handoff_rehearsal::*;
pub use psion_actual_pretraining_continue_restart_decisions::*;
pub use psion_actual_pretraining_dashboard::*;
pub use psion_actual_pretraining_data_bundle::*;
pub use psion_actual_pretraining_evidence_contract::*;
pub use psion_actual_pretraining_hardware_qualification::*;
pub use psion_actual_pretraining_lane::*;
pub use psion_actual_pretraining_launcher::*;
pub use psion_actual_pretraining_recipe_bundle::*;
pub use psion_actual_pretraining_run_shape_qualification::*;
pub use psion_actual_pretraining_scaling_bundle::*;
pub use psion_actual_pretraining_status_surface::*;
pub use psion_actual_pretraining_systems_bundle::*;
pub use psion_benchmark_label_generation::*;
pub use psion_benchmark_packages::*;
pub use psion_checkpoint_recovery::*;
pub use psion_cs336_a1_demo_launcher::*;
pub use psion_cs336_a1_demo_operator::*;
pub use psion_decentralized_contribution::*;
pub use psion_executor_4080_decision_grade_run::*;
pub use psion_executor_4080_durable_checkpoint::*;
pub use psion_executor_4080_frequent_eval_attachment::*;
pub use psion_executor_4080_interruption_recovery::*;
pub use psion_executor_4080_remote_launch::*;
pub use psion_executor_4080_smoke_run::*;
pub use psion_executor_admitted_profiles::*;
pub use psion_executor_article_closeout_set::*;
pub use psion_executor_baseline_truth::*;
pub use psion_executor_batch_accumulation_ablation::*;
pub use psion_executor_canonical_mixture::*;
pub use psion_executor_continue_restart_policy::*;
pub use psion_executor_curriculum_boundaries::*;
pub use psion_executor_decision_thresholds::*;
pub use psion_executor_eval_packs::*;
pub use psion_executor_failure_bundle_taxonomy::*;
pub use psion_executor_formatting_audit::*;
pub use psion_executor_hull_cache_benchmark::*;
pub use psion_executor_local_cluster_autoblocks::*;
pub use psion_executor_local_cluster_dashboard::*;
pub use psion_executor_local_cluster_ledger::*;
pub use psion_executor_local_cluster_review_workflow::*;
pub use psion_executor_local_cluster_roundtrip::*;
pub use psion_executor_local_cluster_run_registration::*;
pub use psion_executor_long_run_rehearsal::*;
pub use psion_executor_mac_export_inspection::*;
pub use psion_executor_mandatory_live_metrics::*;
pub use psion_executor_mixture_rollback_policy::*;
pub use psion_executor_mixture_search_cadence::*;
pub use psion_executor_mlx_checkpoint_compatibility::*;
pub use psion_executor_mlx_decision_grade_run::*;
pub use psion_executor_mlx_forward_load_parity::*;
pub use psion_executor_mlx_smoke_run::*;
pub use psion_executor_optimizer_ablation::*;
pub use psion_executor_percepta_closeout_status::*;
pub use psion_executor_phase_two_preflight_checklist::*;
pub use psion_executor_research_branch::*;
pub use psion_executor_scheduler_ablation::*;
pub use psion_executor_source_family_contribution::*;
pub use psion_executor_supervision_density_ablation::*;
pub use psion_executor_tokenizer_architecture_gate::*;
pub use psion_executor_trace_family_weighting_ablation::*;
pub use psion_executor_trace_native_metrics::*;
pub use psion_executor_trained_v1_promotion::*;
pub use psion_executor_trained_v1_replacement_report::*;
pub use psion_executor_unified_throughput_reporting::*;
pub use psion_forge_eval_pack_manifests::*;
pub use psion_google_single_node_visualization::*;
pub use psion_google_two_node_swarm_contract::*;
pub use psion_google_two_node_swarm_runtime::*;
pub use psion_instruct_sft_lane::*;
pub use psion_pilot_pretraining_run::*;
pub use psion_plugin_argument_construction_benchmark::*;
pub use psion_plugin_benchmark_packages::*;
pub use psion_plugin_conditioned_compact_decoder::*;
pub use psion_plugin_conditioned_sft::*;
pub use psion_plugin_discovery_selection_benchmark::*;
pub use psion_plugin_guest_plugin_benchmark::*;
pub use psion_plugin_host_native_accelerated_lane::*;
pub use psion_plugin_host_native_capability_matrix::*;
pub use psion_plugin_host_native_reference_lane::*;
pub use psion_plugin_mixed_capability_matrix::*;
pub use psion_plugin_mixed_reference_lane::*;
pub use psion_plugin_refusal_request_structure_benchmark::*;
pub use psion_plugin_result_interpretation_benchmark::*;
pub use psion_plugin_route_refusal_hardening::*;
pub use psion_plugin_sequencing_benchmark::*;
pub use psion_pretrain_stage::*;
pub use psion_reasoning_sft::*;
pub use psion_reference_pilot::*;
pub use psion_refusal_calibration::*;
pub use psion_rented_cluster_runbook::*;
pub use psion_route_class_evaluation::*;
pub use psion_run_observability::*;
pub use psion_sampling_policy::*;
pub use psion_trusted_cluster_run::*;
pub use public_dataset_authority_contract::*;
pub use public_miner_protocol_contract::*;
pub use public_network_registry_contract::*;
pub use public_run_explorer_contract::*;
pub use public_testnet_readiness_contract::*;
pub use public_work_assignment_contract::*;
pub use qwen_legal_adapter_registry::*;
pub use qwen_legal_adapter_sft::*;
pub use qwen_legal_checkpoint_recovery::*;
pub use qwen_legal_full_artifact_promotion::*;
pub use qwen_legal_ft_command_surface::*;
pub use qwen_legal_hillclimb_controller::*;
pub use qwen_legal_lora_merge::*;
pub use qwen_legal_rl_rollout_batch::*;
pub use qwen36_27b_legal_ft_milestone::*;
pub use qwen36_real_lora_sft::*;
pub use qwen36_real_pylon_rehearsal::*;
pub use qwen38_training_adapter::*;
pub use qwen_legal_distributed_run_milestone::*;
pub use qwen_legal_pylon_network_sft::*;
pub use qwen_legal_pylon_dispatch::*;
pub use qwen_legal_pylon_training_job::*;
pub use qwen_legal_three_task_milestone::*;
pub use quantized_outer_sync_contract::*;
pub use qwen_legal_training_placement::*;
pub use reference_program::*;
pub use reliability::*;
pub use remote_artifact_backend_contract::*;
pub use remote_training_visualization::*;
pub use remote_training_visualization_v2::*;
pub use replay_truth::*;
pub use reward_ledger_contract::*;
pub use rl_artifacts::*;
pub use rl_online_update::*;
pub use rl_run_service::*;
pub use rollout_validation::*;
pub use run_graph::*;
pub use runpod_local_training_binder_projection::*;
pub use sampler_service::*;
pub use scheduling_accounting::*;
pub use security_posture::*;
pub use settlement_publication_contract::*;
pub use shadow_window_ramp::*;
pub use signed_node_identity_contract::*;
pub use sparta_canary::*;
pub use stability::*;
pub use stage_program::*;
pub use studybench_gepa_feedback::*;
pub use swarm_cuda_bringup::*;
pub use swarm_first_closeout::*;
pub use swarm_first_evidence_bundle::*;
pub use swarm_first_live_runtime::*;
pub use swarm_mlx_bringup::*;
pub use swarm_open_adapter::*;
pub use swarm_open_adapter_receipt::*;
pub use swarm_trusted_lan::*;
pub use swarm_trusted_lan_rehearsal::*;
pub use tassadar::*;
pub use tassadar_architecture_bakeoff::*;
pub use tassadar_article_transformer_training::*;
pub use tassadar_article_transformer_weight_production::*;
pub use tassadar_call_frames::*;
pub use tassadar_compiled_distillation::*;
pub use tassadar_conditional_masking_suite::*;
pub use tassadar_cpu_transform_training_receipt::*;
pub use tassadar_default_train_lane::*;
pub use tassadar_default_train_rehearsal::*;
pub use tassadar_error_regime_catalog::*;
pub use tassadar_executor_9x9_promotion::*;
pub use tassadar_executor_9x9_reference_run::*;
pub use tassadar_executor_hull_benchmark::*;
pub use tassadar_executor_hungarian_10x10_article_run::*;
pub use tassadar_executor_hungarian_learned_run::*;
pub use tassadar_executor_postmortem::*;
pub use tassadar_executor_promotion::*;
pub use tassadar_executor_run::*;
pub use tassadar_executor_scale_plan::*;
pub use tassadar_executor_telemetry::*;
pub use tassadar_executor_training::*;
pub use tassadar_executor_windowed_family_comparison::*;
pub use tassadar_learnability_gap::*;
pub use tassadar_learned_call_stack_heap_suite::*;
pub use tassadar_locality_scratchpad_suite::*;
pub use tassadar_memory_abi_v2::*;
pub use tassadar_mixed_trajectory_suite::*;
pub use tassadar_module_state_curriculum::*;
pub use tassadar_module_trace_abi_v2::*;
pub use tassadar_negative_invocation::*;
pub use tassadar_no_hint_self_supervision::*;
pub use tassadar_numeric_encoding_suite::*;
pub use tassadar_pointer_memory_scratchpad::*;
pub use tassadar_program_family_frontier::*;
pub use tassadar_receipt_supervision::*;
pub use tassadar_scratchpad_framework_comparison::*;
pub use tassadar_search_native_executor::*;
pub use tassadar_sequence::*;
pub use tassadar_shared_depth_curriculum::*;
pub use tassadar_shared_primitive_transfer::*;
pub use tassadar_subroutine_supervision::*;
pub use tassadar_supervision_density::*;
pub use tassadar_trace_family_comparison::*;
pub use tassadar_train_launcher::*;
pub use tassadar_verifier_guided_search_trace_family::*;
pub use tassadar_weak_supervision_executor::*;
pub use train_checkpoint::*;
pub use train_checkpoint_handoff::*;
pub use train_membership::*;
pub use train_runtime::*;
pub use train_validator::*;
pub use train_window::*;
pub use training_cpu_budget::*;
pub use training_execution_evidence_bundle::*;
pub use validator_challenge_scoring_contract::*;
pub use validator_promotion_contract::*;
pub use wan_overlay_route_contract::*;
pub use weak_device_accepted_outcome_proof::*;
pub use worker_protocol::*;
pub use xtrain_explorer_artifacts::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "training core, orchestrator, rollout artifacts, scheduling/accounting, reliability, benchmark acceptance, artifact lifecycle, checkpoint, recovery, and elastic membership substrate";

/// Error returned by training-session state transitions.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TrainingSessionError {
    /// The observed cluster does not match the session-owned cluster.
    #[error("training session cluster mismatch: expected {expected}, found {actual}")]
    ClusterMismatch {
        /// Stable cluster ID owned by the session.
        expected: String,
        /// Stable cluster ID observed in cluster state.
        actual: String,
    },
    /// The checkpoint manifest was not checkpoint-scoped.
    #[error("datastream manifest `{stream_id}` is not checkpoint-scoped")]
    ManifestNotCheckpoint {
        /// Stable manifest stream ID.
        stream_id: String,
    },
    /// The checkpoint manifest did not carry checkpoint identity.
    #[error("datastream manifest `{stream_id}` is missing checkpoint binding")]
    CheckpointBindingMissing {
        /// Stable manifest stream ID.
        stream_id: String,
    },
    /// The checkpoint family mismatched the session-owned family.
    #[error("checkpoint family mismatch: expected {expected}, found {actual}")]
    CheckpointFamilyMismatch {
        /// Session-owned family.
        expected: String,
        /// Family surfaced by the manifest.
        actual: String,
    },
    /// One writer node is not currently a cluster member.
    #[error("writer node `{node_id}` is not a known cluster member")]
    UnknownWriterNode {
        /// Stable writer node ID.
        node_id: String,
    },
    /// One writer node is present but not ready for a checkpoint write.
    #[error("writer node `{node_id}` is not ready for checkpoint writes: status `{status}`")]
    WriterNodeNotReady {
        /// Stable writer node ID.
        node_id: String,
        /// Membership status observed for the node.
        status: String,
    },
    /// Another checkpoint write is already active.
    #[error("checkpoint write `{write_id}` is already active")]
    CheckpointWriteInFlight {
        /// Stable active write ID.
        write_id: String,
    },
    /// A durable transition referenced a write that is not active.
    #[error("checkpoint write `{write_id}` is not active")]
    UnknownCheckpointWrite {
        /// Stable write ID.
        write_id: String,
    },
    /// A durable transition used an impossible timestamp.
    #[error("checkpoint durable timestamp {durable_at_ms} is earlier than start {started_at_ms}")]
    DurableTimestampBeforeStart {
        /// Requested durability timestamp.
        durable_at_ms: u64,
        /// Recorded write start timestamp.
        started_at_ms: u64,
    },
    /// One recovery or late-join node is not present in cluster truth.
    #[error("recovery node `{node_id}` is not a known cluster member")]
    UnknownRecoveryNode {
        /// Stable node ID.
        node_id: String,
    },
}

/// Status for one async checkpoint write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsyncCheckpointStatus {
    /// Checkpoint bytes are still flushing.
    Writing,
    /// Checkpoint bytes are durable and may be used for recovery.
    Durable,
}

/// One in-flight or completed async checkpoint write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsyncCheckpointWrite {
    /// Stable write identifier.
    pub write_id: String,
    /// Stable datastream reference for the checkpoint.
    pub manifest: DatastreamManifestRef,
    /// Runtime-visible checkpoint identity.
    pub checkpoint: TrainingCheckpointReference,
    /// Membership facts active when the write began.
    pub elastic_membership: TrainingElasticMembershipContext,
    /// Current write status.
    pub status: AsyncCheckpointStatus,
    /// Low-level runtime work item describing the flush.
    pub flush_work_item: RuntimeWorkItem,
}

impl AsyncCheckpointWrite {
    fn new(
        write_id: String,
        manifest: DatastreamManifestRef,
        checkpoint: TrainingCheckpointReference,
        elastic_membership: TrainingElasticMembershipContext,
    ) -> Self {
        Self {
            write_id,
            flush_work_item: RuntimeWorkItem::new(
                RuntimeWorkClass::CheckpointFlush,
                manifest.chunk_count.max(1),
                manifest.total_bytes,
            ),
            manifest,
            checkpoint,
            elastic_membership,
            status: AsyncCheckpointStatus::Writing,
        }
    }

    fn into_durable(mut self, durable_at_ms: u64) -> Self {
        self.status = AsyncCheckpointStatus::Durable;
        self.checkpoint = self.checkpoint.with_durable_at_ms(durable_at_ms);
        self
    }
}

/// Membership epoch emitted when cluster truth changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingElasticMembershipEpoch {
    /// Current elastic-membership context.
    pub context: TrainingElasticMembershipContext,
    /// Whether this observation changed epoch-relative truth.
    pub changed: bool,
}

/// Explicit action surfaced by one live-recovery plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRecoveryAction {
    /// Resume from the latest durable checkpoint.
    ResumeFromDurableCheckpoint,
    /// Keep the recovering nodes fenced until checkpoint-backed recovery completes.
    FenceRecoveringNodes,
    /// Stage the durable checkpoint for late joiners.
    StageCheckpointForLateJoiners,
    /// Rebalance the effective world size after join/recovery.
    RebalanceWorldSize,
    /// Recovery is blocked until a durable checkpoint exists.
    BlockUntilDurableCheckpoint,
    /// Keep serving current work without topology change.
    ContinueSteadyState,
}

/// Explicit live-recovery plan derived from ordered cluster truth and checkpoint posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingRecoveryPlan {
    /// Stable cluster ID this plan applies to.
    pub cluster_id: String,
    /// Stable checkpoint family this session owns.
    pub checkpoint_family: String,
    /// Stable digest over the plan contents.
    pub plan_digest: String,
    /// Whether checkpoint-backed state transfer is required.
    pub checkpoint_required: bool,
    /// Runtime-visible recovery context for later clustered execution evidence.
    pub recovery_context: TrainingRecoveryContext,
    /// Explicit recovery actions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<TrainingRecoveryAction>,
    /// Durable checkpoint streams required by the plan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoint_streams: Vec<DatastreamManifestRef>,
}

impl TrainingRecoveryPlan {
    fn new(
        cluster_id: impl Into<String>,
        checkpoint_family: impl Into<String>,
        checkpoint_required: bool,
        recovery_context: TrainingRecoveryContext,
        actions: Vec<TrainingRecoveryAction>,
        checkpoint_streams: Vec<DatastreamManifestRef>,
    ) -> Self {
        let cluster_id = cluster_id.into();
        let checkpoint_family = checkpoint_family.into();
        let plan_digest = stable_recovery_plan_digest(
            cluster_id.as_str(),
            checkpoint_family.as_str(),
            checkpoint_required,
            &recovery_context,
            &actions,
            &checkpoint_streams,
        );
        Self {
            cluster_id,
            checkpoint_family,
            plan_digest,
            checkpoint_required,
            recovery_context,
            actions,
            checkpoint_streams,
        }
    }
}

/// Persistent training-session truth for one checkpoint family on one cluster.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingSessionState {
    /// Stable cluster ID owned by this session.
    pub cluster_id: String,
    /// Stable checkpoint family owned by this session.
    pub checkpoint_family: String,
    /// Latest observed elastic-membership epoch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_membership: Option<TrainingElasticMembershipContext>,
    /// Active async checkpoint write when one is in flight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_checkpoint_write: Option<AsyncCheckpointWrite>,
    /// Latest durable checkpoint when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_durable_checkpoint: Option<TrainingCheckpointReference>,
    /// Latest durable checkpoint datastream reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_durable_manifest: Option<DatastreamManifestRef>,
    /// Latest recovery posture derived by the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_recovery_context: Option<TrainingRecoveryContext>,
    next_write_sequence: u64,
}

impl TrainingSessionState {
    /// Creates one training-session state owned by a cluster and checkpoint family.
    #[must_use]
    pub fn new(cluster_id: impl Into<String>, checkpoint_family: impl Into<String>) -> Self {
        Self {
            cluster_id: cluster_id.into(),
            checkpoint_family: checkpoint_family.into(),
            latest_membership: None,
            active_checkpoint_write: None,
            latest_durable_checkpoint: None,
            latest_durable_manifest: None,
            latest_recovery_context: None,
            next_write_sequence: 1,
        }
    }

    /// Returns the latest durable checkpoint when one exists.
    #[must_use]
    pub fn latest_durable_checkpoint(&self) -> Option<&TrainingCheckpointReference> {
        self.latest_durable_checkpoint.as_ref()
    }

    /// Returns the current in-flight checkpoint write when one exists.
    #[must_use]
    pub fn active_checkpoint_write(&self) -> Option<&AsyncCheckpointWrite> {
        self.active_checkpoint_write.as_ref()
    }

    /// Observes authoritative cluster truth and emits an epoch when membership changes.
    pub fn observe_membership(
        &mut self,
        cluster_state: &ClusterState,
    ) -> Result<TrainingElasticMembershipEpoch, TrainingSessionError> {
        ensure_cluster_matches(cluster_state, self.cluster_id.as_str())?;
        let context = elastic_membership_context(cluster_state, self.latest_membership.as_ref());
        let changed = self
            .latest_membership
            .as_ref()
            .is_none_or(|previous| previous != &context);
        self.latest_membership = Some(context.clone());
        Ok(TrainingElasticMembershipEpoch { context, changed })
    }

    /// Begins one async checkpoint write from a checkpoint-scoped datastream manifest.
    pub fn begin_async_checkpoint(
        &mut self,
        cluster_state: &ClusterState,
        manifest: &DatastreamManifest,
        writer_node_id: &NodeId,
        started_at_ms: u64,
    ) -> Result<AsyncCheckpointWrite, TrainingSessionError> {
        if let Some(active) = &self.active_checkpoint_write {
            return Err(TrainingSessionError::CheckpointWriteInFlight {
                write_id: active.write_id.clone(),
            });
        }
        ensure_cluster_matches(cluster_state, self.cluster_id.as_str())?;
        ensure_checkpoint_manifest(manifest, self.checkpoint_family.as_str())?;
        ensure_writer_ready(cluster_state, writer_node_id)?;

        let membership = self.observe_membership(cluster_state)?.context;
        let manifest_ref = manifest.manifest_ref();
        let binding = manifest_ref.checkpoint_binding.clone().ok_or_else(|| {
            TrainingSessionError::CheckpointBindingMissing {
                stream_id: manifest_ref.stream_id.clone(),
            }
        })?;
        let checkpoint = checkpoint_reference_from_manifest(
            &manifest_ref,
            &binding,
            writer_node_id,
            &membership,
            started_at_ms,
        );
        let write_id = format!(
            "{}-checkpoint-write-{}",
            self.checkpoint_family, self.next_write_sequence
        );
        self.next_write_sequence = self.next_write_sequence.saturating_add(1);
        let write =
            AsyncCheckpointWrite::new(write_id, manifest_ref, checkpoint, membership.clone());
        self.latest_recovery_context = Some(
            TrainingRecoveryContext::new(
                TrainingRecoveryPosture::AsyncCheckpointInFlight,
                TrainingCheckpointAvailability::AsyncWriteInFlight,
                membership,
            )
            .with_latest_checkpoint(write.checkpoint.clone())
            .with_detail("checkpoint bytes are flushing asynchronously"),
        );
        self.active_checkpoint_write = Some(write.clone());
        Ok(write)
    }

    /// Marks the current checkpoint write durable and returns the updated recovery context.
    pub fn mark_checkpoint_durable(
        &mut self,
        write_id: &str,
        durable_at_ms: u64,
    ) -> Result<TrainingRecoveryContext, TrainingSessionError> {
        let active = self.active_checkpoint_write.take().ok_or_else(|| {
            TrainingSessionError::UnknownCheckpointWrite {
                write_id: write_id.to_owned(),
            }
        })?;
        if active.write_id != write_id {
            self.active_checkpoint_write = Some(active.clone());
            return Err(TrainingSessionError::UnknownCheckpointWrite {
                write_id: write_id.to_owned(),
            });
        }
        if durable_at_ms < active.checkpoint.started_at_ms {
            let started_at_ms = active.checkpoint.started_at_ms;
            self.active_checkpoint_write = Some(active);
            return Err(TrainingSessionError::DurableTimestampBeforeStart {
                durable_at_ms,
                started_at_ms,
            });
        }

        let durable = active.into_durable(durable_at_ms);
        let membership = durable.elastic_membership.clone();
        self.latest_durable_checkpoint = Some(durable.checkpoint.clone());
        self.latest_durable_manifest = Some(durable.manifest.clone());
        let recovery_context = TrainingRecoveryContext::new(
            TrainingRecoveryPosture::SteadyState,
            TrainingCheckpointAvailability::Durable,
            membership,
        )
        .with_latest_checkpoint(durable.checkpoint.clone())
        .with_detail("latest checkpoint is durable and eligible for recovery");
        self.latest_recovery_context = Some(recovery_context.clone());
        Ok(recovery_context)
    }

    /// Plans explicit live recovery and late-join behavior from current cluster truth.
    pub fn plan_live_recovery(
        &mut self,
        cluster_state: &ClusterState,
        recovering_node_ids: &[NodeId],
        late_joiner_node_ids: &[NodeId],
        requested_at_ms: u64,
    ) -> Result<TrainingRecoveryPlan, TrainingSessionError> {
        let membership = self.observe_membership(cluster_state)?.context;
        let recovering_node_ids = validate_known_nodes(cluster_state, recovering_node_ids)?;
        let late_joiner_node_ids =
            late_joiners_from_membership(cluster_state, late_joiner_node_ids)?;
        let checkpoint_required =
            !recovering_node_ids.is_empty() || !late_joiner_node_ids.is_empty();
        let checkpoint_availability = if self.latest_durable_checkpoint.is_some() {
            TrainingCheckpointAvailability::Durable
        } else if self.active_checkpoint_write.is_some() {
            TrainingCheckpointAvailability::AsyncWriteInFlight
        } else {
            TrainingCheckpointAvailability::None
        };
        let posture = if !recovering_node_ids.is_empty() && !late_joiner_node_ids.is_empty() {
            TrainingRecoveryPosture::ElasticReconfiguration
        } else if !recovering_node_ids.is_empty() {
            TrainingRecoveryPosture::Recovering
        } else if !late_joiner_node_ids.is_empty() {
            TrainingRecoveryPosture::LateJoinPending
        } else if self.active_checkpoint_write.is_some() {
            TrainingRecoveryPosture::AsyncCheckpointInFlight
        } else {
            TrainingRecoveryPosture::SteadyState
        };

        let mut recovery_context =
            TrainingRecoveryContext::new(posture, checkpoint_availability, membership.clone())
                .with_recovering_node_ids(recovering_node_ids.clone())
                .with_late_joiner_node_ids(late_joiner_node_ids.clone())
                .with_requested_at_ms(requested_at_ms);
        let mut actions = Vec::new();
        let mut checkpoint_streams = Vec::new();

        if let Some(checkpoint) = self.latest_durable_checkpoint.clone() {
            recovery_context = recovery_context.with_latest_checkpoint(checkpoint);
            if checkpoint_required {
                actions.push(TrainingRecoveryAction::ResumeFromDurableCheckpoint);
            }
            if let Some(manifest) = self.latest_durable_manifest.clone() {
                checkpoint_streams.push(manifest);
            }
        } else if checkpoint_required {
            actions.push(TrainingRecoveryAction::BlockUntilDurableCheckpoint);
            recovery_context = recovery_context.with_detail(
                "checkpoint-backed recovery is required before recovery or late join can proceed",
            );
        }

        if !recovering_node_ids.is_empty() {
            actions.push(TrainingRecoveryAction::FenceRecoveringNodes);
        }
        if !late_joiner_node_ids.is_empty() {
            if self.latest_durable_checkpoint.is_some() {
                actions.push(TrainingRecoveryAction::StageCheckpointForLateJoiners);
            }
            actions.push(TrainingRecoveryAction::RebalanceWorldSize);
        }
        if actions.is_empty() {
            actions.push(TrainingRecoveryAction::ContinueSteadyState);
            recovery_context =
                recovery_context.with_detail("cluster membership remains steady for this session");
        } else if recovery_context.detail.is_none() {
            recovery_context = recovery_context.with_detail(
                "recovery plan is derived from durable checkpoint and elastic membership truth",
            );
        }

        self.latest_recovery_context = Some(recovery_context.clone());
        Ok(TrainingRecoveryPlan::new(
            self.cluster_id.clone(),
            self.checkpoint_family.clone(),
            checkpoint_required,
            recovery_context,
            actions,
            checkpoint_streams,
        ))
    }
}

fn ensure_cluster_matches(
    cluster_state: &ClusterState,
    expected_cluster_id: &str,
) -> Result<(), TrainingSessionError> {
    let actual = cluster_state.cluster_id().as_str();
    if actual != expected_cluster_id {
        return Err(TrainingSessionError::ClusterMismatch {
            expected: String::from(expected_cluster_id),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn ensure_checkpoint_manifest(
    manifest: &DatastreamManifest,
    expected_checkpoint_family: &str,
) -> Result<(), TrainingSessionError> {
    if manifest.subject != DatastreamSubjectKind::Checkpoint {
        return Err(TrainingSessionError::ManifestNotCheckpoint {
            stream_id: manifest.stream_id.clone(),
        });
    }
    let binding = manifest.checkpoint_binding.as_ref().ok_or_else(|| {
        TrainingSessionError::CheckpointBindingMissing {
            stream_id: manifest.stream_id.clone(),
        }
    })?;
    if binding.checkpoint_family != expected_checkpoint_family {
        return Err(TrainingSessionError::CheckpointFamilyMismatch {
            expected: String::from(expected_checkpoint_family),
            actual: binding.checkpoint_family.clone(),
        });
    }
    Ok(())
}

fn ensure_writer_ready(
    cluster_state: &ClusterState,
    writer_node_id: &NodeId,
) -> Result<(), TrainingSessionError> {
    let Some(record) = cluster_state.memberships().get(writer_node_id) else {
        return Err(TrainingSessionError::UnknownWriterNode {
            node_id: String::from(writer_node_id.as_str()),
        });
    };
    if record.status != ClusterMembershipStatus::Ready {
        return Err(TrainingSessionError::WriterNodeNotReady {
            node_id: String::from(writer_node_id.as_str()),
            status: membership_status_label(record.status),
        });
    }
    Ok(())
}

fn elastic_membership_context(
    cluster_state: &ClusterState,
    previous_context: Option<&TrainingElasticMembershipContext>,
) -> TrainingElasticMembershipContext {
    let mut active_node_ids = Vec::new();
    let mut joining_node_ids = Vec::new();
    let mut draining_node_ids = Vec::new();
    let mut offline_node_ids = Vec::new();

    for membership in cluster_state.memberships().values() {
        match membership.status {
            ClusterMembershipStatus::Ready => {
                active_node_ids.push(String::from(membership.identity.node_id.as_str()));
            }
            ClusterMembershipStatus::Joining => {
                joining_node_ids.push(String::from(membership.identity.node_id.as_str()));
            }
            ClusterMembershipStatus::Draining => {
                draining_node_ids.push(String::from(membership.identity.node_id.as_str()));
            }
            ClusterMembershipStatus::Offline => {
                offline_node_ids.push(String::from(membership.identity.node_id.as_str()));
            }
        }
    }

    let cluster_state_digest = cluster_state.stable_digest();
    let topology_digest = cluster_state.snapshot().topology_digest();
    let active_node_ids = sorted_distinct_strings(active_node_ids);
    let joining_node_ids = sorted_distinct_strings(joining_node_ids);
    let draining_node_ids = sorted_distinct_strings(draining_node_ids);
    let offline_node_ids = sorted_distinct_strings(offline_node_ids);
    let membership_epoch = previous_context.map_or(1, |previous| {
        if previous.cluster_state_digest == cluster_state_digest
            && previous.topology_digest == topology_digest
            && previous.active_node_ids == active_node_ids
            && previous.joining_node_ids == joining_node_ids
            && previous.draining_node_ids == draining_node_ids
            && previous.offline_node_ids == offline_node_ids
        {
            previous.membership_epoch
        } else {
            previous.membership_epoch.saturating_add(1)
        }
    });

    TrainingElasticMembershipContext::new(
        membership_epoch,
        cluster_state_digest,
        topology_digest,
        active_node_ids,
    )
    .with_joining_node_ids(joining_node_ids)
    .with_draining_node_ids(draining_node_ids)
    .with_offline_node_ids(offline_node_ids)
}

fn checkpoint_reference_from_manifest(
    manifest_ref: &DatastreamManifestRef,
    binding: &DatastreamCheckpointBinding,
    writer_node_id: &NodeId,
    membership: &TrainingElasticMembershipContext,
    started_at_ms: u64,
) -> TrainingCheckpointReference {
    let mut checkpoint = TrainingCheckpointReference::new(
        binding.checkpoint_family.clone(),
        manifest_ref.stream_id.clone(),
        manifest_ref.manifest_digest.clone(),
        manifest_ref.object_digest.clone(),
        writer_node_id.as_str(),
        membership.membership_epoch,
        membership.cluster_state_digest.clone(),
        membership.topology_digest.clone(),
        started_at_ms,
    );
    if let Some(checkpoint_ref) = &binding.checkpoint_ref {
        checkpoint = checkpoint.with_checkpoint_ref(checkpoint_ref.clone());
    }
    if let Some(step) = binding.step {
        checkpoint = checkpoint.with_step(step);
    }
    checkpoint
}

fn validate_known_nodes(
    cluster_state: &ClusterState,
    node_ids: &[NodeId],
) -> Result<Vec<String>, TrainingSessionError> {
    let mut resolved = BTreeSet::new();
    for node_id in node_ids {
        if !cluster_state.memberships().contains_key(node_id) {
            return Err(TrainingSessionError::UnknownRecoveryNode {
                node_id: String::from(node_id.as_str()),
            });
        }
        resolved.insert(String::from(node_id.as_str()));
    }
    Ok(resolved.into_iter().collect())
}

fn late_joiners_from_membership(
    cluster_state: &ClusterState,
    explicit_node_ids: &[NodeId],
) -> Result<Vec<String>, TrainingSessionError> {
    let mut late_joiners = BTreeSet::new();
    for membership in cluster_state.memberships().values() {
        if membership.status == ClusterMembershipStatus::Joining {
            late_joiners.insert(String::from(membership.identity.node_id.as_str()));
        }
    }
    for node_id in explicit_node_ids {
        if !cluster_state.memberships().contains_key(node_id) {
            return Err(TrainingSessionError::UnknownRecoveryNode {
                node_id: String::from(node_id.as_str()),
            });
        }
        late_joiners.insert(String::from(node_id.as_str()));
    }
    Ok(late_joiners.into_iter().collect())
}

fn stable_recovery_plan_digest(
    cluster_id: &str,
    checkpoint_family: &str,
    checkpoint_required: bool,
    recovery_context: &TrainingRecoveryContext,
    actions: &[TrainingRecoveryAction],
    checkpoint_streams: &[DatastreamManifestRef],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_train_recovery_plan|");
    hasher.update(cluster_id.as_bytes());
    hasher.update(b"|");
    hasher.update(checkpoint_family.as_bytes());
    hasher.update(b"|");
    hasher.update(if checkpoint_required {
        b"required".as_slice()
    } else {
        b"not_required".as_slice()
    });
    hasher.update(b"|membership_epoch|");
    hasher.update(
        recovery_context
            .elastic_membership
            .membership_epoch
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|cluster_state|");
    hasher.update(
        recovery_context
            .elastic_membership
            .cluster_state_digest
            .as_bytes(),
    );
    hasher.update(b"|topology|");
    hasher.update(
        recovery_context
            .elastic_membership
            .topology_digest
            .as_bytes(),
    );
    hasher.update(b"|posture|");
    hasher.update(training_recovery_posture_label(recovery_context.posture));
    hasher.update(b"|checkpoint_availability|");
    hasher.update(training_checkpoint_availability_label(
        recovery_context.checkpoint_availability,
    ));
    for node_id in &recovery_context.recovering_node_ids {
        hasher.update(b"|recovering|");
        hasher.update(node_id.as_bytes());
    }
    for node_id in &recovery_context.late_joiner_node_ids {
        hasher.update(b"|late_joiner|");
        hasher.update(node_id.as_bytes());
    }
    if let Some(checkpoint) = &recovery_context.latest_checkpoint {
        hasher.update(b"|checkpoint_stream|");
        hasher.update(checkpoint.stream_id.as_bytes());
        hasher.update(b"|manifest|");
        hasher.update(checkpoint.manifest_digest.as_bytes());
        hasher.update(b"|object|");
        hasher.update(checkpoint.object_digest.as_bytes());
    }
    for action in actions {
        hasher.update(b"|action|");
        hasher.update(training_recovery_action_label(*action));
    }
    for stream in checkpoint_streams {
        hasher.update(b"|stream|");
        hasher.update(stream.stream_id.as_bytes());
        hasher.update(b"|");
        hasher.update(stream.manifest_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn membership_status_label(status: ClusterMembershipStatus) -> String {
    String::from(match status {
        ClusterMembershipStatus::Joining => "joining",
        ClusterMembershipStatus::Ready => "ready",
        ClusterMembershipStatus::Draining => "draining",
        ClusterMembershipStatus::Offline => "offline",
    })
}

fn training_recovery_posture_label(posture: TrainingRecoveryPosture) -> &'static [u8] {
    match posture {
        TrainingRecoveryPosture::SteadyState => b"steady_state",
        TrainingRecoveryPosture::LateJoinPending => b"late_join_pending",
        TrainingRecoveryPosture::Recovering => b"recovering",
        TrainingRecoveryPosture::ElasticReconfiguration => b"elastic_reconfiguration",
        TrainingRecoveryPosture::AsyncCheckpointInFlight => b"async_checkpoint_in_flight",
    }
}

fn training_checkpoint_availability_label(
    availability: TrainingCheckpointAvailability,
) -> &'static [u8] {
    match availability {
        TrainingCheckpointAvailability::None => b"none",
        TrainingCheckpointAvailability::AsyncWriteInFlight => b"async_write_in_flight",
        TrainingCheckpointAvailability::Durable => b"durable",
    }
}

fn training_recovery_action_label(action: TrainingRecoveryAction) -> &'static [u8] {
    match action {
        TrainingRecoveryAction::ResumeFromDurableCheckpoint => b"resume_from_durable_checkpoint",
        TrainingRecoveryAction::FenceRecoveringNodes => b"fence_recovering_nodes",
        TrainingRecoveryAction::StageCheckpointForLateJoiners => {
            b"stage_checkpoint_for_late_joiners"
        }
        TrainingRecoveryAction::RebalanceWorldSize => b"rebalance_world_size",
        TrainingRecoveryAction::BlockUntilDurableCheckpoint => b"block_until_durable_checkpoint",
        TrainingRecoveryAction::ContinueSteadyState => b"continue_steady_state",
    }
}

fn sorted_distinct_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use psionic_cluster::{
        AdmissionToken, ClusterId, ClusterMembershipRecord, ClusterMembershipStatus,
        ClusterNamespace, ClusterNodeIdentity, ClusterSnapshot, ClusterState, NodeEpoch, NodeId,
        NodeRole,
    };
    use psionic_datastream::{
        DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamSubjectKind,
    };
    use psionic_runtime::{RuntimeWorkClass, TrainingCheckpointAvailability};

    use super::{
        AsyncCheckpointStatus, TrainingRecoveryAction, TrainingRecoveryPosture,
        TrainingSessionState,
    };

    fn cluster_id() -> ClusterId {
        ClusterId::new(
            &ClusterNamespace::new("train-cluster"),
            &AdmissionToken::new("shared-secret"),
        )
    }

    fn membership(
        cluster_id: &ClusterId,
        node_id: &str,
        status: ClusterMembershipStatus,
    ) -> ClusterMembershipRecord {
        ClusterMembershipRecord::new(
            ClusterNodeIdentity {
                cluster_id: cluster_id.clone(),
                node_id: NodeId::new(node_id),
                node_epoch: NodeEpoch::initial(),
                role: NodeRole::Mixed,
                auth_public_key: format!("{node_id}-pub"),
                attestation: None,
            },
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_000)),
            status,
        )
    }

    fn cluster_state(records: &[(&str, ClusterMembershipStatus)]) -> ClusterState {
        let cluster_id = cluster_id();
        let mut snapshot = ClusterSnapshot::new(cluster_id.clone());
        snapshot.memberships = records
            .iter()
            .map(|(node_id, status)| {
                (
                    NodeId::new(*node_id),
                    membership(&cluster_id, node_id, *status),
                )
            })
            .collect::<BTreeMap<_, _>>();
        ClusterState::from_snapshot(snapshot)
    }

    #[test]
    fn async_checkpoint_write_becomes_durable() -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
        ]);
        let manifest = DatastreamManifest::from_bytes(
            "checkpoint-stream",
            DatastreamSubjectKind::Checkpoint,
            b"checkpoint-bytes",
            4,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("train.decoder")
                .with_checkpoint_ref("step-16")
                .with_step(16),
        );
        let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
        let write =
            session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;

        assert_eq!(write.status, AsyncCheckpointStatus::Writing);
        assert_eq!(
            write.flush_work_item.class,
            RuntimeWorkClass::CheckpointFlush
        );
        assert_eq!(write.flush_work_item.bytes, manifest.total_bytes);

        let recovery = session.mark_checkpoint_durable(write.write_id.as_str(), 1_250)?;

        assert_eq!(recovery.posture, TrainingRecoveryPosture::SteadyState);
        assert_eq!(
            recovery.checkpoint_availability,
            TrainingCheckpointAvailability::Durable
        );
        assert_eq!(
            recovery
                .latest_checkpoint
                .as_ref()
                .and_then(|checkpoint| checkpoint.checkpoint_ref.as_deref()),
            Some("step-16")
        );
        assert_eq!(
            session
                .latest_durable_checkpoint()
                .and_then(|checkpoint| checkpoint.step),
            Some(16)
        );
        assert!(session.active_checkpoint_write().is_none());
        Ok(())
    }

    #[test]
    fn observe_membership_advances_epoch_only_when_truth_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let stable = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
        ]);
        let changed = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
            ("worker-c", ClusterMembershipStatus::Joining),
        ]);
        let mut session = TrainingSessionState::new(stable.cluster_id().as_str(), "train.decoder");

        let first = session.observe_membership(&stable)?;
        let second = session.observe_membership(&stable)?;
        let third = session.observe_membership(&changed)?;

        assert_eq!(first.context.membership_epoch, 1);
        assert!(first.changed);
        assert_eq!(second.context.membership_epoch, 1);
        assert!(!second.changed);
        assert_eq!(third.context.membership_epoch, 2);
        assert!(third.changed);
        assert_eq!(
            third.context.joining_node_ids,
            vec![String::from("worker-c")]
        );
        Ok(())
    }

    #[test]
    fn live_recovery_plan_exposes_recovery_and_late_join_semantics()
    -> Result<(), Box<dyn std::error::Error>> {
        let state = cluster_state(&[
            ("worker-a", ClusterMembershipStatus::Ready),
            ("worker-b", ClusterMembershipStatus::Ready),
            ("worker-c", ClusterMembershipStatus::Joining),
        ]);
        let manifest = DatastreamManifest::from_bytes(
            "checkpoint-stream",
            DatastreamSubjectKind::Checkpoint,
            b"checkpoint-bytes",
            4,
            DatastreamEncoding::Safetensors,
        )
        .with_checkpoint_binding(
            DatastreamCheckpointBinding::new("train.decoder")
                .with_checkpoint_ref("step-32")
                .with_step(32),
        );
        let mut session = TrainingSessionState::new(state.cluster_id().as_str(), "train.decoder");
        let write =
            session.begin_async_checkpoint(&state, &manifest, &NodeId::new("worker-a"), 1_000)?;
        session.mark_checkpoint_durable(write.write_id.as_str(), 1_300)?;

        let plan = session.plan_live_recovery(
            &state,
            &[NodeId::new("worker-b")],
            &[NodeId::new("worker-c")],
            1_500,
        )?;

        assert!(plan.checkpoint_required);
        assert_eq!(
            plan.recovery_context.posture,
            TrainingRecoveryPosture::ElasticReconfiguration
        );
        assert_eq!(
            plan.recovery_context.recovering_node_ids,
            vec![String::from("worker-b")]
        );
        assert_eq!(
            plan.recovery_context.late_joiner_node_ids,
            vec![String::from("worker-c")]
        );
        assert!(
            plan.actions
                .contains(&TrainingRecoveryAction::ResumeFromDurableCheckpoint)
        );
        assert!(
            plan.actions
                .contains(&TrainingRecoveryAction::StageCheckpointForLateJoiners)
        );
        assert!(
            plan.actions
                .contains(&TrainingRecoveryAction::RebalanceWorldSize)
        );
        assert_eq!(plan.checkpoint_streams.len(), 1);
        assert!(!plan.plan_digest.is_empty());
        Ok(())
    }
}
