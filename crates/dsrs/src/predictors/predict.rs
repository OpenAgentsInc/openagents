use indexmap::IndexMap;
use rig::tool::ToolDyn;
use std::sync::Arc;
use uuid::Uuid;

use crate::core::{MetaSignature, Optimizable, get_callback};
use crate::{ChatAdapter, Example, GLOBAL_SETTINGS, LM, Prediction, adapter::Adapter};

pub struct Predict {
    pub signature: Arc<dyn MetaSignature>,
    pub tools: Vec<Arc<dyn ToolDyn>>,
}

impl Predict {
    pub fn new(signature: impl MetaSignature + 'static) -> Self {
        Self {
            signature: Arc::new(signature),
            tools: vec![],
        }
    }

    pub fn new_with_tools(
        signature: impl MetaSignature + 'static,
        tools: Vec<Box<dyn ToolDyn>>,
    ) -> Self {
        Self {
            signature: Arc::new(signature),
            tools: tools.into_iter().map(Arc::from).collect(),
        }
    }

    pub fn with_tools(mut self, tools: Vec<Box<dyn ToolDyn>>) -> Self {
        self.tools = tools.into_iter().map(Arc::from).collect();
        self
    }

    pub fn add_tool(mut self, tool: Box<dyn ToolDyn>) -> Self {
        self.tools.push(Arc::from(tool));
        self
    }
}

impl super::Predictor for Predict {
    async fn forward(&self, inputs: Example) -> anyhow::Result<Prediction> {
        // Generate unique call ID for callbacks
        let call_id = Uuid::new_v4();
        let callback = get_callback();

        // Emit module start callback
        callback.on_module_start(call_id, "Predict", &inputs);

        let trace_node_id = if crate::trace::is_tracing() {
            let input_id = if let Some(id) = inputs.node_id {
                id
            } else {
                crate::trace::record_node(
                    crate::trace::NodeType::Root,
                    vec![],
                    Some(inputs.clone()),
                )
                .unwrap_or(0)
            };

            crate::trace::record_node(
                crate::trace::NodeType::Predict {
                    signature_name: "Predict".to_string(),
                    signature: self.signature.clone(),
                },
                vec![input_id],
                None,
            )
        } else {
            None
        };

        let (adapter, lm) = {
            let guard = GLOBAL_SETTINGS.read().unwrap();
            let settings = guard.as_ref().unwrap();
            (settings.adapter.clone(), Arc::clone(&settings.lm))
        }; // guard is dropped here

        let result = adapter
            .call(lm, self.signature.as_ref(), inputs, self.tools.clone())
            .await;

        // Handle result and emit callbacks
        match result {
            Ok(mut prediction) => {
                if let Some(id) = trace_node_id {
                    prediction.node_id = Some(id);
                    crate::trace::record_output(id, prediction.clone());
                }

                // Emit module end callback (success)
                callback.on_module_end(call_id, Ok(&prediction));

                Ok(prediction)
            }
            Err(e) => {
                // Emit module end callback (error)
                callback.on_module_end(call_id, Err(&e));
                Err(e)
            }
        }
    }

    async fn forward_with_config(
        &self,
        inputs: Example,
        lm: Arc<LM>,
    ) -> anyhow::Result<Prediction> {
        // Generate unique call ID for callbacks
        let call_id = Uuid::new_v4();
        let callback = get_callback();

        // Emit module start callback
        callback.on_module_start(call_id, "Predict", &inputs);

        let trace_node_id = if crate::trace::is_tracing() {
            let input_id = if let Some(id) = inputs.node_id {
                id
            } else {
                crate::trace::record_node(
                    crate::trace::NodeType::Root,
                    vec![],
                    Some(inputs.clone()),
                )
                .unwrap_or(0)
            };

            crate::trace::record_node(
                crate::trace::NodeType::Predict {
                    signature_name: "Predict".to_string(),
                    signature: self.signature.clone(),
                },
                vec![input_id],
                None,
            )
        } else {
            None
        };

        let result = ChatAdapter
            .call(lm, self.signature.as_ref(), inputs, self.tools.clone())
            .await;

        // Handle result and emit callbacks
        match result {
            Ok(mut prediction) => {
                if let Some(id) = trace_node_id {
                    prediction.node_id = Some(id);
                    crate::trace::record_output(id, prediction.clone());
                }

                // Emit module end callback (success)
                callback.on_module_end(call_id, Ok(&prediction));

                Ok(prediction)
            }
            Err(e) => {
                // Emit module end callback (error)
                callback.on_module_end(call_id, Err(&e));
                Err(e)
            }
        }
    }
}

impl Optimizable for Predict {
    fn get_signature(&self) -> &dyn MetaSignature {
        self.signature.as_ref()
    }

    fn parameters(&mut self) -> IndexMap<String, &mut dyn Optimizable> {
        IndexMap::new()
    }

    fn update_signature_instruction(&mut self, instruction: String) -> anyhow::Result<()> {
        if let Some(sig) = Arc::get_mut(&mut self.signature) {
            sig.update_instruction(instruction)?;
            Ok(())
        } else {
            // If Arc is shared, we might need to clone it first?
            // But Optimizable usually assumes exclusive access for modification.
            // If we are optimizing, we should have ownership or mutable access.
            // If tracing is active, `Predict` instances might be shared in Graph, but here we are modifying the instance.
            // If we can't get mut, it means it's shared.
            // We can clone-on-write? But MetaSignature is a trait object, so we can't easily clone it unless we implement Clone for Box<dyn MetaSignature>.
            // However, we changed it to Arc.
            // If we are running optimization, we probably shouldn't be tracing or the graph is already built.
            // For now, let's error or assume we can clone if we had a way.
            // But actually, we can't clone `dyn MetaSignature` easily without more boilerplate.
            // Let's assume unique ownership for optimization.
            anyhow::bail!(
                "Cannot update signature instruction: Signature is shared (Arc has multiple strong references)"
            )
        }
    }
}
