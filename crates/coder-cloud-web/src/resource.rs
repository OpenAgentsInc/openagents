//! A pinned resource connection can remove a view but cannot replace its source.

use crate::digest;
use serde::Deserialize;

pub const MAX_RESOURCE_BYTES: usize = 4 * 1024;
pub const MAX_RESOURCE_DESCRIPTOR_BYTES: usize = 8 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    endpoint: String,
    identity: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Standing {
    active: bool,
    identity: String,
}

impl Resource {
    pub fn admit(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_RESOURCE_DESCRIPTOR_BYTES {
            return None;
        }
        let resource: Self = serde_json::from_slice(bytes).ok()?;
        (endpoint(&resource.endpoint) && digest(&resource.identity)).then_some(resource)
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn accepts(&self, bytes: &[u8]) -> bool {
        if bytes.len() > MAX_RESOURCE_BYTES {
            return false;
        }
        serde_json::from_slice::<Standing>(bytes).is_ok_and(|standing| {
            standing.active && digest(&standing.identity) && standing.identity == self.identity
        })
    }
}

fn endpoint(value: &str) -> bool {
    if value.len() > 4096
        || !value.bytes().all(|byte| byte.is_ascii_graphic())
        || value.contains('#')
    {
        return false;
    }
    let path = value.split('?').next().unwrap_or_default();
    path.strip_prefix("/cloud/app/hosts/")
        .and_then(|path| path.strip_suffix("/standing"))
        .is_some_and(|binding| {
            !binding.is_empty()
                && binding.len() <= 128
                && !matches!(binding, "." | "..")
                && binding
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_- .".contains(&byte))
                && !binding.contains(' ')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn descriptor(endpoint: &str) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"endpoint":endpoint,"identity":format!("sha256:{}", "a".repeat(64))}),
        )
        .unwrap()
    }

    #[test]
    fn resource_endpoints_stay_in_the_exact_same_origin_namespace() {
        for endpoint in [
            "/cloud/app/hosts/test-host/standing",
            "/cloud/app/hosts/test-host/standing?task=task-1&attempt=attempt-2",
        ] {
            assert!(Resource::admit(&descriptor(endpoint)).is_some());
        }
        let long = format!(
            "/cloud/app/hosts/test-host/standing?pin={}",
            "a".repeat(4000)
        );
        assert!(Resource::admit(&descriptor(&long)).is_some());
        let too_long = format!(
            "/cloud/app/hosts/test-host/standing?pin={}",
            "a".repeat(4096)
        );
        assert!(Resource::admit(&descriptor(&too_long)).is_none());
        assert!(Resource::admit(&vec![b' '; MAX_RESOURCE_DESCRIPTOR_BYTES + 1]).is_none());
        for endpoint in [
            "https://other.invalid/cloud/app/hosts/test-host/standing",
            "//other.invalid/cloud/app/hosts/test-host/standing",
            "/cloud/sign-out",
            "/cloud/app/hosts/../standing",
            "/cloud/app/hosts/a/b/standing",
            "/cloud/app/hosts/%2e%2e/standing",
            "/cloud/app/hosts/test-host/standing#fragment",
        ] {
            assert!(Resource::admit(&descriptor(endpoint)).is_none());
        }
    }

    #[test]
    fn unchanged_identity_allows_an_append_only_head_but_replacements_refuse() {
        let resource = Resource::admit(&descriptor(
            "/cloud/app/hosts/test-host/standing?task=task-1",
        ))
        .unwrap();
        let standing = |active, hex: &str| {
            serde_json::to_vec(
                &json!({"active":active,"identity":format!("sha256:{}",hex.repeat(64))}),
            )
            .unwrap()
        };
        assert!(resource.accepts(&standing(true, "a")));
        assert!(resource.accepts(&standing(true, "a")));
        assert!(!resource.accepts(&standing(true, "b")));
        assert!(!resource.accepts(&standing(false, "a")));
        assert!(!resource.accepts(b"{}"));
        assert!(!resource.accepts(&vec![b' '; MAX_RESOURCE_BYTES + 1]));
    }
}
