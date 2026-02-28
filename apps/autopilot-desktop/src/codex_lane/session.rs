use super::*;

pub(super) fn mcp_auth_status_label(status: codex_client::McpAuthStatus) -> &'static str {
    match status {
        codex_client::McpAuthStatus::Unsupported => "unsupported",
        codex_client::McpAuthStatus::NotLoggedIn => "not_logged_in",
        codex_client::McpAuthStatus::BearerToken => "bearer_token",
        codex_client::McpAuthStatus::OAuth => "oauth",
    }
}

pub(super) fn is_disconnect_error(error: &anyhow::Error) -> bool {
    let text = error.to_string().to_ascii_lowercase();
    text.contains("connection closed")
        || text.contains("channel closed")
        || text.contains("broken pipe")
        || text.contains("transport endpoint is not connected")
        || text.contains("request canceled")
        || text.contains("app-server write failed")
        || text.contains("app-server request canceled")
        || text.contains("app-server connection closed")
}

pub(super) fn summarize_skills_list_response(
    response: SkillsListResponse,
) -> Vec<CodexSkillListEntry> {
    response
        .data
        .into_iter()
        .map(|entry| CodexSkillListEntry {
            cwd: entry.cwd.display().to_string(),
            skills: entry
                .skills
                .into_iter()
                .map(|skill| CodexSkillSummary {
                    name: skill.name,
                    path: skill.path.display().to_string(),
                    scope: skill_scope_label(skill.scope).to_string(),
                    enabled: skill.enabled,
                    interface_display_name: skill
                        .interface
                        .and_then(|interface| interface.display_name),
                    dependency_count: skill
                        .dependencies
                        .map_or(0, |dependencies| dependencies.tools.len()),
                })
                .collect(),
            errors: entry
                .errors
                .into_iter()
                .map(|error| format!("{}: {}", error.path.display(), error.message))
                .collect(),
        })
        .collect()
}

pub(super) fn account_summary(response: &codex_client::GetAccountResponse) -> String {
    match response.account.as_ref() {
        Some(codex_client::AccountInfo::ApiKey) => "apiKey".to_string(),
        Some(codex_client::AccountInfo::Chatgpt { email, plan_type }) => {
            format!("chatgpt:{}:{plan_type:?}", email)
        }
        None => "none".to_string(),
    }
}

pub(super) fn rate_limits_summary(rate_limits: &Value) -> String {
    let plan = rate_limits
        .get("planType")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let primary_used = rate_limits
        .get("primary")
        .and_then(|value| value.get("usedPercent"))
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let secondary_used = rate_limits
        .get("secondary")
        .and_then(|value| value.get("usedPercent"))
        .and_then(Value::as_i64)
        .unwrap_or_default();
    format!("plan={plan} primary={primary_used}% secondary={secondary_used}%")
}

pub(super) fn fetch_model_catalog_entries(
    runtime: &Runtime,
    client: &AppServerClient,
    params: ModelListParams,
) -> Result<(Vec<CodexModelCatalogEntry>, Option<String>)> {
    let mut cursor = params.cursor;
    let limit = params.limit.or(Some(100));
    let include_hidden = params.include_hidden;
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    let mut default_model = None;

    loop {
        let response = runtime.block_on(client.model_list(ModelListParams {
            cursor: cursor.clone(),
            limit,
            include_hidden,
        }))?;

        for model in response.data {
            let value = model.model.trim();
            if value.is_empty() {
                continue;
            }
            if !seen.insert(value.to_string()) {
                continue;
            }

            if model.is_default && default_model.is_none() {
                default_model = Some(value.to_string());
            }

            let default_reasoning_effort = serde_json::to_string(&model.default_reasoning_effort)
                .unwrap_or_else(|_| "\"unknown\"".to_string())
                .trim_matches('"')
                .to_string();
            let supported_reasoning_efforts = model
                .supported_reasoning_efforts
                .iter()
                .map(|effort| {
                    serde_json::to_string(&effort.reasoning_effort)
                        .unwrap_or_else(|_| "\"unknown\"".to_string())
                        .trim_matches('"')
                        .to_string()
                })
                .collect();

            entries.push(CodexModelCatalogEntry {
                model: value.to_string(),
                display_name: model.display_name,
                description: model.description,
                hidden: model.hidden,
                is_default: model.is_default,
                default_reasoning_effort,
                supported_reasoning_efforts,
            });
        }

        match response.next_cursor {
            Some(next) if !next.is_empty() => {
                cursor = Some(next);
            }
            _ => break,
        }
    }

    Ok((entries, default_model))
}

pub(super) fn fetch_model_catalog(
    runtime: &Runtime,
    client: &AppServerClient,
) -> Result<(Vec<String>, Option<String>)> {
    let (entries, default_model) = fetch_model_catalog_entries(
        runtime,
        client,
        ModelListParams {
            cursor: None,
            limit: Some(100),
            include_hidden: None,
        },
    )?;
    let models = entries.into_iter().map(|entry| entry.model).collect();
    Ok((models, default_model))
}

pub(super) fn skill_scope_label(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::User => "user",
        SkillScope::Repo => "repo",
        SkillScope::System => "system",
        SkillScope::Admin => "admin",
    }
}
