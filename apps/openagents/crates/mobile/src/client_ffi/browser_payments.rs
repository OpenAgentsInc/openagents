//! Wallet > Payments from your browser (#12306): the phone approves and
//! pays what the person's browser asks for on openagents.com. The logic is
//! `crate::wallet::browser::Approvals`; the shells only draw its card and
//! pass its button presses here.
use super::{CoreAccount, CoreError, CoreResult};
use crate::wallet::browser::{Approvals, Book, BrowserApi};
use coder_access::spend::{Grant, Receipt};
use openagents_account::Controller;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Told whenever the card changes, so the shell draws it again.
#[uniffi::export(with_foreign)]
pub trait BrowserPaymentsListener: Send + Sync {
    fn on_browser_payments_changed(&self);
}

fn failed(message: impl Into<String>) -> CoreError {
    CoreError::InvalidArgument {
        message: message.into(),
    }
}

const SIGNED_OUT: &str = "Log in to answer your browser's payments.";
const UNREACHABLE: &str =
    "We can't reach your browser's payments right now. Try again in a minute.";

/// The site, under the app's session.
struct Site(Arc<Controller>);

impl BrowserApi for Site {
    fn book(&self) -> Pin<Box<dyn Future<Output = Result<Book, String>> + Send + '_>> {
        Box::pin(async move {
            let lease = self.0.lease().ok_or(SIGNED_OUT)?;
            let wallet = lease
                .client()
                .browser_wallet(lease.session())
                .await
                .map_err(|_| UNREACHABLE.to_string())?;
            Ok(Book::from_sdk(wallet))
        })
    }
    fn grant(&self, grant: Grant) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            let lease = self.0.lease().ok_or(SIGNED_OUT)?;
            let grant = serde_json::to_value(grant).map_err(|_| UNREACHABLE.to_string())?;
            lease
                .client()
                .grant_browser_wallet(lease.session(), grant)
                .await
                .map(|_| ())
                .map_err(|_| UNREACHABLE.to_string())
        })
    }
    fn settle(
        &self,
        receipt: Receipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            let lease = self.0.lease().ok_or(SIGNED_OUT)?;
            let id = receipt.request.clone();
            let receipt = serde_json::to_value(receipt).map_err(|_| UNREACHABLE.to_string())?;
            lease
                .client()
                .settle_browser_request(lease.session(), &id, receipt)
                .await
                .map(|_| ())
                .map_err(|_| UNREACHABLE.to_string())
        })
    }
}

/// The browser's payment requests, for this phone's wallet and key.
#[derive(uniffi::Object)]
pub struct CoreBrowserPayments {
    approvals: Arc<Approvals>,
    listener: Arc<dyn BrowserPaymentsListener>,
}

#[uniffi::export]
impl CoreAccount {
    /// Wallet > Payments from your browser, paying through `wallet`.
    pub fn browser_payments(
        &self,
        wallet: Arc<crate::wallet::CoreWallet>,
        listener: Arc<dyn BrowserPaymentsListener>,
    ) -> CoreResult<Arc<CoreBrowserPayments>> {
        let node = wallet
            .node()
            .ok_or_else(|| failed("Open your wallet first."))?;
        let identity = (self.key_source())().ok_or_else(|| failed("Make your ID first."))?;
        let approvals = Approvals::new(
            Arc::new(Site(self.controller.clone())),
            node,
            identity.pubkey_hex(),
            Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs())
            }),
        );
        Ok(Arc::new(CoreBrowserPayments {
            approvals: Arc::new(approvals),
            listener,
        }))
    }
}

#[uniffi::export]
impl CoreBrowserPayments {
    /// The card as Rust Native view JSON.
    pub fn snapshot(&self) -> CoreResult<String> {
        let view = self
            .approvals
            .view()
            .validate()
            .map_err(|_| failed("Couldn't open your payments."))?;
        serde_json::to_string(view.view()).map_err(|_| failed("Couldn't open your payments."))
    }

    /// Read the browser's requests again. The app calls it while the
    /// Wallet is open, every 10 seconds, as it does for computers.
    pub async fn refresh(&self) -> CoreResult<()> {
        let approvals = self.approvals.clone();
        let result = openagents_client::runtime::shared()
            .spawn(async move { approvals.refresh().await })
            .await
            .map_err(|_| failed(UNREACHABLE))?;
        self.listener.on_browser_payments_changed();
        result.map_err(failed)
    }

    /// Press Pay or Decline on a request.
    pub async fn activate(&self, instance: String, revision: u64, key: String) -> CoreResult<()> {
        let _ = (instance, revision);
        let approvals = self.approvals.clone();
        let result = openagents_client::runtime::shared()
            .spawn(async move {
                if let Some(id) = key.strip_prefix("approve:") {
                    approvals.approve(id).await
                } else if let Some(id) = key.strip_prefix("decline:") {
                    approvals.decline(id).await
                } else {
                    Err("Choose Pay or Decline.".to_string())
                }
            })
            .await
            .map_err(|_| failed(UNREACHABLE))?;
        self.listener.on_browser_payments_changed();
        result.map_err(failed)
    }
}
