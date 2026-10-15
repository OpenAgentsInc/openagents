//! Resumable settlement delivery with caller-owned transactional checkpoints.
use crate::{Client, Error, Result, error::unsigned, proto::lnrpc};
use openagents_wallet::{PaymentDirection, PaymentRecord, PaymentStatus};
pub fn record(i: &lnrpc::Invoice) -> Result<PaymentRecord> {
    Ok(PaymentRecord {
        payment_hash: hex::encode(&i.r_hash),
        direction: PaymentDirection::Inbound,
        status: match lnrpc::invoice::InvoiceState::try_from(i.state) {
            Ok(lnrpc::invoice::InvoiceState::Settled) => PaymentStatus::Succeeded,
            Ok(lnrpc::invoice::InvoiceState::Canceled) => PaymentStatus::Failed,
            _ => PaymentStatus::Pending,
        },
        amount_msat: Some(unsigned(i.value_msat)?),
        fee_msat: None,
        preimage: None,
        bolt11: Some(i.payment_request.clone()),
        updated_at: unsigned(i.settle_date.max(i.creation_date))?,
    })
}
/// Implementations must atomically save the effect and its settlement index.
/// The SDK never claims exactly-once effects across separate writes.
pub trait SettlementSink {
    fn position(&self) -> Result<u64>;
    fn commit(&mut self, invoice: &lnrpc::Invoice) -> Result<()>;
}
pub(crate) fn deliver(sink: &mut impl SettlementSink, invoice: &lnrpc::Invoice) -> Result<bool> {
    if invoice.state != lnrpc::invoice::InvoiceState::Settled as i32
        || invoice.settle_index <= sink.position()?
    {
        return Ok(false);
    }
    sink.commit(invoice)?;
    if sink.position()? != invoice.settle_index {
        return Err(Error::Checkpoint);
    }
    Ok(true)
}
impl Client {
    pub async fn settlements(&self, sink: &mut impl SettlementSink, count: usize) -> Result<()> {
        let mut delivered = 0;
        let mut failures = 0;
        while delivered < count {
            let cursor = sink.position()?;
            if cursor == 0 {
                // Zero starts at the live tip and LND delays stream headers
                // until an event. Poll the initial history until a settlement
                // gives us a nonzero replay position; then switch to streaming.
                let mut offset = 0;
                let mut backlog = Vec::new();
                loop {
                    let page = crate::transport::read(|| {
                        let mut rpc = self.rpc();
                        async move {
                            rpc.list_invoices(lnrpc::ListInvoiceRequest {
                                index_offset: offset,
                                num_max_invoices: 1000,
                                ..Default::default()
                            })
                            .await
                        }
                    })
                    .await?;
                    let length = page.invoices.len();
                    backlog.extend(
                        page.invoices
                            .into_iter()
                            .filter(|i| i.state == lnrpc::invoice::InvoiceState::Settled as i32),
                    );
                    if backlog.len() > 100_000 {
                        return Err(Error::Invalid("initial settlement backlog"));
                    }
                    if length < 1000 {
                        break;
                    }
                    if page.last_index_offset <= offset {
                        return Err(Error::Invalid("invoice page position"));
                    }
                    offset = page.last_index_offset;
                }
                backlog.sort_by_key(|i| i.settle_index);
                for i in backlog {
                    if deliver(sink, &i)? {
                        delivered += 1;
                        if delivered == count {
                            return Ok(());
                        }
                    }
                }
            }
            if cursor == 0 {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                continue;
            }
            let subscription = self
                .rpc()
                .subscribe_invoices(lnrpc::InvoiceSubscription {
                    settle_index: cursor,
                    ..Default::default()
                })
                .await;
            let mut stream = match subscription {
                Ok(r) => r.into_inner(),
                Err(s)
                    if matches!(
                        s.code(),
                        tonic::Code::Unavailable | tonic::Code::Cancelled | tonic::Code::Unknown
                    ) =>
                {
                    failures = (failures + 1).min(7);
                    tokio::time::sleep(std::time::Duration::from_millis(
                        (50 << failures.min(7)).min(5000),
                    ))
                    .await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            loop {
                match stream.message().await {
                    Ok(Some(i)) => {
                        if deliver(sink, &i)? {
                            delivered += 1;
                            failures = 0;
                            if delivered == count {
                                return Ok(());
                            }
                        }
                    }
                    Ok(None) => {
                        failures = (failures + 1).min(7);
                        break;
                    }
                    Err(status)
                        if matches!(
                            status.code(),
                            tonic::Code::Unavailable
                                | tonic::Code::Cancelled
                                | tonic::Code::Unknown
                        ) =>
                    {
                        failures = (failures + 1).min(7);
                        break;
                    }
                    Err(status) => return Err(status.into()),
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(
                (50 << failures.min(7)).min(5000),
            ))
            .await;
        }
        Ok(())
    }
}
