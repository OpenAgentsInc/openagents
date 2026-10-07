//! Session-bound approval for the native prepaid browser forms.
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FundingForm {
    pub action: String,
    pub door: String,
    pub id: String,
    #[serde(default)]
    pub gross_cents: u64,
    #[serde(default)]
    pub approved: String,
    pub expires: u64,
    pub csrf: String,
}
fn message(workspace: &str, form: &FundingForm) -> Vec<u8> {
    // Structured encoding prevents delimiters in caller input from aliasing another approval.
    // Quoting proposes terms and charges nothing; its amount is a user input.
    // Checkout approval instead binds the returned digest of all admitted terms.
    let amount = if form.action == "quote" {
        0
    } else {
        form.gross_cents
    };
    serde_json::to_vec(&(
        workspace,
        &form.action,
        &form.door,
        &form.id,
        amount,
        &form.approved,
        form.expires,
    ))
    .expect("Native form values serialize.")
}
fn mac(headers: &HeaderMap, workspace: &str, form: &FundingForm) -> Result<Hmac<Sha256>, Response> {
    let token = crate::dashboard::cookie_token(headers).ok_or_else(|| {
        crate::dashboard::page_error(
            StatusCode::UNAUTHORIZED,
            "Sign-in required",
            "Sign in before approving checkout.",
        )
    })?;
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(token.as_bytes())
        .expect("HMAC accepts a session token.");
    mac.update(&message(workspace, form));
    Ok(mac)
}
pub(crate) fn sign(
    headers: &HeaderMap,
    workspace: &str,
    form: &FundingForm,
) -> Result<String, Response> {
    Ok(mac(headers, workspace, form)?
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
pub(crate) fn check(
    headers: &HeaderMap,
    workspace: &str,
    form: &FundingForm,
    now: u64,
) -> Result<(), Response> {
    let invalid = || {
        crate::dashboard::page_error(
            StatusCode::FORBIDDEN,
            "Approval expired or changed",
            "Refresh the billing page and review the original checkout terms.",
        )
    };
    if form.expires < now || form.expires > now.saturating_add(300) || form.csrf.len() != 64 {
        return Err(invalid());
    }
    let bytes = form
        .csrf
        .as_bytes()
        .chunks_exact(2)
        .map(|p| {
            let h = std::str::from_utf8(p).map_err(|_| ())?;
            u8::from_str_radix(h, 16).map_err(|_| ())
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| invalid())?;
    mac(headers, workspace, form)?
        .verify_slice(&bytes)
        .map_err(|_| invalid())
}
pub(crate) fn authorization(headers: &HeaderMap, workspace: &str) -> Result<HeaderMap, Response> {
    let token = crate::dashboard::cookie_token(headers).ok_or_else(|| {
        crate::dashboard::page_error(
            StatusCode::UNAUTHORIZED,
            "Sign-in required",
            "Sign in to read private checkout status.",
        )
    })?;
    let value = HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| {
        crate::dashboard::page_error(
            StatusCode::UNAUTHORIZED,
            "Sign-in required",
            "The session credential is invalid.",
        )
    })?;
    let mut forwarded = HeaderMap::new();
    forwarded.insert("authorization", value);
    forwarded.insert(
        "x-workspace-id",
        workspace.parse().map_err(|_| {
            crate::dashboard::page_error(
                StatusCode::BAD_REQUEST,
                "Invalid workspace",
                "Select a current workspace.",
            )
        })?,
    );
    Ok(forwarded)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approval_binds_current_session_action_workspace_and_exact_terms() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            "oa_session=sess_isolated_browser_fixture".parse().unwrap(),
        );
        let mut f = FundingForm {
            action: "checkout".into(),
            door: "door".into(),
            id: "original".into(),
            gross_cents: 0,
            approved: "a".repeat(64),
            expires: 1200,
            csrf: String::new(),
        };
        f.csrf = sign(&headers, "workspace", &f).unwrap();
        assert!(check(&headers, "workspace", &f, 1000).is_ok());
        assert!(check(&headers, "other-workspace", &f, 1000).is_err());
        assert!(check(&headers, "workspace", &f, 1201).is_err());
        f.approved = "b".repeat(64);
        assert!(check(&headers, "workspace", &f, 1000).is_err());
        f.approved = "a".repeat(64);
        f.action = "reconcile".into();
        assert!(check(&headers, "workspace", &f, 1000).is_err());
        f.action = "checkout".into();
        headers.insert("cookie", "oa_session=sess_other_fixture".parse().unwrap());
        assert!(check(&headers, "workspace", &f, 1000).is_err());
    }
}
