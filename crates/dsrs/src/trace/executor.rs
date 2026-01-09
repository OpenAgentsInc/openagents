use crate::trace::dag::{Graph, NodeType};
use crate::{Example, GLOBAL_SETTINGS, Prediction};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Executor {
    pub graph: Graph,
}

impl Executor {
    pub fn new(graph: Graph) -> Self {
        Self { graph }
    }

    pub async fn execute(&self, root_input: Example) -> Result<Vec<Prediction>> {
        // Simple execution: assume graph nodes are in topological order (which they are by construction of trace)
        // Store outputs of each node
        let mut node_outputs: HashMap<usize, Prediction> = HashMap::new();
        // Store input example for root node 0 (if valid)
        // Actually, Root node 0 usually contains the input data from trace.
        // If we want to run with NEW input, we replace Root's data.

        // We will return the output of the *last* node(s), or just all predictions?
        // Usually we want the leaf nodes.

        let mut final_predictions = Vec::new();

        for node in &self.graph.nodes {
            match &node.node_type {
                NodeType::Root => {
                    // For root, we use the provided root_input
                    // But wait, the graph might have multiple roots or specific inputs?
                    // For simplicity, assume node 0 is the main root and takes root_input.
                    // Or we check if node.id == 0.
                    if node.id == 0 {
                        // Creating a "Prediction" that just holds the input data, so downstream nodes can read it.
                        // Wait, Prediction structure is for outputs.
                        // But Map nodes read from "Prediction" or "Example"?
                        // Map inputs come from `TrackedValue`, which stores (node_id, key).
                        // If node_id points to Root, we need to get data from Root.
                        // We can synthesize a Prediction from Example data for uniform access.

                        let pred = Prediction::from(
                            root_input
                                .data
                                .iter()
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect::<Vec<_>>(),
                        );
                        node_outputs.insert(node.id, pred);
                    } else {
                        // Other roots? maybe constants?
                        if let Some(data) = &node.input_data {
                            let pred = Prediction::from(
                                data.data
                                    .iter()
                                    .map(|(k, v)| (k.clone(), v.clone()))
                                    .collect::<Vec<_>>(),
                            );
                            node_outputs.insert(node.id, pred);
                        }
                    }
                }
                NodeType::Predict { signature, .. } => {
                    // Gather inputs
                    // Predict node inputs usually come from a Map node (which prepared the Example)
                    // Or directly if it's just raw data?
                    // Typically: Node A -> Map -> Node B (Predict).
                    // Node B's `inputs` list contains the Map node ID.
                    // But `Predict` takes an `Example`.
                    // We need to reconstruct the `Example` from the output of the previous node.

                    // IF the previous node was a Map node, its "output" should be the `Example` ready for this predictor.
                    // Let's see how Map nodes work.

                    // Actually, `Predict` takes `Example`.
                    // In the trace, we recorded `inputs.node_id`.
                    // So the parent of this Predict node IS the node that produced the `Example`.
                    // If that parent is a Map node, we expect the Map node to produce a "Prediction" that acts as the Example?
                    // Yes, `Map` node output can be treated as the `Example` data.

                    if let Some(parent_id) = node.inputs.first()
                        && let Some(input_pred) = node_outputs.get(parent_id)
                    {
                        // Convert Prediction back to Example
                        let example = Example::new(
                            input_pred.data.clone(),
                            vec![], // input_keys
                            vec![], // output_keys
                        );

                        // Execute Predict
                        let (adapter, lm) = {
                            let guard = GLOBAL_SETTINGS.read().unwrap();
                            let settings = guard.as_ref().unwrap();
                            (settings.adapter.clone(), Arc::clone(&settings.lm))
                        };

                        // We need to use the stored signature
                        // Predict struct isn't stored, just signature.
                        // We reconstruct a temporary "Predict"-like behavior.
                        // Tools are lost in current trace? Yes, need to fix that if tools are important.
                        // For now, no tools.

                        let tools = vec![];
                        let result = adapter.call(lm, signature.as_ref(), example, tools).await?;

                        node_outputs.insert(node.id, result.clone());
                        final_predictions.push(result);
                    }
                }
                NodeType::Map { mapping } => {
                    // Execute the mapping
                    // We create a new "Prediction" (acting as data container) based on sources.
                    let mut data = HashMap::new();

                    for (output_key, (source_node_id, source_key)) in mapping {
                        if let Some(source_pred) = node_outputs.get(source_node_id) {
                            let val = source_pred.get(source_key, None);
                            data.insert(output_key.clone(), val);
                        }
                    }

                    let result = Prediction::from(
                        data.iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect::<Vec<_>>(),
                    );
                    node_outputs.insert(node.id, result);
                }
                NodeType::Operator { .. } => {
                    // Not implemented yet
                }
            }
        }

        // Return the output of the last node? or all Predict outputs?
        // Let's return the output of the last node in the list.
        if let Some(last_node) = self.graph.nodes.last()
            && let Some(output) = node_outputs.get(&last_node.id)
        {
            return Ok(vec![output.clone()]);
        }

        Ok(vec![])
    }
}
