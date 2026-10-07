use psionic_ir::{
    TassadarLocalityScratchpadPass, TassadarLocalityScratchpadTraceFamily,
    TassadarScratchpadEncoding, TassadarScratchpadFormatConfig,
    TassadarScratchpadFormattedSequence, TassadarSymbolicBinaryOp, TassadarSymbolicExpr,
    TassadarSymbolicOperand, TassadarSymbolicStatement, format_tassadar_sequence_with_scratchpad,
    tassadar_locality_preserving_scratchpad_passes, tassadar_symbolic_program_examples,
};
use psionic_runtime::{
    TassadarHostImportStubKind, TassadarModuleExecutionError, TassadarModuleFrameTransition,
    TassadarModuleTraceEvent, TassadarModuleTraceV2Artifact, TassadarStructuredControlBinaryOp,
    build_tassadar_module_trace_v2_artifact, execute_tassadar_module_execution_program,
    tassadar_seeded_module_call_indirect_program,
    tassadar_seeded_module_deterministic_import_program,
    tassadar_seeded_module_global_state_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// One same-program baseline-vs-candidate compilation under the locality-preserving scratchpad pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadCompilation {
    /// Stable case identifier.
    pub case_id: String,
    /// Trace family covered by the compilation.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Stable source trace reference.
    pub source_trace_ref: String,
    /// Stable source trace digest.
    pub source_trace_digest: String,
    /// Stable pass contract applied to the case.
    pub pass: TassadarLocalityScratchpadPass,
    /// Prompt/header tokens kept outside the replayed trace body.
    pub prompt_tokens: Vec<String>,
    /// Replay-authoritative trace tokens carried by the pass.
    pub source_trace_tokens: Vec<String>,
    /// Baseline no-scratchpad formatting.
    pub baseline_sequence: TassadarScratchpadFormattedSequence,
    /// Candidate scratchpad formatting.
    pub candidate_sequence: TassadarScratchpadFormattedSequence,
    /// Stable compilation digest.
    pub compilation_digest: String,
}

impl TassadarLocalityScratchpadCompilation {
    fn new(
        case_id: impl Into<String>,
        trace_family: TassadarLocalityScratchpadTraceFamily,
        source_trace_ref: impl Into<String>,
        source_trace_digest: impl Into<String>,
        pass: TassadarLocalityScratchpadPass,
        prompt_tokens: Vec<String>,
        source_trace_tokens: Vec<String>,
    ) -> Result<Self, TassadarLocalityScratchpadCompileError> {
        let baseline_format = TassadarScratchpadFormatConfig::new(
            TassadarScratchpadEncoding::FlatTrace,
            psionic_ir::TassadarControlledPositionScheme::AbsoluteMonotonic,
            pass.format.scratchpad_segment_token_cap,
        );
        let baseline_sequence = format_tassadar_sequence_with_scratchpad(
            prompt_tokens.as_slice(),
            source_trace_tokens.as_slice(),
            &baseline_format,
        );
        let candidate_sequence = format_tassadar_sequence_with_scratchpad(
            prompt_tokens.as_slice(),
            source_trace_tokens.as_slice(),
            &pass.format,
        );
        let overhead_bps = scratchpad_overhead_bps(&baseline_sequence, &candidate_sequence);
        let case_id = case_id.into();
        if overhead_bps > pass.max_inserted_token_overhead_bps {
            return Err(
                TassadarLocalityScratchpadCompileError::ScratchpadOverheadBudgetExceeded {
                    case_id,
                    pass_id: pass.pass_id,
                    observed_bps: overhead_bps,
                    budget_bps: pass.max_inserted_token_overhead_bps,
                },
            );
        }

        let mut compilation = Self {
            case_id,
            trace_family,
            source_trace_ref: source_trace_ref.into(),
            source_trace_digest: source_trace_digest.into(),
            pass,
            prompt_tokens,
            source_trace_tokens,
            baseline_sequence,
            candidate_sequence,
            compilation_digest: String::new(),
        };
        compilation.compilation_digest = stable_digest(
            b"psionic_tassadar_locality_scratchpad_compilation|",
            &compilation,
        );
        Ok(compilation)
    }
}

/// Compilation failure for the locality-preserving scratchpad pass.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarLocalityScratchpadCompileError {
    /// One supported trace family was missing its pass contract.
    #[error("missing locality-preserving scratchpad pass for trace family `{trace_family:?}`")]
    MissingPassForTraceFamily {
        /// Trace family that lacked a pass.
        trace_family: TassadarLocalityScratchpadTraceFamily,
    },
    /// One candidate expansion exceeded the declared overhead budget.
    #[error(
        "locality-preserving scratchpad pass `{pass_id}` exceeded the overhead budget for `{case_id}`: observed={observed_bps}bps budget={budget_bps}bps"
    )]
    ScratchpadOverheadBudgetExceeded {
        /// Stable case identifier.
        case_id: String,
        /// Stable pass identifier.
        pass_id: String,
        /// Observed overhead.
        observed_bps: u32,
        /// Declared budget.
        budget_bps: u32,
    },
    /// Module execution or trace materialization failed.
    #[error(transparent)]
    Runtime(#[from] TassadarModuleExecutionError),
}

/// Builds the seeded same-program locality-preserving scratchpad compilations.
pub fn compile_tassadar_locality_preserving_scratchpad_cases()
-> Result<Vec<TassadarLocalityScratchpadCompilation>, TassadarLocalityScratchpadCompileError> {
    let passes = tassadar_locality_preserving_scratchpad_passes();
    let symbolic_pass = pass_for_family(
        passes.as_slice(),
        TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine,
    )?;
    let module_pass = pass_for_family(
        passes.as_slice(),
        TassadarLocalityScratchpadTraceFamily::ModuleTraceV2,
    )?;

    let mut cases = Vec::new();
    for example in tassadar_symbolic_program_examples() {
        cases.push(TassadarLocalityScratchpadCompilation::new(
            example.case_id,
            TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine,
            format!("symbolic://{}", example.program.program_id),
            example.program.stable_digest(),
            symbolic_pass.clone(),
            vec![
                String::from("<symbolic_program>"),
                example.program.program_id,
            ],
            symbolic_trace_tokens(example.program.statements.as_slice()),
        )?);
    }

    for (case_id, source_trace_ref, source_trace_digest, prompt_tokens, trace_tokens) in
        module_trace_cases()?
    {
        cases.push(TassadarLocalityScratchpadCompilation::new(
            case_id,
            TassadarLocalityScratchpadTraceFamily::ModuleTraceV2,
            source_trace_ref,
            source_trace_digest,
            module_pass.clone(),
            prompt_tokens,
            trace_tokens,
        )?);
    }

    Ok(cases)
}

fn pass_for_family(
    passes: &[TassadarLocalityScratchpadPass],
    trace_family: TassadarLocalityScratchpadTraceFamily,
) -> Result<TassadarLocalityScratchpadPass, TassadarLocalityScratchpadCompileError> {
    passes
        .iter()
        .find(|pass| pass.trace_family == trace_family)
        .cloned()
        .ok_or(TassadarLocalityScratchpadCompileError::MissingPassForTraceFamily { trace_family })
}

fn module_trace_cases() -> Result<
    Vec<(String, String, String, Vec<String>, Vec<String>)>,
    TassadarLocalityScratchpadCompileError,
> {
    let cases = vec![
        (
            "module_global_state_trace",
            tassadar_seeded_module_global_state_program(),
        ),
        (
            "module_call_indirect_trace",
            tassadar_seeded_module_call_indirect_program(),
        ),
        (
            "module_deterministic_import_trace",
            tassadar_seeded_module_deterministic_import_program(),
        ),
    ];

    let mut compiled_cases = Vec::with_capacity(cases.len());
    for (case_id, program) in cases {
        let execution = execute_tassadar_module_execution_program(&program)?;
        let artifact = build_tassadar_module_trace_v2_artifact(
            format!("tassadar://module_trace_v2/locality/{case_id}"),
            &program,
            &execution,
        )?;
        compiled_cases.push((
            String::from(case_id),
            format!("module_trace_v2://{}", program.program_id),
            artifact.artifact_digest.clone(),
            vec![
                String::from("<module_trace_v2>"),
                program.program_id.clone(),
                artifact.trace_abi_id.clone(),
            ],
            module_trace_tokens(&artifact),
        ));
    }
    Ok(compiled_cases)
}

fn symbolic_trace_tokens(statements: &[TassadarSymbolicStatement]) -> Vec<String> {
    statements
        .iter()
        .flat_map(|statement| match statement {
            TassadarSymbolicStatement::Let { name, expr } => {
                let mut tokens = vec![String::from("let"), name.clone()];
                match expr {
                    TassadarSymbolicExpr::Operand { operand } => {
                        tokens.push(String::from("operand"));
                        tokens.extend(symbolic_operand_tokens(operand));
                    }
                    TassadarSymbolicExpr::Binary { op, left, right } => {
                        tokens.push(symbolic_binary_op_label(*op).to_string());
                        tokens.extend(symbolic_operand_tokens(left));
                        tokens.extend(symbolic_operand_tokens(right));
                    }
                }
                tokens
            }
            TassadarSymbolicStatement::Store { slot, value } => {
                let mut tokens = vec![String::from("store"), format!("slot_{slot}")];
                tokens.extend(symbolic_operand_tokens(value));
                tokens
            }
            TassadarSymbolicStatement::Output { value } => {
                let mut tokens = vec![String::from("output")];
                tokens.extend(symbolic_operand_tokens(value));
                tokens
            }
        })
        .collect()
}

fn symbolic_operand_tokens(operand: &TassadarSymbolicOperand) -> Vec<String> {
    match operand {
        TassadarSymbolicOperand::Name { name } => vec![String::from("name"), name.clone()],
        TassadarSymbolicOperand::Const { value } => vec![String::from("const"), value.to_string()],
        TassadarSymbolicOperand::MemorySlot { slot } => {
            vec![String::from("memory_slot"), slot.to_string()]
        }
    }
}

fn symbolic_binary_op_label(op: TassadarSymbolicBinaryOp) -> &'static str {
    match op {
        TassadarSymbolicBinaryOp::Add => "add",
        TassadarSymbolicBinaryOp::Sub => "sub",
        TassadarSymbolicBinaryOp::Mul => "mul",
        TassadarSymbolicBinaryOp::Lt => "lt",
    }
}

fn module_trace_tokens(artifact: &TassadarModuleTraceV2Artifact) -> Vec<String> {
    let mut tokens = Vec::new();
    for step in &artifact.steps {
        tokens.push(format!("step_{}", step.step_index));
        tokens.extend(module_trace_event_tokens(&step.event));
        match &step.frame_transition {
            TassadarModuleFrameTransition::None => tokens.push(String::from("frame_none")),
            TassadarModuleFrameTransition::Enter {
                caller_function_index,
                function_index,
                function_name,
            } => {
                tokens.push(String::from("frame_enter"));
                if let Some(caller_function_index) = caller_function_index {
                    tokens.push(format!("caller_{caller_function_index}"));
                }
                tokens.push(format!("fn_{function_index}"));
                tokens.push(function_name.clone());
            }
            TassadarModuleFrameTransition::Exit {
                function_index,
                function_name,
                returned_to_function_index,
            } => {
                tokens.push(String::from("frame_exit"));
                tokens.push(format!("fn_{function_index}"));
                tokens.push(function_name.clone());
                if let Some(returned_to_function_index) = returned_to_function_index {
                    tokens.push(format!("return_to_{returned_to_function_index}"));
                }
            }
        }
        for delta in &step.local_deltas {
            tokens.push(format!(
                "local_{}_{}_{}",
                delta.function_index, delta.local_index, delta.after
            ));
        }
        for delta in &step.global_deltas {
            tokens.push(format!("global_{}_{}", delta.global_index, delta.after));
        }
    }
    tokens
}

fn module_trace_event_tokens(event: &TassadarModuleTraceEvent) -> Vec<String> {
    match event {
        TassadarModuleTraceEvent::ElementSegmentApplied {
            element_segment_index,
            table_index,
            offset,
            ..
        } => vec![
            String::from("element_segment_applied"),
            element_segment_index.to_string(),
            table_index.to_string(),
            offset.to_string(),
        ],
        TassadarModuleTraceEvent::DataSegmentApplied {
            data_segment_index,
            memory_index,
            offset,
            byte_len,
        } => vec![
            String::from("data_segment_applied"),
            data_segment_index.to_string(),
            memory_index.to_string(),
            offset.to_string(),
            byte_len.to_string(),
        ],
        TassadarModuleTraceEvent::ConstPush { value } => {
            vec![String::from("const_push"), value.to_string()]
        }
        TassadarModuleTraceEvent::LocalGet { local_index, value } => {
            vec![
                String::from("local_get"),
                local_index.to_string(),
                value.to_string(),
            ]
        }
        TassadarModuleTraceEvent::LocalSet { local_index, value } => {
            vec![
                String::from("local_set"),
                local_index.to_string(),
                value.to_string(),
            ]
        }
        TassadarModuleTraceEvent::GlobalGet {
            global_index,
            value,
        } => {
            vec![
                String::from("global_get"),
                global_index.to_string(),
                value.to_string(),
            ]
        }
        TassadarModuleTraceEvent::GlobalSet {
            global_index,
            value,
        } => {
            vec![
                String::from("global_set"),
                global_index.to_string(),
                value.to_string(),
            ]
        }
        TassadarModuleTraceEvent::Drop { value } => {
            vec![String::from("drop"), value.to_string()]
        }
        TassadarModuleTraceEvent::BinaryOp {
            op,
            left,
            right,
            result,
        } => vec![
            String::from("binary_op"),
            structured_control_binary_op_label(*op).to_string(),
            left.to_string(),
            right.to_string(),
            result.to_string(),
        ],
        TassadarModuleTraceEvent::Load {
            memory_index,
            address,
            offset,
            value,
            ..
        } => vec![
            String::from("load"),
            memory_index.to_string(),
            address.to_string(),
            offset.to_string(),
            value.to_string(),
        ],
        TassadarModuleTraceEvent::Store {
            memory_index,
            address,
            offset,
            value,
            ..
        } => vec![
            String::from("store"),
            memory_index.to_string(),
            address.to_string(),
            offset.to_string(),
            value.to_string(),
        ],
        TassadarModuleTraceEvent::MemorySize {
            memory_index,
            pages,
        } => vec![
            String::from("memory_size"),
            memory_index.to_string(),
            pages.to_string(),
        ],
        TassadarModuleTraceEvent::MemoryGrow {
            memory_index,
            requested_pages,
            previous_pages,
            result,
        } => vec![
            String::from("memory_grow"),
            memory_index.to_string(),
            requested_pages.to_string(),
            previous_pages.to_string(),
            result.to_string(),
        ],
        TassadarModuleTraceEvent::MemoryCopy {
            dst_memory_index,
            src_memory_index,
            dst_address,
            src_address,
            byte_len,
        } => vec![
            String::from("memory_copy"),
            dst_memory_index.to_string(),
            src_memory_index.to_string(),
            dst_address.to_string(),
            src_address.to_string(),
            byte_len.to_string(),
        ],
        TassadarModuleTraceEvent::MemoryFill {
            memory_index,
            address,
            value,
            byte_len,
        } => vec![
            String::from("memory_fill"),
            memory_index.to_string(),
            address.to_string(),
            value.to_string(),
            byte_len.to_string(),
        ],
        TassadarModuleTraceEvent::Call { function_index } => {
            vec![String::from("call"), function_index.to_string()]
        }
        TassadarModuleTraceEvent::CallIndirect {
            table_index,
            selector,
            function_index,
        } => vec![
            String::from("call_indirect"),
            table_index.to_string(),
            selector.to_string(),
            function_index.to_string(),
        ],
        TassadarModuleTraceEvent::HostCall {
            import_ref,
            stub_kind,
            result,
        } => {
            let mut tokens = vec![
                String::from("host_call"),
                import_ref.clone(),
                host_import_stub_kind_label(*stub_kind).to_string(),
            ];
            if let Some(result) = result {
                tokens.push(result.to_string());
            }
            tokens
        }
        TassadarModuleTraceEvent::Return {
            function_index,
            value,
            implicit,
        } => {
            let mut tokens = vec![
                String::from("return"),
                function_index.to_string(),
                if *implicit {
                    String::from("implicit")
                } else {
                    String::from("explicit")
                },
            ];
            if let Some(value) = value {
                tokens.push(value.to_string());
            }
            tokens
        }
    }
}

fn structured_control_binary_op_label(op: TassadarStructuredControlBinaryOp) -> &'static str {
    match op {
        TassadarStructuredControlBinaryOp::Add => "add",
        TassadarStructuredControlBinaryOp::Sub => "sub",
        TassadarStructuredControlBinaryOp::Mul => "mul",
        TassadarStructuredControlBinaryOp::Eq => "eq",
        TassadarStructuredControlBinaryOp::Ne => "ne",
        TassadarStructuredControlBinaryOp::LtS => "lt_s",
        TassadarStructuredControlBinaryOp::LtU => "lt_u",
        TassadarStructuredControlBinaryOp::GtS => "gt_s",
        TassadarStructuredControlBinaryOp::GtU => "gt_u",
        TassadarStructuredControlBinaryOp::LeS => "le_s",
        TassadarStructuredControlBinaryOp::LeU => "le_u",
        TassadarStructuredControlBinaryOp::GeS => "ge_s",
        TassadarStructuredControlBinaryOp::GeU => "ge_u",
        TassadarStructuredControlBinaryOp::And => "and",
        TassadarStructuredControlBinaryOp::Or => "or",
        TassadarStructuredControlBinaryOp::Xor => "xor",
        TassadarStructuredControlBinaryOp::Shl => "shl",
        TassadarStructuredControlBinaryOp::ShrS => "shr_s",
        TassadarStructuredControlBinaryOp::ShrU => "shr_u",
    }
}

fn host_import_stub_kind_label(stub_kind: TassadarHostImportStubKind) -> &'static str {
    match stub_kind {
        TassadarHostImportStubKind::DeterministicI32Const => "deterministic_i32_const",
        TassadarHostImportStubKind::UnsupportedHostCall => "unsupported_host_call",
    }
}

fn scratchpad_overhead_bps(
    baseline: &TassadarScratchpadFormattedSequence,
    candidate: &TassadarScratchpadFormattedSequence,
) -> u32 {
    if baseline.tokens.is_empty() {
        0
    } else {
        (((candidate.tokens.len().saturating_sub(baseline.tokens.len())) as u64) * 10_000
            / baseline.tokens.len() as u64) as u32
    }
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("locality-preserving scratchpad compilation should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_ir::TassadarLocalityScratchpadTraceFamily;

    use super::compile_tassadar_locality_preserving_scratchpad_cases;

    #[test]
    fn locality_scratchpad_compilations_cover_symbolic_and_module_trace_families() {
        let cases = compile_tassadar_locality_preserving_scratchpad_cases()
            .expect("locality-preserving scratchpad cases should compile");
        assert!(
            cases.iter().any(|case| case.trace_family
                == TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine)
        );
        assert!(
            cases
                .iter()
                .any(|case| case.trace_family
                    == TassadarLocalityScratchpadTraceFamily::ModuleTraceV2)
        );
        assert!(cases.iter().all(|case| !case.compilation_digest.is_empty()));
    }
}
