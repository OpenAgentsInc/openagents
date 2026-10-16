//! Non-spending access to an HTTP 402 challenge. Receiving a challenge never
//! validates an offer or authorizes a payment.

use std::fmt;

use reqwest::header::HeaderMap;

/// The same encoded-header limit used by the existing x402 HTTP codec.
pub const MAX_PAYMENT_REQUIRED_HEADER_BYTES: usize = 64 * 1024;

/// Why an HTTP 402 did not supply one usable text header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaymentRequiredHeaderError {
    Missing,
    Repeated,
    TooLarge,
    InvalidText,
}

/// An untrusted `PAYMENT-REQUIRED` header from an HTTP 402 response.
///
/// The SDK preserves the header without parsing, paying, or following any URL
/// it contains. An application must validate the offer, invoice, and actual
/// request through its separately authorized payment integration before spending.
/// Debug output omits the header, which can contain private request details.
#[derive(Clone, PartialEq, Eq)]
pub struct PaymentRequired {
    header: Result<String, PaymentRequiredHeaderError>,
}

impl PaymentRequired {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        let mut values = headers.get_all("payment-required").iter();
        let header = match values.next() {
            None => Err(PaymentRequiredHeaderError::Missing),
            Some(_) if values.next().is_some() => Err(PaymentRequiredHeaderError::Repeated),
            Some(value) if value.as_bytes().len() > MAX_PAYMENT_REQUIRED_HEADER_BYTES => {
                Err(PaymentRequiredHeaderError::TooLarge)
            }
            Some(value) => value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| PaymentRequiredHeaderError::InvalidText),
        };
        Self { header }
    }

    /// The exact text header value, without decoding or offer validation.
    /// Missing, repeated, oversized, or non-text headers remain typed HTTP 402
    /// errors, but their contents are not retained.
    pub fn header(&self) -> Result<&str, PaymentRequiredHeaderError> {
        self.header.as_deref().map_err(|error| *error)
    }
}

impl fmt::Debug for PaymentRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PaymentRequired")
            .field("header_error", &self.header.as_ref().err())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PaymentRequired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Payment is required. No payment was sent.")
    }
}
