//! Reviewed attribution over the exact native retail ledger. No rights are added.
use crate::{Backend, Error, Result, Service, Store};
use commercial_accounts::{NativeSources, NativeStore};
use openagents_wallet::LightningWallet;
use receipts::purchase::{CommercialProduct, CommercialRef, CommercialSource};
use serde::{Deserialize, Serialize};
use std::{os::unix::fs::MetadataExt, path::PathBuf};
use tenancy::accounts::commercial::{Product, Source};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub canonical_directory: PathBuf,
    pub issuer: String,
    pub native: commercial_accounts::Config,
}
pub(crate) struct Mapping {
    issuer: String,
    sources: NativeSources,
}
impl<B: Backend, W: LightningWallet + Send + Sync + 'static> Service<B, W> {
    /// Enable only an explicit operator mapping over this service's held ledger.
    pub fn with_commercial(mut self, config: Config) -> Result<Self> {
        self.lock()?.check()?;
        let original = std::fs::symlink_metadata(&self.config.ledger)?;
        let matching = config
            .native
            .stores
            .iter()
            .filter(|s| matches!(s, NativeStore::Retail {issuer, ..} if issuer == &config.issuer))
            .collect::<Vec<_>>();
        if config.issuer.is_empty() || matching.len() != 1 {
            return Err(Error::Denied);
        }
        let NativeStore::Retail { ledger, .. } = matching[0] else {
            return Err(Error::Denied);
        };
        let selected = std::fs::symlink_metadata(ledger)?;
        if !original.is_file()
            || !selected.is_file()
            || original.dev() != selected.dev()
            || original.ino() != selected.ino()
        {
            return Err(Error::Denied);
        }
        let sources = NativeSources::open(&config.canonical_directory, &config.native)
            .map_err(|_| Error::Denied)?;
        self.lock()?.check()?;
        self.commercial = Some(Mapping {
            issuer: config.issuer,
            sources,
        });
        Ok(self)
    }
    pub(crate) fn commercial_ref(
        &self,
        store: &Store,
        account: &str,
    ) -> Result<Option<CommercialRef>> {
        store.check()?;
        let Some(mapping) = &self.commercial else {
            return Ok(None);
        };
        let source = Source {
            product: Product::Retail,
            issuer: mapping.issuer.clone(),
            account: account.into(),
            workspace: None,
        };
        let revision = mapping
            .sources
            .selection(&source)
            .map_err(|_| Error::Denied)?
            .ok_or(Error::Denied)?;
        let value = CommercialRef {
            binding: revision.binding,
            revision: revision.revision,
            digest: revision.digest,
            customer: revision.customer,
            workspace: revision.workspace,
            source: CommercialSource {
                product: CommercialProduct::Retail,
                issuer: source.issuer,
                account: source.account,
                workspace: None,
            },
        };
        value.validate().map_err(|_| Error::Denied)?;
        store.check()?;
        Ok(Some(value))
    }
    pub(crate) fn commercial_current(
        &self,
        store: &Store,
        account: &str,
        frozen: Option<&CommercialRef>,
    ) -> bool {
        self.commercial_ref(store, account)
            .is_ok_and(|current| current.as_ref() == frozen)
    }
}
