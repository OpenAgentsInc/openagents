use anyhow::Result;
use rig::tool::ToolDyn;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use super::Adapter;
use crate::callbacks::DspyCallback;
use crate::serde_utils::get_iter_from_value;
use crate::{Cache, CallResult, Chat, Example, LM, Message, MetaSignature, Prediction};

#[derive(Default, Clone)]
pub struct ChatAdapter;

const SIGNATURE_CACHE_KEY: &str = "__signature";

fn get_type_hint(field: &Value) -> String {
    let schema = &field["schema"];
    let type_str = field["type"].as_str().unwrap_or("String");

    // Check if schema exists and is not empty (either as string or object)
    let has_schema = if let Some(s) = schema.as_str() {
        !s.is_empty()
    } else {
        schema.is_object()
    };

    if !has_schema && type_str == "String" {
        String::new()
    } else {
        format!(" (must be formatted as valid Rust {type_str})")
    }
}

impl ChatAdapter {
    fn cache_key(signature: &dyn MetaSignature, inputs: &Example) -> Example {
        let mut key = inputs.clone();
        key.data.insert(
            SIGNATURE_CACHE_KEY.to_string(),
            Value::String(signature.signature_name().to_string()),
        );
        if !key.input_keys.contains(&SIGNATURE_CACHE_KEY.to_string()) {
            key.input_keys.push(SIGNATURE_CACHE_KEY.to_string());
        }
        key.output_keys
            .retain(|field| field != SIGNATURE_CACHE_KEY);
        key
    }

    fn get_field_attribute_list(
        &self,
        field_iter: impl Iterator<Item = (String, Value)>,
    ) -> String {
        let mut field_attributes = String::new();
        for (i, (field_name, field)) in field_iter.enumerate() {
            let data_type = field["type"].as_str().unwrap_or("String");
            let desc = field["desc"].as_str().unwrap_or("");

            field_attributes.push_str(format!("{}. `{field_name}` ({data_type})", i + 1).as_str());
            if !desc.is_empty() {
                field_attributes.push_str(format!(": {desc}").as_str());
            }
            field_attributes.push('\n');
        }
        field_attributes
    }

    fn get_field_structure(&self, field_iter: impl Iterator<Item = (String, Value)>) -> String {
        let mut field_structure = String::new();
        for (field_name, field) in field_iter {
            let schema = &field["schema"];
            let data_type = field["type"].as_str().unwrap_or("String");

            // Handle schema as either string or JSON object
            let schema_prompt = if let Some(s) = schema.as_str() {
                if s.is_empty() && data_type == "String" {
                    "".to_string()
                } else if !s.is_empty() {
                    format!("\t# note: the value you produce must adhere to the JSON schema: {s}")
                } else {
                    format!("\t# note: the value you produce must be a single {data_type} value")
                }
            } else if schema.is_object() || schema.is_array() {
                // Convert JSON object/array to string for display
                let schema_str = schema.to_string();
                format!(
                    "\t# note: the value you produce must adhere to the JSON schema: {schema_str}"
                )
            } else if data_type == "String" {
                "".to_string()
            } else {
                format!("\t# note: the value you produce must be a single {data_type} value")
            };

            field_structure.push_str(
                format!("[[ ## {field_name} ## ]]\n{field_name}{schema_prompt}\n\n").as_str(),
            );
        }
        field_structure
    }

    fn format_system_message(&self, signature: &dyn MetaSignature) -> String {
        let field_description = self.format_field_description(signature);
        let field_structure = self.format_field_structure(signature);
        let task_description = self.format_task_description(signature);

        format!("{field_description}\n{field_structure}\n{task_description}")
    }

    fn format_field_description(&self, signature: &dyn MetaSignature) -> String {
        let input_field_description =
            self.get_field_attribute_list(get_iter_from_value(&signature.input_fields()));
        let output_field_description =
            self.get_field_attribute_list(get_iter_from_value(&signature.output_fields()));

        format!(
            "Your input fields are:\n{input_field_description}\nYour output fields are:\n{output_field_description}"
        )
    }

    fn format_field_structure(&self, signature: &dyn MetaSignature) -> String {
        let input_field_structure =
            self.get_field_structure(get_iter_from_value(&signature.input_fields()));
        let output_field_structure =
            self.get_field_structure(get_iter_from_value(&signature.output_fields()));

        format!(
            "All interactions will be structured in the following way, with the appropriate values filled in.\n\n{input_field_structure}{output_field_structure}[[ ## completed ## ]]\n"
        )
    }

    fn format_task_description(&self, signature: &dyn MetaSignature) -> String {
        let instruction = if signature.instruction().is_empty() {
            format!(
                "Given the fields {}, produce the fields {}.",
                signature
                    .input_fields()
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<String>>()
                    .join(", "),
                signature
                    .output_fields()
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|k| format!("`{k}`"))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        } else {
            signature.instruction().clone()
        };

        format!("In adhering to this structure, your objective is:\n\t{instruction}")
    }

    fn format_user_message(&self, signature: &dyn MetaSignature, inputs: &Example) -> String {
        let mut input_str = String::new();
        for (field_name, _) in get_iter_from_value(&signature.input_fields()) {
            let field_value = inputs.get(field_name.as_str(), None);
            // Extract the actual string value if it's a JSON string, otherwise use as is
            let field_value_str = if let Some(s) = field_value.as_str() {
                s.to_string()
            } else {
                field_value.to_string()
            };

            input_str
                .push_str(format!("[[ ## {field_name} ## ]]\n{field_value_str}\n\n",).as_str());
        }

        let first_output_field = signature
            .output_fields()
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .clone();
        let first_output_field_value = signature
            .output_fields()
            .as_object()
            .unwrap()
            .get(&first_output_field)
            .unwrap()
            .clone();

        let type_hint = get_type_hint(&first_output_field_value);

        let mut user_message = format!(
            "Respond with the corresponding output fields, starting with the field `{first_output_field}`{type_hint},"
        );
        for (field_name, field) in get_iter_from_value(&signature.output_fields()).skip(1) {
            user_message
                .push_str(format!(" then `{field_name}`{},", get_type_hint(&field)).as_str());
        }
        user_message.push_str(" and then ending with the marker for `completed`.");

        format!("{input_str}{user_message}")
    }

    fn format_assistant_message(&self, signature: &dyn MetaSignature, outputs: &Example) -> String {
        let mut assistant_message = String::new();
        for (field_name, _) in get_iter_from_value(&signature.output_fields()) {
            let field_value = outputs.get(field_name.as_str(), None);
            // Extract the actual string value if it's a JSON string, otherwise use as is
            let field_value_str = if let Some(s) = field_value.as_str() {
                s.to_string()
            } else {
                field_value.to_string()
            };

            assistant_message
                .push_str(format!("[[ ## {field_name} ## ]]\n{field_value_str}\n\n",).as_str());
        }
        assistant_message.push_str("[[ ## completed ## ]]\n");
        assistant_message
    }

    fn format_demos(&self, signature: &dyn MetaSignature, demos: &Vec<Example>) -> Chat {
        let mut chat = Chat::new(vec![]);

        for demo in demos {
            let user_message = self.format_user_message(signature, demo);
            let assistant_message = self.format_assistant_message(signature, demo);
            chat.push("user", &user_message);
            chat.push("assistant", &assistant_message);
        }

        chat
    }
}

#[async_trait::async_trait]
impl Adapter for ChatAdapter {
    fn format(&self, signature: &dyn MetaSignature, inputs: Example) -> Chat {
        let system_message = self.format_system_message(signature);
        let user_message = self.format_user_message(signature, &inputs);

        let demos = signature.demos();
        let demos = self.format_demos(signature, &demos);

        let mut chat = Chat::new(vec![]);
        chat.push("system", &system_message);
        chat.push_all(&demos);
        chat.push("user", &user_message);

        chat
    }

    fn parse_response(
        &self,
        signature: &dyn MetaSignature,
        response: Message,
    ) -> HashMap<String, Value> {
        let mut output = HashMap::new();

        let response_content = response.content();

        for (field_name, field) in get_iter_from_value(&signature.output_fields()) {
            let field_value = response_content
                .split(format!("[[ ## {field_name} ## ]]\n").as_str())
                .nth(1);

            if field_value.is_none() {
                continue; // Skip field if not found in response
            }
            let field_value = field_value.unwrap();

            let extracted_field = field_value.split("[[ ## ").nth(0).unwrap().trim();
            let data_type = field["type"].as_str().unwrap();
            let schema = &field["schema"];

            // Check if schema exists (as string or object)
            let has_schema = if let Some(s) = schema.as_str() {
                !s.is_empty()
            } else {
                schema.is_object() || schema.is_array()
            };

            if !has_schema && data_type == "String" {
                output.insert(field_name.clone(), json!(extracted_field));
            } else {
                output.insert(
                    field_name.clone(),
                    serde_json::from_str(extracted_field).unwrap(),
                );
            }
        }

        output
    }

    async fn call(
        &self,
        lm: Arc<LM>,
        signature: &dyn MetaSignature,
        inputs: Example,
        tools: Vec<Arc<dyn ToolDyn>>,
    ) -> Result<Prediction> {
        self.call_streaming(lm, signature, inputs, tools, None).await
    }

    async fn call_streaming(
        &self,
        lm: Arc<LM>,
        signature: &dyn MetaSignature,
        inputs: Example,
        tools: Vec<Arc<dyn ToolDyn>>,
        callback: Option<&dyn DspyCallback>,
    ) -> Result<Prediction> {
        let cache_key = Self::cache_key(signature, &inputs);

        // Check cache first (release lock immediately after checking)
        if lm.cache
            && let Some(cache) = lm.cache_handler.as_ref()
        {
            if let Some(cached) = cache.lock().await.get(cache_key.clone()).await? {
                return Ok(cached);
            }
        }

        let messages = self.format(signature, inputs.clone());
        let response = lm
            .call_with_signature_streaming(Some(signature), messages, tools, callback)
            .await?;
        let prompt_str = response.chat.to_json().to_string();

        let mut output = self.parse_response(signature, response.output);
        if !response.tool_calls.is_empty() {
            output.insert(
                "tool_calls".to_string(),
                response
                    .tool_calls
                    .into_iter()
                    .map(|call| json!(call))
                    .collect::<Value>(),
            );
            output.insert(
                "tool_executions".to_string(),
                response
                    .tool_executions
                    .into_iter()
                    .map(|execution| json!(execution))
                    .collect::<Value>(),
            );
        }

        let prediction = Prediction {
            data: output,
            lm_usage: response.usage,
            node_id: None,
        };

        // Store in cache if enabled
        if lm.cache
            && let Some(cache) = lm.cache_handler.as_ref()
        {
            let (tx, rx) = tokio::sync::mpsc::channel(1);
            let cache_clone = cache.clone();
            let inputs_clone = cache_key.clone();

            // Spawn the cache insert operation to avoid deadlock
            tokio::spawn(async move {
                let _ = cache_clone.lock().await.insert(inputs_clone, rx).await;
            });

            // Send the result to the cache
            tx.send(CallResult {
                prompt: prompt_str,
                prediction: prediction.clone(),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Failed to send to cache"))?;
        }

        Ok(prediction)
    }
}
