//! Payment sends are attempted once. Interrupted sends are recovered by hash.
use crate::{
    Client, Error, Result,
    error::{signed, unsigned},
    proto::{lnrpc, routerrpc},
    transport,
};
use openagents_wallet::{PaymentDirection, PaymentRecord, PaymentStatus, Proof, WalletError};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub fn invoice(text: &str) -> std::result::Result<lightning_invoice::Bolt11Invoice, WalletError> {
    text.parse()
        .map_err(|_| WalletError::Invalid("invalid Lightning invoice".into()))
}
pub(crate) fn record(p: &lnrpc::Payment) -> Result<PaymentRecord> {
    let hash = hex::decode(&p.payment_hash).map_err(|_| Error::Invalid("payment hash"))?;
    if hash.len() != 32 {
        return Err(Error::Invalid("payment hash"));
    }
    let status = match lnrpc::payment::PaymentStatus::try_from(p.status) {
        Ok(lnrpc::payment::PaymentStatus::Succeeded) => PaymentStatus::Succeeded,
        Ok(lnrpc::payment::PaymentStatus::Failed) => PaymentStatus::Failed,
        Ok(lnrpc::payment::PaymentStatus::InFlight | lnrpc::payment::PaymentStatus::Initiated) => {
            PaymentStatus::Pending
        }
        _ => return Err(Error::InFlight),
    };
    if status == PaymentStatus::Succeeded {
        let preimage = hex::decode(&p.payment_preimage).map_err(|_| Error::Invalid("preimage"))?;
        if preimage.len() != 32 || Sha256::digest(&preimage).as_slice() != hash {
            return Err(Error::Invalid("payment proof"));
        }
    }
    Ok(PaymentRecord {
        payment_hash: p.payment_hash.clone(),
        direction: PaymentDirection::Outbound,
        status,
        amount_msat: Some(unsigned(p.value_msat)?),
        fee_msat: Some(unsigned(p.fee_msat)?),
        preimage: if status == PaymentStatus::Succeeded {
            Some(p.payment_preimage.clone())
        } else {
            None
        },
        bolt11: Some(p.payment_request.clone()),
        updated_at: unsigned(p.creation_time_ns)?.saturating_div(1_000_000_000),
    })
}
pub(crate) fn failure(reason: i32) -> Error {
    match lnrpc::PaymentFailureReason::try_from(reason) {
        Ok(lnrpc::PaymentFailureReason::FailureReasonNoRoute) => Error::NoRoute,
        Ok(lnrpc::PaymentFailureReason::FailureReasonInsufficientBalance) => {
            Error::InsufficientBalance
        }
        Ok(lnrpc::PaymentFailureReason::FailureReasonIncorrectPaymentDetails) => {
            Error::IncorrectPaymentDetails
        }
        Ok(lnrpc::PaymentFailureReason::FailureReasonTimeout) => Error::Timeout,
        _ => Error::Refused,
    }
}
impl Client {
    fn router(&self) -> routerrpc::router_client::RouterClient<transport::AuthChannel> {
        routerrpc::router_client::RouterClient::new(self.channel.clone())
    }
    pub async fn track(&self, hash: [u8; 32]) -> Result<lnrpc::Payment> {
        let mut stream = transport::read(|| {
            let mut rpc = self.router();
            async move {
                rpc.track_payment_v2(routerrpc::TrackPaymentRequest {
                    payment_hash: hash.to_vec(),
                    no_inflight_updates: false,
                })
                .await
            }
        })
        .await?;
        let p = stream
            .message()
            .await
            .map_err(Error::from)?
            .ok_or(Error::Unavailable)?;
        if p.payment_hash != hex::encode(hash) {
            return Err(Error::Invalid("payment identity"));
        }
        Ok(p)
    }
    pub async fn send_payment(
        &self,
        invoice_text: &str,
        max_fee_msat: u64,
        wait: Duration,
    ) -> Result<Proof> {
        let decoded = invoice(invoice_text).map_err(|_| Error::Invalid("invoice"))?;
        if decoded.currency() != self.invoice_currency() {
            return Err(Error::Invalid("invoice network"));
        }
        self.ensure_current().await?;

        let amount = decoded
            .amount_milli_satoshis()
            .filter(|a| *a > 0)
            .ok_or(Error::Invalid("invoice amount"))?;
        let hash: [u8; 32] = hex::decode(decoded.payment_hash().to_string())
            .unwrap()
            .try_into()
            .unwrap();
        let mut rpc = self.router();
        let request = routerrpc::SendPaymentRequest {
            payment_request: invoice_text.into(),
            fee_limit_msat: signed(max_fee_msat, "fee cap")?,
            timeout_seconds: i32::try_from(wait.as_secs().max(1))
                .map_err(|_| Error::Invalid("payment timeout"))?,
            ..Default::default()
        };
        // No read retry helper here: the RPC may already have dispatched.
        let outcome = tokio::time::timeout(wait, async {
            let mut stream = match rpc.send_payment_v2(request).await {
                Ok(response) => response.into_inner(),
                Err(status) if status.code() == tonic::Code::AlreadyExists => {
                    return completed_proof(
                        &self.track(hash).await?,
                        invoice_text,
                        amount,
                        max_fee_msat,
                    );
                }
                Err(status) => return Err(status.into()),
            };
            while let Some(p) = stream.message().await.map_err(Error::from)? {
                if p.payment_hash != hex::encode(hash) {
                    return Err(Error::Invalid("payment identity"));
                }
                match record(&p)?.status {
                    PaymentStatus::Succeeded => {
                        return completed_proof(&p, invoice_text, amount, max_fee_msat);
                    }
                    PaymentStatus::Failed => return Err(failure(p.failure_reason)),
                    PaymentStatus::Pending => {}
                }
            }
            Err(Error::InFlight)
        })
        .await;
        outcome.unwrap_or(Err(Error::Timeout))
    }
    pub async fn estimate_route_fee(
        &self,
        invoice: String,
        timeout: u32,
    ) -> Result<routerrpc::RouteFeeResponse> {
        transport::read(|| {
            let mut rpc = self.router();
            let invoice = invoice.clone();
            async move {
                rpc.estimate_route_fee(routerrpc::RouteFeeRequest {
                    payment_request: invoice,
                    timeout,
                    ..Default::default()
                })
                .await
            }
        })
        .await
    }
}

fn completed_proof(
    p: &lnrpc::Payment,
    invoice_text: &str,
    amount: u64,
    max_fee_msat: u64,
) -> Result<Proof> {
    match record(p)?.status {
        PaymentStatus::Pending => return Err(Error::InFlight),
        PaymentStatus::Failed => return Err(failure(p.failure_reason)),
        PaymentStatus::Succeeded => {}
    }
    if unsigned(p.value_msat)? != amount || unsigned(p.fee_msat)? > max_fee_msat {
        return Err(Error::Invalid("payment terms"));
    }
    Ok(Proof {
        payment_hash: p.payment_hash.clone(),
        preimage: p.payment_preimage.clone(),
        amount_msat: amount,
        fee_msat: unsigned(p.fee_msat)?,
        bolt11: invoice_text.into(),
    })
}
