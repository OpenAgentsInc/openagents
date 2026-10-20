//! Opt-in, in-memory Spark wallet. Recovery words never cross this API's local SDK boundary.
use crate::model::{PaymentRow, sdk_payment_row};
use breez_sdk_spark::*;

pub struct BrowserWallet {
    sdk: BreezSdk,
    uncertain: bool,
    prepared: Option<(PrepareSendPaymentResponse, String, bool)>,
}
pub struct BrowserQuote {
    pub amount_sats: u64,
    pub fee_sats: u64,
}
impl BrowserWallet {
    /// Restore only after the person opts into browser custody. No filesystem or browser storage.
    pub async fn restore(words: &str) -> Result<Self, String> {
        let phrase = bip39::Mnemonic::parse_in(bip39::Language::English, words)
            .map_err(|_| "Check your 12 recovery words and their order.".to_owned())?;
        if phrase.word_count() != 12 {
            return Err("Enter your 12 recovery words.".into());
        }
        let sdk = SdkBuilder::new(
            crate::config::sdk_config(Network::Mainnet),
            Seed::Mnemonic {
                mnemonic: phrase.to_string(),
                passphrase: None,
            },
        )
        .with_storage_backend(crate::store::memory_backend())
        .build()
        .await
        .map_err(|_| {
            "We couldn't open your wallet. Check your connection and try again.".to_owned()
        })?;
        Ok(Self {
            sdk,
            uncertain: false,
            prepared: None,
        })
    }
    pub async fn balance(&self) -> Result<u64, String> {
        self.sdk
            .get_info(GetInfoRequest {
                ensure_synced: Some(true),
            })
            .await
            .map(|r| r.balance_sats)
            .map_err(|_| "We couldn't refresh your wallet.".into())
    }
    pub async fn payments(&mut self) -> Result<Vec<PaymentRow>, String> {
        let rows = self
            .sdk
            .list_payments(ListPaymentsRequest {
                limit: Some(20),
                ..Default::default()
            })
            .await
            .map(|r| r.payments.iter().map(sdk_payment_row).collect::<Vec<_>>())
            .map_err(|_| "We couldn't read your payments.".to_owned())?;
        if let Some((_, id, _)) = &self.prepared {
            if rows
                .iter()
                .any(|row| row.id == *id && (row.status == "completed" || row.status == "failed"))
            {
                self.uncertain = false;
                self.prepared = None;
            }
        }
        Ok(rows)
    }
    pub async fn invoice(&self, amount_sats: u64) -> Result<String, String> {
        if amount_sats == 0 || amount_sats > bitcoin_amount::MAX_SUPPLY {
            return Err("Choose a valid amount.".into());
        }
        self.sdk
            .receive_payment(ReceivePaymentRequest {
                payment_method: ReceivePaymentMethod::Bolt11Invoice {
                    description: "OpenAgents wallet".into(),
                    amount_sats: Some(amount_sats),
                    expiry_secs: None,
                    payment_hash: None,
                    receiver_identity_public_key: None,
                },
            })
            .await
            .map(|r| r.payment_request)
            .map_err(|_| "We couldn't create that payment request.".into())
    }
    /// This browser flow accepts Lightning invoices only and always quotes before sending.
    pub async fn quote(&mut self, invoice: &str) -> Result<BrowserQuote, String> {
        if self.uncertain {
            return Err(
                "Check your history or retry the original payment confirmation first.".into(),
            );
        }
        self.prepared = None;
        let invoice = invoice
            .trim()
            .strip_prefix("lightning:")
            .unwrap_or(invoice.trim());
        if invoice.len() > 16384 || !invoice.to_ascii_lowercase().starts_with("lnbc") {
            return Err("Enter a Bitcoin Lightning invoice.".into());
        }
        let prepared = self
            .sdk
            .prepare_send_payment(PrepareSendPaymentRequest {
                payment_request: PaymentRequest::Input {
                    input: invoice.into(),
                },
                amount: None,
                token_identifier: None,
                conversion_options: None,
                fee_policy: None,
            })
            .await
            .map_err(|_| {
                "We couldn't review that payment. Check the invoice and your balance.".to_owned()
            })?;
        if prepared.token_identifier.is_some() || prepared.conversion_estimate.is_some() {
            return Err("Enter a Bitcoin Lightning invoice.".into());
        }
        let SendPaymentMethod::Bolt11Invoice {
            spark_transfer_fee_sats,
            lightning_fee_sats,
            ..
        } = &prepared.payment_method
        else {
            return Err("Enter a Bitcoin Lightning invoice.".into());
        };
        let amount_sats =
            u64::try_from(prepared.amount).map_err(|_| "That amount is too large.".to_owned())?;
        let (fee_sats, prefer_spark) =
            crate::config::lightning_route(*lightning_fee_sats, *spark_transfer_fee_sats);
        self.prepared = Some((prepared, uuid::Uuid::new_v4().to_string(), prefer_spark));
        Ok(BrowserQuote {
            amount_sats,
            fee_sats,
        })
    }
    /// Repeating an uncertain send keeps its original idempotency key and prepared payment.
    pub fn cancel_quote(&mut self) {
        if !self.uncertain {
            self.prepared = None;
        }
    }
    pub fn uncertain(&self) -> bool {
        self.uncertain
    }
    pub async fn pay(&mut self) -> Result<PaymentRow, String> {
        let (prepared, key, prefer_spark) =
            self.prepared.as_ref().ok_or("Review the payment first.")?;
        match self
            .sdk
            .send_payment(SendPaymentRequest {
                prepare_response: prepared.clone(),
                options: Some(SendPaymentOptions::Bolt11Invoice {
                    prefer_spark: *prefer_spark,
                    completion_timeout_secs: Some(30),
                }),
                idempotency_key: Some(key.clone()),
            })
            .await
        {
            Ok(r) => {
                self.prepared = None;
                self.uncertain = false;
                Ok(sdk_payment_row(&r.payment))
            }
            Err(_) => {
                self.uncertain = true;
                Err("We couldn't confirm the payment. Refresh your history or retry this confirmation.".into())
            }
        }
    }
    pub async fn disconnect(&mut self) -> Result<(), String> {
        self.prepared = None;
        self.sdk
            .disconnect()
            .await
            .map_err(|_| "Close this tab to finish disconnecting.".into())
    }
}
