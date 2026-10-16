//! On-chain reads and fresh funding addresses.
use crate::{
    Client, Result,
    proto::{lnrpc, walletrpc},
    transport,
};
impl Client {
    /// Index the original first-channel public key after the signet SCB restore.
    /// Callers must save their mutation marker and check watch-only account counts.
    #[cfg(feature = "supervisor")]
    pub async fn index_signet_research_first_channel_key(&self, expected: [u8; 33]) -> Result<()> {
        if self.network() != "signet" {
            return Err(crate::Error::PermissionDenied);
        }
        self.ensure_current().await?;
        let mut rpc = walletrpc::wallet_kit_client::WalletKitClient::new(self.channel.clone());
        let locator = crate::proto::signrpc::KeyLocator {
            key_family: 0,
            key_index: 0,
        };
        let derived = transport::once(rpc.derive_key(locator)).await?;
        if derived.raw_key_bytes != expected || derived.key_loc != Some(locator) {
            return Err(crate::Error::Invalid("original research channel key"));
        }
        let indexed = transport::once(rpc.derive_next_key(walletrpc::KeyReq {
            key_family: 0,
            key_finger_print: 0,
        }))
        .await?;
        if indexed.raw_key_bytes != expected || indexed.key_loc != Some(locator) {
            return Err(crate::Error::Invalid("indexed research channel key"));
        }
        Ok(())
    }
    pub async fn new_address(&self) -> Result<String> {
        // Allocating an address is a mutation; do not retry it implicitly.
        self.ensure_current().await?;
        Ok(
            transport::once(self.rpc().new_address(lnrpc::NewAddressRequest::default()))
                .await?
                .address,
        )
    }
    pub async fn list_unspent(&self, min_confs: i32) -> Result<walletrpc::ListUnspentResponse> {
        transport::read(|| {
            let mut rpc = walletrpc::wallet_kit_client::WalletKitClient::new(self.channel.clone());
            async move {
                rpc.list_unspent(walletrpc::ListUnspentRequest {
                    min_confs,
                    max_confs: i32::MAX,
                    ..Default::default()
                })
                .await
            }
        })
        .await
    }
}
