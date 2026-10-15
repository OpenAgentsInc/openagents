//! On-chain reads and fresh funding addresses.
use crate::{
    Client, Result,
    proto::{lnrpc, walletrpc},
    transport,
};
impl Client {
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
