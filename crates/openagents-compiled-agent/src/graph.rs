use crate::{
    GroundedAnswerInput, GroundedAnswerOutput, GroundedAnswerSignature, IntentRouteInput,
    IntentRouteOutput, IntentRouteSignature, ModuleRun, ToolArgumentsInput, ToolArgumentsOutput,
    ToolArgumentsSignature, ToolPolicyInput, ToolPolicyOutput, ToolPolicySignature, TypedModule,
    VerifyInput, VerifyOutput, VerifySignature,
};

/// First narrow compiled-agent graph with phase-separated module slots.
pub struct FirstCompiledAgentGraph<R, P, A, G, V>
where
    R: TypedModule<IntentRouteSignature>,
    P: TypedModule<ToolPolicySignature>,
    A: TypedModule<ToolArgumentsSignature>,
    G: TypedModule<GroundedAnswerSignature>,
    V: TypedModule<VerifySignature>,
{
    /// Route selection slot.
    pub intent_route: R,
    /// Tool exposure slot.
    pub tool_policy: P,
    /// Tool argument slot.
    pub tool_arguments: A,
    /// Grounded synthesis slot.
    pub grounded_answer: G,
    /// Final verification or refusal slot.
    pub verify: V,
}

impl<R, P, A, G, V> FirstCompiledAgentGraph<R, P, A, G, V>
where
    R: TypedModule<IntentRouteSignature>,
    P: TypedModule<ToolPolicySignature>,
    A: TypedModule<ToolArgumentsSignature>,
    G: TypedModule<GroundedAnswerSignature>,
    V: TypedModule<VerifySignature>,
{
    /// Run the route phase.
    pub fn route(&self, input: &IntentRouteInput) -> ModuleRun<IntentRouteOutput> {
        self.intent_route.run(input)
    }

    /// Run the tool policy phase.
    pub fn tool_policy(&self, input: &ToolPolicyInput) -> ModuleRun<ToolPolicyOutput> {
        self.tool_policy.run(input)
    }

    /// Run the tool argument phase.
    pub fn tool_arguments(&self, input: &ToolArgumentsInput) -> ModuleRun<ToolArgumentsOutput> {
        self.tool_arguments.run(input)
    }

    /// Run the grounded answer phase.
    pub fn grounded_answer(
        &self,
        input: &GroundedAnswerInput,
    ) -> ModuleRun<GroundedAnswerOutput> {
        self.grounded_answer.run(input)
    }

    /// Run the verification phase.
    pub fn verify(&self, input: &VerifyInput) -> ModuleRun<VerifyOutput> {
        self.verify.run(input)
    }
}

