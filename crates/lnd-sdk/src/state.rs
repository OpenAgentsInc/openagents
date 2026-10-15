//! Readiness without wallet-password or seed access.
use crate::{Client, Result, proto::lnrpc, transport};
impl Client {
    pub async fn state(&self) -> Result<lnrpc::WalletState> {
        let response = transport::read(|| {
            let mut rpc = lnrpc::state_client::StateClient::new(self.channel.clone());
            async move { rpc.get_state(lnrpc::GetStateRequest {}).await }
        })
        .await?;
        lnrpc::WalletState::try_from(response.state).map_err(|_| crate::Error::NotSynced)
    }
}
