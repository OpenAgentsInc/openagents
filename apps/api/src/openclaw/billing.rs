use worker::{Env, Result};

use crate::openclaw::convex::BillingSummary;

pub async fn get_summary(env: &Env, user_id: &str) -> Result<BillingSummary> {
    crate::openclaw::convex::get_billing_summary(env, user_id).await
}
