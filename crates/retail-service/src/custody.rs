//! Refuse replaced service custody before and after blocking collaborators.
use crate::store::Custody;
use openagents_wallet::*;
use retail_cloud::{
    authority::Source,
    cancel::{StopEvidence, StopOwner},
    dispatch::{DispatchSpec, OwnerError, TaskEvent, TaskOwner, TaskStatus},
    material::Sandbox,
    provision::{CreateSpec, Provider, ProviderError, Resource, ResourceState},
    retain::{Artifacts, Manifest},
};
use std::sync::Arc;
use std::time::Duration;

pub(crate) struct GuardBackend<B> {
    pub inner: Arc<B>,
    pub custody: Arc<Custody>,
}
macro_rules! method {
    ($name:ident($($arg:ident:$ty:ty),*) -> $ret:ty, $error:expr) => {
        fn $name(&self,$($arg:$ty),*) -> $ret {
            self.custody.check().map_err(|_|$error)?;
            let result=self.inner.$name($($arg),*);
            self.custody.check().map_err(|_|$error)?;
            result
        }
    };
}
impl<B: Provider> Provider for GuardBackend<B> {
    method!(create(spec:&CreateSpec)->std::result::Result<Resource,ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
    method!(find(id:&str)->std::result::Result<Option<Resource>,ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
    method!(reconcile_creation(spec:&CreateSpec,now:i64)->std::result::Result<Option<Resource>,ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
    method!(state(id:&str)->std::result::Result<ResourceState,ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
    method!(delete(id:&str)->std::result::Result<(),ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
    method!(usage_seconds(id:&str)->std::result::Result<Option<u64>,ProviderError>,ProviderError::Unknown("private retail custody changed".into()));
}
impl<B: Sandbox> Sandbox for GuardBackend<B> {
    method!(write_private(r:&str,p:&str,c:&str)->retail_cloud::Result<bool>,retail_cloud::Error::Conflict("private retail custody changed"));
    method!(clone_source(r:&str,s:&Source)->retail_cloud::Result<(String,bool)>,retail_cloud::Error::Conflict("private retail custody changed"));
    method!(remove(r:&str,p:&str)->retail_cloud::Result<()>,retail_cloud::Error::Conflict("private retail custody changed"));
    method!(exists(r:&str,p:&str)->retail_cloud::Result<bool>,retail_cloud::Error::Conflict("private retail custody changed"));
}
impl<B: TaskOwner> TaskOwner for GuardBackend<B> {
    method!(submit(r:&str,s:&DispatchSpec)->std::result::Result<(),OwnerError>,OwnerError::Unknown("private retail custody changed".into()));
    method!(status(r:&str,t:&str)->std::result::Result<Option<TaskStatus>,OwnerError>,OwnerError::Unknown("private retail custody changed".into()));
    method!(events(r:&str,t:&str,c:u64)->std::result::Result<Vec<TaskEvent>,OwnerError>,OwnerError::Unknown("private retail custody changed".into()));
}
impl<B: StopOwner> StopOwner for GuardBackend<B> {
    method!(stop(r:&str,t:&str,k:&str)->std::result::Result<StopEvidence,OwnerError>,OwnerError::Unknown("private retail custody changed".into()));
    method!(stopped(r:&str,t:&str,k:&str)->std::result::Result<Option<StopEvidence>,OwnerError>,OwnerError::Unknown("private retail custody changed".into()));
}
impl<B: Artifacts> Artifacts for GuardBackend<B> {
    method!(manifest(r:&str,t:&str)->retail_cloud::Result<Manifest>,retail_cloud::Error::Conflict("private retail custody changed"));
    method!(read(r:&str,t:&str,n:&str,max:usize)->retail_cloud::Result<Vec<u8>>,retail_cloud::Error::Conflict("private retail custody changed"));
}
pub(crate) struct GuardWallet<W> {
    pub inner: Arc<W>,
    pub custody: Arc<Custody>,
    pub node: String,
}
impl<W: LightningWallet> GuardWallet<W> {
    fn check(&self) -> std::result::Result<(), WalletError> {
        self.custody
            .check()
            .map_err(|_| WalletError::Setup("private retail custody changed".into()))
    }
}
impl<W: LightningWallet> LightningWallet for GuardWallet<W> {
    fn node_id(&self) -> String {
        self.node.clone()
    }
    fn receive_exact(
        &self,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> std::result::Result<IssuedInvoice, WalletError> {
        self.check()?;
        let result = self
            .inner
            .receive_exact_from_node(&self.node, amount, hash, expiry);
        self.check()?;
        let issued = result?;
        let encoded = hash.iter().map(|b| format!("{b:02x}")).collect::<String>();
        if issued.amount_msat != amount
            || issued.pay_to != self.node
            || issued.description_hash != encoded
            || issued.expiry_secs != expiry
            || parse_hash32(&issued.payment_hash).is_err()
        {
            return Err(WalletError::Invalid(
                "the issued invoice differs from the exact receiver request".into(),
            ));
        }
        Ok(issued)
    }
    fn lookup(&self, hash: [u8; 32]) -> std::result::Result<Option<PaymentRecord>, WalletError> {
        self.check()?;
        let result = self.inner.lookup_from_node(&self.node, hash);
        self.check()?;
        let record = result?;
        if let Some(r) = &record {
            if parse_hash32(&r.payment_hash)? != hash
                || r.direction != PaymentDirection::Inbound
                || r.preimage.as_ref().is_some_and(|p| {
                    parse_hash32(p).map_or(true, |bytes| {
                        retail_cloud::sha256_hex(&bytes) != r.payment_hash
                    })
                })
            {
                return Err(WalletError::Invalid(
                    "the receiver record has another payment identity or direction".into(),
                ));
            }
        }
        Ok(record)
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> std::result::Result<Proof, WalletError> {
        Err(WalletError::Invalid(
            "the retail receiver has no payment authority".into(),
        ))
    }
    fn balance(&self) -> std::result::Result<Balance, WalletError> {
        Err(WalletError::Invalid(
            "retail balance reads use the central compute ledger".into(),
        ))
    }
    fn channels(&self) -> std::result::Result<Vec<Channel>, WalletError> {
        Err(WalletError::Invalid(
            "retail receiver does not expose channels".into(),
        ))
    }
    fn funding_address(&self) -> std::result::Result<String, WalletError> {
        Err(WalletError::Invalid(
            "retail receiver does not expose chain funding".into(),
        ))
    }
    fn open_channel(
        &self,
        _: &str,
        _: &str,
        _: u64,
        _: bool,
    ) -> std::result::Result<String, WalletError> {
        Err(WalletError::Invalid(
            "retail receiver does not control channels".into(),
        ))
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> std::result::Result<(), WalletError> {
        Err(WalletError::Invalid(
            "retail receiver does not control channels".into(),
        ))
    }
}
