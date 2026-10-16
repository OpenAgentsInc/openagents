//! Gallery moderation uses the account service's current site-admin flag.
use super::*;

async fn gallery_request(
    f: &Fixture,
    cookies: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let request = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::ORIGIN, ORIGIN)
        .header(header::COOKIE, cookies)
        .header("x-sand-garden", "1")
        .header(header::CONTENT_TYPE, "application/json")
        .body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
        .unwrap();
    let response = f.site.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn a_site_admin_can_delete_another_visitors_creation_and_loses_that_right_when_removed() {
    let f = fixture().await;
    let path = "/studios/blue-rush/garden/api";
    let creator = "oa_visitor=11111111111111111111111111111111";
    let mut ids = vec![];
    for i in 0..2 {
        let (status,body)=gallery_request(&f,creator,path,Some(json!({"title":"A little beach","request_id":format!("{i:032x}"),"drawing":{"actions":[{"tool":"stone","size":1.,"angle":0.,"points":[{"x":120.,"y":180.}]}]}}))).await;
        assert_eq!(status, StatusCode::CREATED);
        ids.push(body["id"].as_str().unwrap().to_owned());
    }
    f.state.lock().unwrap().admins.insert("alice".into());
    let alice = login(&f, "alice").await;
    let cookies = format!(
        "{}; oa_visitor=22222222222222222222222222222222",
        alice.header()
    );
    let (status, list) = gallery_request(&f, &cookies, path, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["admin"], true);
    assert_eq!(list["items"][0]["can_delete"], true);
    assert_eq!(
        gallery_request(
            &f,
            &cookies,
            &format!("{path}/{}/delete", ids[0]),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::OK
    );
    f.state.lock().unwrap().admins.remove("alice");
    assert_eq!(
        gallery_request(
            &f,
            &cookies,
            &format!("{path}/{}/delete", ids[1]),
            Some(json!({}))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}
