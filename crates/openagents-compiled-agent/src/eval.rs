use serde::{Deserialize, Serialize};
use serde_json::to_value;

use crate::{Signature, TypedModule};

/// Single module evaluation case.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleEvalCase<S: Signature> {
    /// Stable case identifier.
    pub case_id: String,
    /// Typed module input.
    pub input: S::Input,
    /// Expected typed output.
    pub expected: S::Output,
}

/// Recorded module evaluation outcome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleEvalRecord<S: Signature> {
    /// Stable case identifier.
    pub case_id: String,
    /// Executed manifest identifier.
    pub manifest_id: String,
    /// Whether the judge accepted the module output.
    pub passed: bool,
    /// Claimed module confidence.
    pub confidence: f32,
    /// Serialized actual output for replayable inspection.
    pub actual_output: serde_json::Value,
    /// Serialized expected output for replayable inspection.
    pub expected_output: serde_json::Value,
    /// Serialized trace for inspection.
    pub trace: serde_json::Value,
    /// Type witness for the signature carried through serde.
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<S>,
}

/// Evaluate a single module case with a caller-provided judge.
pub fn evaluate_module_case<S, M, F>(
    module: &M,
    case: &ModuleEvalCase<S>,
    judge: F,
) -> ModuleEvalRecord<S>
where
    S: Signature,
    M: TypedModule<S>,
    F: FnOnce(&S::Output, &S::Output) -> bool,
{
    let run = module.run(&case.input);
    let passed = judge(&run.output, &case.expected);
    ModuleEvalRecord {
        case_id: case.case_id.clone(),
        manifest_id: module.manifest().manifest_id(),
        passed,
        confidence: run.confidence,
        actual_output: to_value(&run.output).unwrap_or_else(|_| serde_json::Value::Null),
        expected_output: to_value(&case.expected).unwrap_or_else(|_| serde_json::Value::Null),
        trace: run.trace,
        _marker: std::marker::PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        AgentRoute, CompiledModuleManifest, IntentRouteInput, IntentRouteOutput,
        IntentRouteSignature, ModulePromotionState, ModuleRun, TypedModule,
    };

    use super::{ModuleEvalCase, evaluate_module_case};

    struct DemoRouteModule {
        manifest: CompiledModuleManifest,
    }

    impl TypedModule<IntentRouteSignature> for DemoRouteModule {
        fn manifest(&self) -> &CompiledModuleManifest {
            &self.manifest
        }

        fn run(&self, input: &IntentRouteInput) -> ModuleRun<IntentRouteOutput> {
            let route = if input.user_request.contains("wallet") {
                AgentRoute::WalletStatus
            } else {
                AgentRoute::ProviderStatus
            };
            ModuleRun::new(
                IntentRouteOutput {
                    route,
                    rationale: "demo".to_string(),
                },
                0.91,
                json!({"module":"demo_route"}),
            )
        }
    }

    #[test]
    fn evaluate_module_case_records_manifest_and_output() {
        let module = DemoRouteModule {
            manifest: CompiledModuleManifest {
                module_name: "intent_route".to_string(),
                signature_name: "intent_route".to_string(),
                implementation_family: "rule_v1".to_string(),
                implementation_label: "demo".to_string(),
                version: "2026-03-28".to_string(),
                promotion_state: ModulePromotionState::Promoted,
                confidence_floor: 0.8,
            },
        };
        let case = ModuleEvalCase::<IntentRouteSignature> {
            case_id: "wallet".to_string(),
            input: IntentRouteInput {
                user_request: "show wallet status".to_string(),
            },
            expected: IntentRouteOutput {
                route: AgentRoute::WalletStatus,
                rationale: "expected".to_string(),
            },
        };

        let record = evaluate_module_case(&module, &case, |actual, expected| {
            actual.route == expected.route
        });

        assert!(record.passed);
        assert_eq!(
            record.manifest_id,
            "intent_route:rule_v1:demo:2026-03-28".to_string()
        );
        assert_eq!(record.confidence, 0.91);
    }
}
