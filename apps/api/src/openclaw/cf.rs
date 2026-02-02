use serde::{Deserialize, Serialize};
use worker::{Env, Result};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProvisionedInstance {
    pub runtime_url: String,
    pub runtime_name: Option<String>,
    pub cf_account_id: Option<String>,
    pub cf_worker_name: Option<String>,
    pub cf_worker_id: Option<String>,
    pub cf_container_app_id: Option<String>,
    pub cf_container_app_name: Option<String>,
    pub r2_bucket_name: Option<String>,
}

fn sanitize_id(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            out.push('-');
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn runtime_url_from_env(env: &Env, user_id: &str) -> Option<String> {
    if let Ok(var) = env.var("OPENCLAW_RUNTIME_URL") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if let Ok(var) = env.var("OPENCLAW_RUNTIME_URL_TEMPLATE") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            if value.contains("{user_id}") {
                return Some(value.replace("{user_id}", user_id));
            }
            return Some(value);
        }
    }
    None
}

fn runtime_name_from_env(env: &Env, user_id: &str) -> Option<String> {
    if let Ok(var) = env.var("OPENCLAW_RUNTIME_NAME") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if let Ok(var) = env.var("OPENCLAW_RUNTIME_NAME_PREFIX") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Some(format!("{}{}", value, sanitize_id(user_id)));
        }
    }
    None
}

fn r2_bucket_from_env(env: &Env, user_id: &str) -> Option<String> {
    if let Ok(var) = env.var("OPENCLAW_R2_BUCKET") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    if let Ok(var) = env.var("OPENCLAW_R2_BUCKET_PREFIX") {
        let value = var.to_string();
        if !value.trim().is_empty() {
            return Some(format!("{}{}", value, sanitize_id(user_id)));
        }
    }
    None
}

pub async fn provision_instance(env: &Env, user_id: &str) -> Result<ProvisionedInstance> {
    let runtime_url = runtime_url_from_env(env, user_id).ok_or_else(|| {
        worker::Error::RustError("OPENCLAW_RUNTIME_URL not configured".to_string())
    })?;

    Ok(ProvisionedInstance {
        runtime_url,
        runtime_name: runtime_name_from_env(env, user_id),
        cf_account_id: env.var("CF_ACCOUNT_ID").ok().map(|v| v.to_string()),
        cf_worker_name: env.var("OPENCLAW_CF_WORKER_NAME").ok().map(|v| v.to_string()),
        cf_worker_id: None,
        cf_container_app_id: None,
        cf_container_app_name: env.var("OPENCLAW_CF_CONTAINER_APP_NAME")
            .ok()
            .map(|v| v.to_string()),
        r2_bucket_name: r2_bucket_from_env(env, user_id),
    })
}
