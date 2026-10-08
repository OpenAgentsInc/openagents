//! The separately configured native operator cloud owner.

use crate::{Code, Principal};
use coder_access::{Operation, Outcome, cloud::Admission};

/// Every read and effect must also pass the owner's operator policy.
/// Host Observe and Operate rights alone admit no project, pool, or credential.
pub trait Cloud: Send + Sync + std::fmt::Debug {
    fn execute(
        &self,
        request: &str,
        principal: &Principal,
        op: &Operation,
    ) -> Result<Outcome, Code>;
    fn admit_recovery(&self, device: &str, admission: &Admission) -> Result<(), Code>;
}
/// Build a fresh native standing check for workers outside synchronous dispatch.
#[cfg(feature = "host")]
pub fn authority(
    host: coder_access::host::Host,
) -> crate::Result<std::sync::Arc<dyn Fn(&Principal) -> bool + Send + Sync>> {
    let authority = std::sync::Arc::new(crate::authority::Authority::open(host)?);
    Ok(std::sync::Arc::new(move |principal| {
        match (&principal.grant, principal.epoch) {
            (None, None) => authority
                .owner()
                .is_ok_and(|owner| owner == principal.device),
            (Some(grant), Some(epoch)) => coder_access::unix_time().is_ok_and(|now| {
                authority
                    .check(&principal.device, grant, epoch, now)
                    .is_ok_and(|rights| rights.contains(coder_access::Right::Operate))
            }),
            _ => false,
        }
    }))
}
