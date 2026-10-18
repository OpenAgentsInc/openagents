mod common;

use archil::{Archil, ClientOptions, Error, derive_s3_base_url, resolve_base_url};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer};

#[test]
fn missing_api_key_and_region_are_config_errors() {
    // Environment fallbacks are exercised by passing explicit empty options
    // only when the variables are unset in the test environment.
    if std::env::var("ARCHIL_API_KEY").is_err() {
        let e = Archil::new(ClientOptions::new().region("aws-us-east-1")).unwrap_err();
        assert!(
            matches!(e, Error::Config(ref m) if m.contains("Missing API key")),
            "{e}"
        );
    }
    if std::env::var("ARCHIL_REGION").is_err() {
        let e = Archil::new(ClientOptions::new().api_key("k")).unwrap_err();
        assert!(
            matches!(e, Error::Config(ref m) if m.contains("Missing region")),
            "{e}"
        );
    }
}

#[test]
fn unknown_region_lists_valid_regions() {
    let e = Archil::new(ClientOptions::new().api_key("k").region("mars-1")).unwrap_err();
    let msg = e.to_string();
    assert!(msg.contains("Unknown region \"mars-1\""), "{msg}");
    assert!(
        msg.contains("aws-us-east-1") && msg.contains("gcp-us-central1"),
        "{msg}"
    );
}

#[test]
fn region_resolves_control_plane_and_s3_urls() {
    let c = Archil::new(ClientOptions::new().api_key("k").region("aws-eu-west-1")).unwrap();
    assert_eq!(
        c.base_url(),
        "https://control.green.eu-west-1.aws.prod.archil.com"
    );
    assert_eq!(
        c.s3_base_url(),
        Some("https://s3.green.eu-west-1.aws.prod.archil.com")
    );
    assert_eq!(
        resolve_base_url("gcp-us-central1").unwrap(),
        "https://control.blue.us-central1.gcp.prod.archil.com"
    );
}

#[test]
fn s3_url_derivation_swaps_only_a_leading_control_label() {
    assert_eq!(
        derive_s3_base_url("https://control.x.y.com/").as_deref(),
        Some("https://s3.x.y.com")
    );
    assert_eq!(
        derive_s3_base_url("http://localhost:8080").as_deref(),
        Some("http://localhost:8080")
    );
    assert_eq!(derive_s3_base_url("not a url"), None);
}

#[test]
fn explicit_base_url_still_requires_region() {
    if std::env::var("ARCHIL_REGION").is_err() {
        let e = Archil::new(ClientOptions::new().api_key("k").base_url("http://x")).unwrap_err();
        assert!(matches!(e, Error::Config(_)));
    }
}

#[tokio::test]
async fn sends_key_prefixed_authorization_and_user_agent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .and(header("authorization", "key-abc"))
        .and(header("user-agent", archil::USER_AGENT))
        .respond_with(common::ok(common::disk_json("d1", "n")))
        .expect(2)
        .mount(&server)
        .await;
    // A key that already has the prefix is not double-prefixed.
    for key in ["abc", "key-abc"] {
        let c = Archil::new(
            ClientOptions::new()
                .api_key(key)
                .region("aws-us-east-1")
                .base_url(server.uri()),
        )
        .unwrap();
        assert_eq!(c.disks().get("d1").await.unwrap().id, "d1");
    }
    assert!(archil::USER_AGENT.starts_with("archil-rs/"));
}

#[test]
fn empty_ca_bundle_is_rejected() {
    // A bundle with no certificates would leave the client with no trust
    // roots at all, so it must fail at construction rather than at first use.
    let e = Archil::new(
        ClientOptions::new()
            .api_key("k")
            .region("aws-us-east-1")
            .ca_pem(b"not a pem".to_vec()),
    )
    .unwrap_err();
    assert!(matches!(e, Error::Config(ref m) if m.contains("CA")), "{e}");
}
