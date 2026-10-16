# Home dashboards

The Rust SDK's `Client::home(&session)` reads project scopes, open issue
summaries, work across computers, saved dashboards, and shared layouts through
`/v1/home`. These calls observe work; they don't start agents.

```rust,no_run
# async fn example(client: openagents_sdk::Client, session: openagents_sdk::Session) -> Result<(), openagents_sdk::Error> {
let cancel = openagents_sdk::CancellationToken::new();
let home = client.home(&session)?;
let projects = home.projects(&cancel).await?;
let work = home.work(None, &cancel).await?;
let library = home.library(&cancel).await?;
let request = home.save_request(library)?;
let saved = home.save(&request, &cancel).await?;
# Ok(())
# }
```

Persist a mutation request before sending it. Retry the same serialized request
after an uncertain response; its ID, account, and deployment remain fixed.
A version conflict requires loading the latest value before making a new edit.
The client returns typed cancellation, access, version, and rate-limit errors.
Respect `retry_after` before making another request.

Issue filters name a project, repositories, an optional GitHub Projects board,
labels, and an assignee. `mine` uses the connected GitHub identity. Pages exclude
pull requests; `complete = false` marks an incomplete count. `next_page` loads
more matching issues. Work pages include explicit continuation and freshness;
an unavailable computer doesn't turn its last report into an idle agent.

`openagents-dashboard` defines versioned, bounded layout packages. Export strips
resource bindings and filters. Import validates before creating a draft and
requires the recipient to choose their own resources. Unknown widget versions
stay visible as unavailable widgets. No package runs code or carries credentials.

Publication is explicit: private, unlisted, or public. Use the directory's
preview before installing a private copy. Template updates create a reviewable
draft and preserve local settings and reordered or removed widgets. Authors can
remove listings; reports hide a listing after three distinct reports pending
administrator review.

See the [product specification](../dashboard-widgets.md) and
[app OpenAPI](openapi.app.json) for the full contract.
