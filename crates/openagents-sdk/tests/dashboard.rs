use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    routing::get,
};
use openagents_sdk::{CancellationToken, Client, Deployment, Error, dashboard::*};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[tokio::test]
async fn summary_reads_and_saved_requests_keep_account_fences_and_cancellation() {
    let writes = Arc::new(AtomicUsize::new(0));
    let calls = writes.clone();
    let router=Router::new()
        .route("/v1/account",get(|headers:HeaderMap|async move{let second=headers["authorization"].to_str().unwrap().ends_with(&"b".repeat(64));Json(json!({"account":{"id":if second{"acct_b"}else{"acct_a"},"label":"Fixture"},"workspaces":[]}))}))
        .route("/v1/home/dashboards",get(||async{Json(json!(Library::default()))}).put(move|Json(body):Json<SaveLibrary>|{let calls=calls.clone();async move{calls.fetch_add(1,Ordering::SeqCst);let mut library=body.library;library.revision+=1;Json(library)}}))
        .route("/v1/home/work",get(||async{Json(json!(WorkPage{complete:true,data:vec![Work{id:"work".into(),state:"future-state".into(),..Work::default()}],..WorkPage::default()}))}))
        .route("/v1/home/issues",axum::routing::post(||async{(StatusCode::TOO_MANY_REQUESTS,[("retry-after","120")],Json(json!({"error":{"code":"rate_limited"}})))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let a = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let b = client
        .authenticate(&format!("oa_sk_{}", "b".repeat(64)))
        .await
        .unwrap();
    let home = client.home(&a).unwrap();
    let cancel = CancellationToken::new();
    let request = home.save_request(Library::default()).unwrap();
    let encoded = serde_json::to_vec(&request).unwrap();
    let restored: Request<SaveLibrary> = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored.body.request_id, request.body.request_id);
    assert_eq!(home.save(&restored, &cancel).await.unwrap().revision, 1);
    assert_eq!(
        client
            .home(&b)
            .unwrap()
            .save(&restored, &cancel)
            .await
            .unwrap_err(),
        Error::WrongAccount
    );
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        home.work(None, &cancel).await.unwrap().data[0].state_label(),
        "Status unavailable"
    );
    let filter = IssueFilter {
        scope: Scope::default(),
        mine: false,
        assignee: None,
        labels: vec![],
        page: 1,
    };
    assert_eq!(
        home.issues(&filter, &cancel).await.unwrap_err(),
        Error::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(120))
        }
    );
    cancel.cancel();
    assert_eq!(home.library(&cancel).await.unwrap_err(), Error::Cancelled);
    let _: Value = serde_json::from_slice(&encoded).unwrap();
    task.abort();
}
