use std::{fs, str::FromStr};

use async_trait::async_trait;
use config::ConfigError;
use http::{header::CONTENT_TYPE, HeaderValue, Uri};
use hyper::{client::HttpConnector, Client};
use hyper_rustls::HttpsConnector;
use nostr::Keys;
use rand::random;
use serde::{Deserialize, Serialize, Serializer};

use crate::{
    config::Settings,
    error::{Error, Result},
};

use super::{InvoiceInfo, InvoiceStatus, PaymentProcessor};

#[derive(Clone, Debug, Serialize)]
struct ClnInvoiceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    cltv: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deschashonly: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expiry: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preimage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exposeprivatechannels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallbacks: Option<Vec<String>>,
    amount_msat: ClnAmount,
    description: String,
    label: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ClnInvoiceResponse {
    bolt11: String,
    payment_hash: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ClnListInvoicesResponse {
    invoices: Vec<ClnListInvoice>,
}

#[derive(Clone, Debug, Deserialize)]
struct ClnListInvoice {
    status: ClnInvoiceStatus,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ClnInvoiceStatus {
    Unpaid,
    Paid,
    Expired,
}

#[derive(Copy, Clone, Debug)]
struct ClnAmount(u64);

impl ClnAmount {
    fn from_sat(sat: u64) -> Self {
        Self(sat.saturating_mul(1_000))
    }
}

impl Serialize for ClnAmount {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}msat", self.0))
    }
}

#[derive(Clone)]
pub struct ClnRestPaymentProcessor {
    client: hyper::Client<HttpsConnector<HttpConnector>, hyper::Body>,
    settings: Settings,
    rune_header: HeaderValue,
}

impl ClnRestPaymentProcessor {
    pub fn new(settings: &Settings) -> Result<Self> {
        let rune_path = settings
            .pay_to_relay
            .rune_path
            .clone()
            .ok_or(ConfigError::NotFound("rune_path".to_string()))?;
        let rune = String::from_utf8(fs::read(rune_path)?)
            .map_err(|_| ConfigError::Message("Rune should be UTF8".to_string()))?;
        let mut rune_header = HeaderValue::from_str(rune.trim())
            .map_err(|_| ConfigError::Message("Invalid Rune header".to_string()))?;
        rune_header.set_sensitive(true);

        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .https_only()
            .enable_http1()
            .build();
        let client = Client::builder().build::<_, hyper::Body>(https);

        Ok(Self {
            client,
            settings: settings.clone(),
            rune_header,
        })
    }
}

#[async_trait]
impl PaymentProcessor for ClnRestPaymentProcessor {
    async fn get_invoice(&self, key: &Keys, amount: u64) -> Result<InvoiceInfo, Error> {
        let random_number: u16 = random();
        let memo = format!("{}: {}", random_number, key.public_key());

        let body = ClnInvoiceRequest {
            cltv: None,
            deschashonly: None,
            expiry: None,
            preimage: None,
            exposeprivatechannels: None,
            fallbacks: None,
            amount_msat: ClnAmount::from_sat(amount),
            description: memo.clone(),
            label: "Nostr".to_string(),
        };
        let uri = Uri::from_str(&format!(
            "{}/v1/invoice",
            &self.settings.pay_to_relay.node_url
        ))
        .map_err(|_| ConfigError::Message("Bad node URL".to_string()))?;

        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header("Rune", self.rune_header.clone())
            .body(hyper::Body::from(serde_json::to_string(&body)?))
            .expect("request builder");

        let res = self.client.request(req).await?;

        let body = hyper::body::to_bytes(res.into_body()).await?;
        let invoice_response: ClnInvoiceResponse = serde_json::from_slice(&body)?;

        Ok(InvoiceInfo {
            pubkey: key.public_key().to_string(),
            payment_hash: invoice_response.payment_hash,
            bolt11: invoice_response.bolt11,
            amount,
            memo,
            status: InvoiceStatus::Unpaid,
            confirmed_at: None,
        })
    }

    async fn check_invoice(&self, payment_hash: &str) -> Result<InvoiceStatus, Error> {
        let uri = Uri::from_str(&format!(
            "{}/v1/listinvoices?payment_hash={}",
            &self.settings.pay_to_relay.node_url, payment_hash
        ))
        .map_err(|_| ConfigError::Message("Bad node URL".to_string()))?;

        let req = hyper::Request::builder()
            .method(hyper::Method::POST)
            .uri(uri)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header("Rune", self.rune_header.clone())
            .body(hyper::Body::empty())
            .expect("request builder");

        let res = self.client.request(req).await?;

        let body = hyper::body::to_bytes(res.into_body()).await?;
        let invoice_response: ClnListInvoicesResponse = serde_json::from_slice(&body)?;
        let invoice = invoice_response
            .invoices
            .first()
            .ok_or(Error::CustomError("Invoice not found".to_string()))?;
        let status = match invoice.status {
            ClnInvoiceStatus::Paid => InvoiceStatus::Paid,
            ClnInvoiceStatus::Unpaid => InvoiceStatus::Unpaid,
            ClnInvoiceStatus::Expired => InvoiceStatus::Expired,
        };
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClnAmount, ClnInvoiceRequest, ClnInvoiceStatus, ClnListInvoicesResponse};

    #[test]
    fn invoice_request_serializes_amount_in_msat() {
        let request = ClnInvoiceRequest {
            cltv: None,
            deschashonly: None,
            expiry: None,
            preimage: None,
            exposeprivatechannels: None,
            fallbacks: None,
            amount_msat: ClnAmount::from_sat(42),
            description: "memo".to_string(),
            label: "label".to_string(),
        };

        let json = serde_json::to_value(request).expect("request should serialize");
        assert_eq!(json["amount_msat"], "42000msat");
    }

    #[test]
    fn list_invoices_status_deserializes_lowercase_values() {
        let response: ClnListInvoicesResponse =
            serde_json::from_str(r#"{"invoices":[{"status":"paid"}]}"#)
                .expect("response should deserialize");
        assert_eq!(response.invoices[0].status, ClnInvoiceStatus::Paid);
    }
}
