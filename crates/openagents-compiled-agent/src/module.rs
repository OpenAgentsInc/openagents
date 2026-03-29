use std::fmt::Debug;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CompiledModuleManifest;

/// Typed signature contract for a compiled module slot.
pub trait Signature {
    /// Stable signature name.
    const NAME: &'static str;
    /// Strongly typed input contract.
    type Input: Clone + Debug + PartialEq + Serialize + for<'de> Deserialize<'de>;
    /// Strongly typed output contract.
    type Output: Clone + Debug + PartialEq + Serialize + for<'de> Deserialize<'de>;
}

/// Typed module execution result with machine-visible confidence and trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModuleRun<O> {
    /// Explicit model or compiler output.
    pub output: O,
    /// Confidence claimed by the module.
    pub confidence: f32,
    /// Internal debug trace kept out of user-visible output.
    pub trace: Value,
}

impl<O> ModuleRun<O> {
    /// Build a new module run value.
    #[must_use]
    pub fn new(output: O, confidence: f32, trace: Value) -> Self {
        Self {
            output,
            confidence,
            trace,
        }
    }
}

/// Executable typed compiled module.
pub trait TypedModule<S: Signature>: Send + Sync {
    /// Stable manifest for the module artifact.
    fn manifest(&self) -> &CompiledModuleManifest;

    /// Run the module against its typed input.
    fn run(&self, input: &S::Input) -> ModuleRun<S::Output>;
}

impl<S, T> TypedModule<S> for Arc<T>
where
    S: Signature,
    T: TypedModule<S> + ?Sized,
{
    fn manifest(&self) -> &CompiledModuleManifest {
        self.as_ref().manifest()
    }

    fn run(&self, input: &S::Input) -> ModuleRun<S::Output> {
        self.as_ref().run(input)
    }
}

