//! NIP-ENV records over the existing `coder-ssh` launcher. The remote
//! account's records decide managed versus external; this adapter never
//! relabels a host, never stops an external one, and keeps the SSH lifetime
//! rules: client exit and tunnel loss are observational.
use crate::{
    Result,
    provider::{Cost, Create, Lookup, Provider, Realized, Teardown},
};
use coder_ssh::{Host, Launcher, Ownership, Removal, Start};
use nostr::contracts::jcs;
use serde_json::json;

/// The launcher operations this adapter uses. [`Launcher`] implements it.
pub trait SshHosts {
    fn up(&mut self) -> std::result::Result<Host, String>;
    fn remove(&mut self) -> std::result::Result<Removal, String>;
}

impl SshHosts for Launcher {
    fn up(&mut self) -> std::result::Result<Host, String> {
        Launcher::up(self).map_err(|e| e.to_string())
    }
    fn remove(&mut self) -> std::result::Result<Removal, String> {
        Launcher::remove(self).map_err(|e| e.to_string())
    }
}

pub struct SshEnvironments<H> {
    hosts: H,
    destination: String,
}

impl<H: SshHosts> SshEnvironments<H> {
    /// `destination` is the host logical account: the SSH destination the
    /// owner independently authorized for this allocator.
    pub fn new(hosts: H, destination: impl Into<String>) -> Self {
        Self {
            hosts,
            destination: destination.into(),
        }
    }
    fn pid(resource: &Realized) -> Result<u32> {
        resource
            .resource
            .strip_prefix("pid:")
            .and_then(|pid| pid.parse().ok())
            .ok_or_else(|| "malformed: SSH resource identity".into())
    }
    fn receipt(&self, action: &str, resource: &Realized, observed: &str) -> Vec<u8> {
        jcs(
            &json!({"provider":"coder-ssh","account":self.destination,"action":action,
            "resource":resource.resource,"generation":resource.generation,"observed":observed}),
        )
        .unwrap_or_default()
    }
}

impl<H: SshHosts> Provider for SshEnvironments<H> {
    fn binding(&self) -> String {
        "environment/coder-ssh".into()
    }
    fn create(&mut self, account: &str, key: &str) -> Create {
        if account != self.destination {
            return Create::Refused("not_admitted: SSH account differs from this binding".into());
        }
        let host = match self.hosts.up() {
            Ok(host) => host,
            // A failed launch may have started a host; it is not "absent".
            Err(e) => return Create::Unknown(e),
        };
        if host.ownership == Ownership::External || host.start == Start::Adopted {
            // Using an external host is observational, so nothing to undo.
            return Create::Refused(
                "conflict: an external host already runs for this account; it is never \
                 relabeled as created"
                    .into(),
            );
        }
        let marker = json!({"v":"openagents.environment-ownership.v1","requires":[],
            "provider":self.binding(),"account":account,"resource":format!("pid:{}",host.pid),
            "generation":format!("{}:{}",host.version,host.pid),"key":key,"observed_at":crate::now()});
        match jcs(&marker) {
            Ok(ownership) => Create::Created(
                Realized {
                    resource: format!("pid:{}", host.pid),
                    generation: format!("{}:{}", host.version, host.pid),
                    account: account.into(),
                    ownership,
                },
                Cost::Unknown,
            ),
            Err(e) => Create::Unknown(e.to_string()),
        }
    }
    fn lookup(&mut self, _account: &str, _key: &str) -> Lookup {
        // `up` is the launcher's only probe and it can start a host, so a lost
        // create stays unknown until the owner reconciles it explicitly.
        Lookup::Unknown("the SSH launcher has no side-effect-free probe".into())
    }
    fn adopt(&mut self, _: &str, _: &str, _: &str, _: &[u8]) -> Result<Realized> {
        Err(
            "cannot_enforce: SSH adoption needs a side-effect-free probe; \
             `coder-ssh` adopts external hosts on its own path"
                .into(),
        )
    }
    fn materialize(
        &mut self,
        resource: &Realized,
        components: &[(String, Vec<u8>)],
    ) -> Result<Vec<(String, String)>> {
        // The launcher installs only its pinned release archive. Report that
        // observed digest; other components stay missing and refuse readiness.
        let version = resource
            .generation
            .split(':')
            .next()
            .ok_or("malformed: SSH generation")?;
        Ok(components
            .iter()
            .filter(|(_, bytes)| {
                nostr::contracts::digest_bytes(bytes) == format!("sha256:{version}")
            })
            .map(|(id, _)| (id.clone(), format!("sha256:{version}")))
            .collect())
    }
    fn collect(&mut self, _resource: &Realized) -> Result<Vec<(String, Vec<u8>)>> {
        Ok(Vec::new())
    }
    fn destroy(&mut self, resource: &Realized) -> Teardown {
        let expected = match Self::pid(resource) {
            Ok(pid) => pid,
            Err(e) => return Teardown::Failed(e),
        };
        match self.hosts.remove() {
            Ok(Removal::Stopped { pid }) if pid == expected => {
                Teardown::Confirmed(self.receipt("destroy", resource, "stopped"), Cost::Unknown)
            }
            Ok(Removal::Stopped { pid }) => Teardown::Unknown(format!(
                "a different managed host ({pid}) was stopped; the leased one is unaccounted"
            )),
            Ok(Removal::Detached { .. }) => {
                Teardown::Failed("the host is external; it keeps running".into())
            }
            Ok(Removal::Absent) => {
                Teardown::Unknown("no host was running; its stop is unattributed".into())
            }
            Err(e) => Teardown::Unknown(e),
        }
    }
    fn detach(&mut self, resource: &Realized) -> Teardown {
        // Detaching changes nothing remote; the host keeps running.
        Teardown::Confirmed(
            self.receipt("detach", resource, "running"),
            Cost::Measured(0),
        )
    }
}
