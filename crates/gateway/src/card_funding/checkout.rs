//! Restricted one-time USD Checkout transport. The billing owner must persist
//! original customer and native quote admission before calling these methods.

use super::*;

pub struct CheckoutRequest {
    pub customer: String,
    pub quote: String,
    pub amount_cents: u64,
    pub expires_at: u64,
    pub return_origin: String,
    pub idempotency: String,
}

pub(super) fn opaque(value: &str) -> Result<(), String> {
    if !(16..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(refusal());
    }
    Ok(())
}

impl Stripe {
    /// Create a metadata-only native customer. No contact, card, or account
    /// label leaves the host. Retry only the original persisted request while
    /// its provider idempotency window remains valid.
    pub async fn create_customer(
        &self,
        reference: &str,
        idempotency: &str,
    ) -> Result<Value, String> {
        opaque(reference)?;
        opaque(idempotency)?;
        self.admitted_account().await?;
        let value = self
            .post(
                "customers",
                &[("metadata[oa_customer]", reference.to_string())],
                idempotency,
            )
            .await?;
        self.check_mode(&value)?;
        identifier(value["id"].as_str().ok_or_else(refusal)?, "cus_")?;
        if value["object"] != "customer"
            || value["metadata"]["oa_customer"] != reference
            || value["deleted"] == true
        {
            return Err(refusal());
        }
        Ok(value)
    }

    /// Open only the original admitted prepaid purchase. This creates neither
    /// a recurring plan nor credit. No refund, capture, or payment method API
    /// is available through this transport.
    pub async fn create_checkout(
        &self,
        request: &CheckoutRequest,
        now: u64,
    ) -> Result<Value, String> {
        identifier(&request.customer, "cus_")?;
        opaque(&request.quote)?;
        opaque(&request.idempotency)?;
        if !(50..=99_999_999).contains(&request.amount_cents)
            || request.return_origin.len() > 2048
            || request.expires_at < now.checked_add(1800).ok_or_else(refusal)?
            || request.expires_at > now.checked_add(86_400).ok_or_else(refusal)?
        {
            return Err(refusal());
        }
        let origin = reqwest::Url::parse(&request.return_origin).map_err(|_| refusal())?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || !matches!(origin.path(), "" | "/")
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.port().is_some_and(|p| p != 443)
        {
            return Err(refusal());
        }
        let success = format!(
            "{}/dashboard/funding/{}",
            request.return_origin.trim_end_matches('/'),
            request.quote
        );
        let fields = [
            ("mode", "payment".to_string()),
            ("customer", request.customer.clone()),
            ("client_reference_id", request.quote.clone()),
            ("metadata[oa_quote]", request.quote.clone()),
            (
                "payment_intent_data[metadata][oa_quote]",
                request.quote.clone(),
            ),
            (
                "payment_intent_data[capture_method]",
                "automatic".to_string(),
            ),
            ("payment_method_types[0]", "card".to_string()),
            ("line_items[0][quantity]", "1".to_string()),
            ("line_items[0][price_data][currency]", "usd".to_string()),
            (
                "line_items[0][price_data][unit_amount]",
                request.amount_cents.to_string(),
            ),
            (
                "line_items[0][price_data][product_data][name]",
                "Prepaid decision usage".to_string(),
            ),
            ("expires_at", request.expires_at.to_string()),
            ("success_url", success.clone()),
            ("cancel_url", success),
        ];
        self.admitted_account().await?;
        let value = self
            .post("checkout/sessions", &fields, &request.idempotency)
            .await?;
        self.check_mode(&value)?;
        let id = value["id"].as_str().ok_or_else(refusal)?;
        identifier(
            id,
            if self.mode == Some(true) {
                "cs_live_"
            } else {
                "cs_test_"
            },
        )?;
        if value["object"] != "checkout.session"
            || value["mode"] != "payment"
            || value["customer"] != request.customer
            || value["currency"] != "usd"
            || value["amount_total"] != request.amount_cents
            || value["client_reference_id"] != request.quote
            || value["metadata"]["oa_quote"] != request.quote
            || value["expires_at"] != request.expires_at
            || !value["subscription"].is_null()
            || !value["setup_intent"].is_null()
        {
            return Err(refusal());
        }
        let url = value["url"].as_str().ok_or_else(refusal)?;
        if url.len() > 8192 {
            return Err(refusal());
        }
        let hosted = reqwest::Url::parse(url).map_err(|_| refusal())?;
        if hosted.scheme() != "https"
            || hosted.host_str() != Some("checkout.stripe.com")
            || !hosted.username().is_empty()
            || hosted.password().is_some()
            || hosted.port().is_some_and(|p| p != 443)
            || hosted.path() != format!("/c/pay/{id}")
        {
            return Err(refusal());
        }
        Ok(value)
    }

    async fn post(
        &self,
        resource: &str,
        fields: &[(&str, String)],
        idempotency: &str,
    ) -> Result<Value, String> {
        if !matches!(resource, "customers" | "checkout/sessions") {
            return Err(refusal());
        }
        self.exchange(
            self.client
                .post(format!("{}/v1/{resource}", self.origin))
                .header("Idempotency-Key", idempotency)
                .form(fields),
        )
        .await
    }
}
