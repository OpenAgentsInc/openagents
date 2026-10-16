//! SPACE's public v2 key-broker protocol over a Cloud KMS backend.
//! The workload bearer authenticates the credential authority. Only configured
//! local SPACE organizations are accepted; federated evidence is refused.

pub mod kms;
pub mod wire {
    tonic::include_proto!("space.credential.provider.v2");
}

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;
use tonic::{Request, Response, Status};
use uuid::Uuid;
use wire::{intermediate_key_broker_server::IntermediateKeyBroker, *};
use zeroize::Zeroizing;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub namespace: String,
    pub tenants: BTreeSet<String>,
    pub active_key: String,
    /// Logical routing names keep historical ciphertext usable after rotation.
    pub key_routes: BTreeMap<String, String>,
}
impl Policy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.namespace.is_empty()
            || self.namespace.len() > 128
            || !self.namespace.bytes().all(|b| b.is_ascii_graphic())
            || self.tenants.is_empty()
            || self.tenants.len() > 128
            || self
                .tenants
                .iter()
                .any(|t| Uuid::parse_str(t).map_or(true, |id| id.to_string() != *t))
            || !self.key_routes.contains_key(&self.active_key)
            || self.key_routes.is_empty()
            || self.key_routes.len() > 16
            || self
                .key_routes
                .iter()
                .any(|(alias, key)| !alias_ok(alias) || !kms::valid_key_name(key))
        {
            return Err("Check the key broker settings.");
        }
        Ok(())
    }
    fn tenant(&self, tenant: Option<&TenantRef>) -> Option<TenantRef> {
        tenant
            .filter(|t| t.namespace == self.namespace && self.tenants.contains(&t.id))
            .cloned()
    }
}
fn alias_ok(alias: &str) -> bool {
    !alias.is_empty() && alias.len() <= 16 && alias.bytes().all(|b| b.is_ascii_alphanumeric())
}
fn route<'a>(id: &'a str, policy: &Policy) -> Option<&'a str> {
    let mut p = id.splitn(3, '_');
    if p.next()? != "ik" {
        return None;
    }
    let alias = p.next()?;
    let uuid = p.next()?;
    if !policy.key_routes.contains_key(alias) || Uuid::parse_str(uuid).ok()?.to_string() != uuid {
        return None;
    }
    Some(alias)
}
fn aad(tenant: &TenantRef, key: &str) -> Vec<u8> {
    serde_json::to_vec(&(
        "openagents.space.intermediate-key.v1",
        &tenant.namespace,
        &tenant.id,
        key,
    ))
    .expect("strings encode")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    InvalidKey,
    Unavailable,
}
#[async_trait]
pub trait Kms: Send + Sync {
    async fn encrypt(&self, route: &str, plain: &[u8], aad: &[u8]) -> Result<Vec<u8>, Failure>;
    async fn decrypt(
        &self,
        route: &str,
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, Failure>;
}

pub struct Broker {
    policy: Policy,
    kms: Arc<dyn Kms>,
    bearer_digest: [u8; 32],
    permits: Semaphore,
}
impl Broker {
    pub fn new(policy: Policy, kms: Arc<dyn Kms>, bearer: &[u8]) -> Result<Self, &'static str> {
        policy.validate()?;
        if !(32..=512).contains(&bearer.len()) || !bearer.iter().all(|b| b.is_ascii_graphic()) {
            return Err("Choose a valid service credential.");
        }
        Ok(Self {
            policy,
            kms,
            bearer_digest: Sha256::digest(bearer).into(),
            permits: Semaphore::new(8),
        })
    }
    fn authorize<T>(&self, request: &Request<T>) -> Result<(), Status> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .filter(|h| (32..=512).contains(&h.len()))
            .ok_or_else(|| Status::unauthenticated("Sign in to continue."))?;
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if bool::from(self.bearer_digest.ct_eq(&digest)) {
            Ok(())
        } else {
            Err(Status::unauthenticated("Sign in to continue."))
        }
    }
}
fn issue_failure(reason: IssueFailureReason) -> IssueIntermediateKeyResponse {
    IssueIntermediateKeyResponse {
        outcome: Some(issue_intermediate_key_response::Outcome::Failure(
            IssueFailure {
                reason: reason as i32,
                detail: match reason {
                    IssueFailureReason::InvalidTenant => "This tenant is unavailable.",
                    IssueFailureReason::AuthorizationRejected => {
                        "Authorization evidence is not supported."
                    }
                    _ => "Key protection is unavailable. Try again later.",
                }
                .into(),
            },
        )),
    }
}
fn unwrap_failure(reason: UnwrapFailureReason) -> UnwrapIntermediateKeyResponse {
    UnwrapIntermediateKeyResponse {
        outcome: Some(unwrap_intermediate_key_response::Outcome::Failure(
            UnwrapFailure {
                reason: reason as i32,
                detail: match reason {
                    UnwrapFailureReason::InvalidKey => "This key could not be opened.",
                    UnwrapFailureReason::AuthorizationRejected => {
                        "Authorization evidence is not supported."
                    }
                    _ => "Key protection is unavailable. Try again later.",
                }
                .into(),
            },
        )),
    }
}

#[tonic::async_trait]
impl IntermediateKeyBroker for Broker {
    async fn issue_intermediate_key(
        &self,
        request: Request<IssueIntermediateKeyRequest>,
    ) -> Result<Response<IssueIntermediateKeyResponse>, Status> {
        self.authorize(&request)?;
        let request = request.into_inner();
        if request.authorization.is_some() {
            return Ok(Response::new(issue_failure(
                IssueFailureReason::AuthorizationRejected,
            )));
        }
        let Some(tenant) = self.policy.tenant(request.tenant.as_ref()) else {
            return Ok(Response::new(issue_failure(
                IssueFailureReason::InvalidTenant,
            )));
        };
        let Ok(_permit) = self.permits.try_acquire() else {
            return Ok(Response::new(issue_failure(
                IssueFailureReason::Unavailable,
            )));
        };
        let mut random = Zeroizing::new([0u8; 48]);
        if getrandom::fill(random.as_mut()).is_err() {
            return Ok(Response::new(issue_failure(
                IssueFailureReason::Unavailable,
            )));
        }
        let mut id: [u8; 16] = random[32..].try_into().expect("fixed length");
        id[6] = (id[6] & 15) | 64;
        id[8] = (id[8] & 63) | 128;
        let key_id = format!("ik_{}_{}", self.policy.active_key, Uuid::from_bytes(id));
        let outcome = match self
            .kms
            .encrypt(
                &self.policy.active_key,
                &random[..32],
                &aad(&tenant, &key_id),
            )
            .await
        {
            Ok(wrapped_key) if !wrapped_key.is_empty() && wrapped_key.len() <= 4096 => {
                IssueIntermediateKeyResponse {
                    outcome: Some(issue_intermediate_key_response::Outcome::Key(
                        WrappedIntermediateKey {
                            key_id,
                            wrapped_key,
                        },
                    )),
                }
            }
            _ => issue_failure(IssueFailureReason::Unavailable),
        };
        Ok(Response::new(outcome))
    }
    async fn unwrap_intermediate_key(
        &self,
        request: Request<UnwrapIntermediateKeyRequest>,
    ) -> Result<Response<UnwrapIntermediateKeyResponse>, Status> {
        self.authorize(&request)?;
        let request = request.into_inner();
        if request.authorization.is_some() {
            return Ok(Response::new(unwrap_failure(
                UnwrapFailureReason::AuthorizationRejected,
            )));
        }
        let Some(tenant) = self.policy.tenant(request.tenant.as_ref()) else {
            return Ok(Response::new(unwrap_failure(
                UnwrapFailureReason::InvalidKey,
            )));
        };
        let Some(alias) = route(&request.key_id, &self.policy) else {
            return Ok(Response::new(unwrap_failure(
                UnwrapFailureReason::InvalidKey,
            )));
        };
        if request.wrapped_key.is_empty() || request.wrapped_key.len() > 4096 {
            return Ok(Response::new(unwrap_failure(
                UnwrapFailureReason::InvalidKey,
            )));
        }
        let Ok(_permit) = self.permits.try_acquire() else {
            return Ok(Response::new(unwrap_failure(
                UnwrapFailureReason::Unavailable,
            )));
        };
        let outcome = match self
            .kms
            .decrypt(alias, &request.wrapped_key, &aad(&tenant, &request.key_id))
            .await
        {
            Ok(plain) if plain.len() == 32 => UnwrapIntermediateKeyResponse {
                outcome: Some(unwrap_intermediate_key_response::Outcome::Key(
                    IntermediateKeyMaterial {
                        plaintext_key: plain.to_vec(),
                        max_cache_ttl: Some(prost_types::Duration {
                            seconds: 0,
                            nanos: 0,
                        }),
                    },
                )),
            },
            Err(Failure::InvalidKey) => unwrap_failure(UnwrapFailureReason::InvalidKey),
            _ => unwrap_failure(UnwrapFailureReason::Unavailable),
        };
        Ok(Response::new(outcome))
    }
}

#[cfg(test)]
mod tests;
