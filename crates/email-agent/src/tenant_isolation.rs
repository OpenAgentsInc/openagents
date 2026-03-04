use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantNetworkBoundary {
    pub allowed_egress_domains: Vec<String>,
    pub relay_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantStorageLayout {
    pub tenant_root: String,
    pub config_path: String,
    pub state_db_path: String,
    pub audit_log_path: String,
    pub attachment_root: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantRuntimeIdentity {
    pub runtime_identity_id: String,
    pub nostr_identity_ref: String,
    pub wallet_identity_ref: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantSecretScope {
    pub scope_id: String,
    pub version: u32,
    pub credentials_namespace: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantEnvironment {
    pub tenant_id: String,
    pub storage: TenantStorageLayout,
    pub runtime_identity: TenantRuntimeIdentity,
    pub secret_scope: TenantSecretScope,
    pub network_boundary: TenantNetworkBoundary,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantProvisionRequest {
    pub tenant_id: String,
    pub root_dir: String,
    pub allowed_egress_domains: Vec<String>,
    pub relay_allowlist: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantTeardownPlan {
    pub tenant_id: String,
    pub revoke_runtime_identity_refs: Vec<String>,
    pub revoke_secret_scope_id: String,
    pub wipe_paths: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TenantIsolationReport {
    pub is_hard_isolation_satisfied: bool,
    pub violations: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct TenantIsolationState {
    pub tenants: BTreeMap<String, TenantEnvironment>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum TenantIsolationError {
    #[error("invalid tenant input: {0}")]
    InvalidInput(String),
    #[error("tenant already exists: {0}")]
    TenantAlreadyExists(String),
    #[error("tenant not found: {0}")]
    TenantNotFound(String),
}

pub fn provision_tenant_environment(
    state: &mut TenantIsolationState,
    request: TenantProvisionRequest,
) -> Result<TenantEnvironment, TenantIsolationError> {
    validate_tenant_id(request.tenant_id.as_str())?;
    if request.root_dir.trim().is_empty() {
        return Err(TenantIsolationError::InvalidInput(
            "root_dir must not be empty".to_string(),
        ));
    }
    if request.allowed_egress_domains.is_empty() {
        return Err(TenantIsolationError::InvalidInput(
            "allowed_egress_domains must not be empty".to_string(),
        ));
    }
    if request.relay_allowlist.is_empty() {
        return Err(TenantIsolationError::InvalidInput(
            "relay_allowlist must not be empty".to_string(),
        ));
    }
    if state.tenants.contains_key(request.tenant_id.as_str()) {
        return Err(TenantIsolationError::TenantAlreadyExists(request.tenant_id));
    }

    let tenant_root = format!("{}/{}/", request.root_dir.trim_end_matches('/'), request.tenant_id);
    let environment = TenantEnvironment {
        tenant_id: request.tenant_id.clone(),
        storage: TenantStorageLayout {
            config_path: format!("{tenant_root}config/email-agent.toml"),
            state_db_path: format!("{tenant_root}state/email-agent.sqlite"),
            audit_log_path: format!("{tenant_root}audit/pipeline.log"),
            attachment_root: format!("{tenant_root}attachments/"),
            tenant_root,
        },
        runtime_identity: TenantRuntimeIdentity {
            runtime_identity_id: format!("runtime:{}", request.tenant_id),
            nostr_identity_ref: format!("nostr/{}", request.tenant_id),
            wallet_identity_ref: format!("wallet/{}", request.tenant_id),
        },
        secret_scope: TenantSecretScope {
            scope_id: format!("secret-scope:{}", request.tenant_id),
            version: 1,
            credentials_namespace: format!("email-agent/{}/", request.tenant_id),
        },
        network_boundary: TenantNetworkBoundary {
            allowed_egress_domains: sorted_unique(request.allowed_egress_domains),
            relay_allowlist: sorted_unique(request.relay_allowlist),
        },
    };

    state
        .tenants
        .insert(environment.tenant_id.clone(), environment.clone());
    Ok(environment)
}

pub fn rotate_tenant_secret_scope(
    state: &mut TenantIsolationState,
    tenant_id: &str,
) -> Result<TenantSecretScope, TenantIsolationError> {
    let tenant = state
        .tenants
        .get_mut(tenant_id)
        .ok_or_else(|| TenantIsolationError::TenantNotFound(tenant_id.to_string()))?;
    tenant.secret_scope.version = tenant.secret_scope.version.saturating_add(1);
    Ok(tenant.secret_scope.clone())
}

pub fn teardown_tenant_environment(
    state: &mut TenantIsolationState,
    tenant_id: &str,
) -> Result<TenantTeardownPlan, TenantIsolationError> {
    let tenant = state
        .tenants
        .remove(tenant_id)
        .ok_or_else(|| TenantIsolationError::TenantNotFound(tenant_id.to_string()))?;

    Ok(TenantTeardownPlan {
        tenant_id: tenant.tenant_id,
        revoke_runtime_identity_refs: vec![
            tenant.runtime_identity.runtime_identity_id,
            tenant.runtime_identity.nostr_identity_ref,
            tenant.runtime_identity.wallet_identity_ref,
        ],
        revoke_secret_scope_id: tenant.secret_scope.scope_id,
        wipe_paths: vec![
            tenant.storage.config_path,
            tenant.storage.state_db_path,
            tenant.storage.audit_log_path,
            tenant.storage.attachment_root,
        ],
    })
}

pub fn verify_hard_tenant_isolation(state: &TenantIsolationState) -> TenantIsolationReport {
    let mut violations = Vec::<String>::new();

    let mut storage_paths = BTreeSet::<String>::new();
    let mut identity_refs = BTreeSet::<String>::new();
    let mut secret_scopes = BTreeSet::<String>::new();
    let mut credential_namespaces = BTreeSet::<String>::new();

    for tenant in state.tenants.values() {
        for path in [
            tenant.storage.config_path.as_str(),
            tenant.storage.state_db_path.as_str(),
            tenant.storage.audit_log_path.as_str(),
            tenant.storage.attachment_root.as_str(),
        ] {
            if !storage_paths.insert(path.to_string()) {
                violations.push(format!("shared storage path detected: {path}"));
            }
        }

        for identity in [
            tenant.runtime_identity.runtime_identity_id.as_str(),
            tenant.runtime_identity.nostr_identity_ref.as_str(),
            tenant.runtime_identity.wallet_identity_ref.as_str(),
        ] {
            if !identity_refs.insert(identity.to_string()) {
                violations.push(format!("shared runtime identity detected: {identity}"));
            }
        }

        if !secret_scopes.insert(tenant.secret_scope.scope_id.clone()) {
            violations.push(format!(
                "shared secret scope detected: {}",
                tenant.secret_scope.scope_id
            ));
        }
        if !credential_namespaces.insert(tenant.secret_scope.credentials_namespace.clone()) {
            violations.push(format!(
                "shared credentials namespace detected: {}",
                tenant.secret_scope.credentials_namespace
            ));
        }
    }

    TenantIsolationReport {
        is_hard_isolation_satisfied: violations.is_empty(),
        violations,
    }
}

fn validate_tenant_id(tenant_id: &str) -> Result<(), TenantIsolationError> {
    if tenant_id.trim().is_empty() {
        return Err(TenantIsolationError::InvalidInput(
            "tenant_id must not be empty".to_string(),
        ));
    }
    if !tenant_id
        .chars()
        .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_'))
    {
        return Err(TenantIsolationError::InvalidInput(
            "tenant_id must contain only ascii letters, digits, '-' or '_'".to_string(),
        ));
    }
    Ok(())
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    let mut unique = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<String>>();
    unique.sort();
    unique.dedup();
    unique
}

#[cfg(test)]
mod tests {
    use super::{
        TenantIsolationState, TenantProvisionRequest, provision_tenant_environment,
        rotate_tenant_secret_scope, teardown_tenant_environment, verify_hard_tenant_isolation,
    };

    #[test]
    fn provisioning_creates_isolated_tenant_layouts() {
        let mut state = TenantIsolationState::default();
        let tenant_a = provision_tenant_environment(
            &mut state,
            TenantProvisionRequest {
                tenant_id: "tenant_a".to_string(),
                root_dir: "/srv/email-agent".to_string(),
                allowed_egress_domains: vec!["gmail.googleapis.com".to_string()],
                relay_allowlist: vec!["wss://relay.example.com".to_string()],
            },
        )
        .expect("tenant a should provision");
        let tenant_b = provision_tenant_environment(
            &mut state,
            TenantProvisionRequest {
                tenant_id: "tenant_b".to_string(),
                root_dir: "/srv/email-agent".to_string(),
                allowed_egress_domains: vec!["gmail.googleapis.com".to_string()],
                relay_allowlist: vec!["wss://relay.example.com".to_string()],
            },
        )
        .expect("tenant b should provision");

        assert_ne!(tenant_a.storage.tenant_root, tenant_b.storage.tenant_root);
        assert_ne!(
            tenant_a.secret_scope.credentials_namespace,
            tenant_b.secret_scope.credentials_namespace
        );
        let report = verify_hard_tenant_isolation(&state);
        assert!(report.is_hard_isolation_satisfied);
    }

    #[test]
    fn isolation_report_detects_shared_storage_paths() {
        let mut state = TenantIsolationState::default();
        let _ = provision_tenant_environment(
            &mut state,
            TenantProvisionRequest {
                tenant_id: "tenant_one".to_string(),
                root_dir: "/srv/email-agent".to_string(),
                allowed_egress_domains: vec!["gmail.googleapis.com".to_string()],
                relay_allowlist: vec!["wss://relay.example.com".to_string()],
            },
        )
        .expect("tenant one should provision");
        let _ = provision_tenant_environment(
            &mut state,
            TenantProvisionRequest {
                tenant_id: "tenant_two".to_string(),
                root_dir: "/srv/email-agent".to_string(),
                allowed_egress_domains: vec!["gmail.googleapis.com".to_string()],
                relay_allowlist: vec!["wss://relay.example.com".to_string()],
            },
        )
        .expect("tenant two should provision");

        let tenant_two = state
            .tenants
            .get_mut("tenant_two")
            .expect("tenant two should exist");
        tenant_two.storage.state_db_path = "/srv/email-agent/tenant_one/state/email-agent.sqlite".to_string();

        let report = verify_hard_tenant_isolation(&state);
        assert!(!report.is_hard_isolation_satisfied);
        assert!(
            report
                .violations
                .iter()
                .any(|violation| violation.contains("shared storage path"))
        );
    }

    #[test]
    fn secret_scope_rotation_and_teardown_are_deterministic() {
        let mut state = TenantIsolationState::default();
        let _ = provision_tenant_environment(
            &mut state,
            TenantProvisionRequest {
                tenant_id: "tenant_rotate".to_string(),
                root_dir: "/srv/email-agent".to_string(),
                allowed_egress_domains: vec!["gmail.googleapis.com".to_string()],
                relay_allowlist: vec!["wss://relay.example.com".to_string()],
            },
        )
        .expect("tenant should provision");

        let rotated = rotate_tenant_secret_scope(&mut state, "tenant_rotate")
            .expect("rotation should succeed");
        assert_eq!(rotated.version, 2);

        let plan = teardown_tenant_environment(&mut state, "tenant_rotate")
            .expect("teardown should succeed");
        assert_eq!(plan.tenant_id, "tenant_rotate");
        assert!(plan.revoke_secret_scope_id.contains("tenant_rotate"));
        assert!(plan.wipe_paths.iter().any(|path| path.contains("audit")));
    }
}
