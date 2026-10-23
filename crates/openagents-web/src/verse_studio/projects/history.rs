//! Authorized saved history and read-only correlation to existing published credits.
use super::*;
use aethyrforge_host::{
    Contribution, ContributionEarnings, Contributor, CreditState, RecordedCredit, contribution_id,
    contribution_summary, project_changes, source_state,
};
use nostr::{domain::Event, earnings};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::PathBuf,
    time::Duration,
};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/history",
            get(history),
        )
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/history/{version}/earnings",
            get(recorded_earnings),
        )
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/history/{version}",
            get(version),
        )
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct Link {
    pub contribution: String,
    pub actor: String,
    pub party: String,
    pub source: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Links {
    pub schema: String,
    pub rule: String,
    pub test_ledger: bool,
    pub links: Vec<Link>,
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Links {
    fn validate(&mut self) -> Result<(), ()> {
        if self.schema != "openagents.verse.reward-links.v1"
            || self.rule != "operator-recorded-credit-link.v1"
            || self.links.len() > 1024
        {
            return Err(());
        }
        let mut unique = Vec::new();
        let mut sources = BTreeMap::new();
        let mut counts = BTreeMap::<String, usize>::new();
        for link in &self.links {
            if !hex(&link.contribution)
                || !valid_id(&link.actor)
                || !hex(&link.party)
                || !hex(&link.source)
            {
                return Err(());
            }
            if unique.contains(link) {
                continue;
            }
            if sources
                .insert((&link.party, &link.source), &link.contribution)
                .is_some()
            {
                return Err(());
            }
            let count = counts.entry(link.contribution.clone()).or_default();
            *count += 1;
            if *count > 8 {
                return Err(());
            }
            unique.push(link.clone());
        }
        self.links = unique;
        Ok(())
    }
}
async fn links(path: Option<&PathBuf>) -> Result<Option<Links>, ()> {
    let Some(path) = path else { return Ok(None) };
    let path = path.clone();
    tokio::task::spawn_blocking(move || {
        if !path.is_absolute() {
            return Err(());
        }
        let file = std::fs::File::open(&path).map_err(|_| ())?;
        if !file.metadata().map_err(|_| ())?.is_file() {
            return Err(());
        }
        let mut bytes = Vec::new();
        file.take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ())?;
        if bytes.len() > 256 * 1024 {
            return Err(());
        }
        let mut book: Links = serde_json::from_slice(&bytes).map_err(|_| ())?;
        book.validate()?;
        Ok(Some(book))
    })
    .await
    .map_err(|_| ())?
}
#[derive(Deserialize)]
struct Published {
    events: Vec<Event>,
    issuers: BTreeSet<String>,
}
async fn credit(app: &App, link: &Link, test: bool) -> Result<Option<RecordedCredit>, ()> {
    static READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(8);
    let _permit = tokio::time::timeout(Duration::from_millis(250), READS.acquire())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    let service = app.config.cloud.as_deref().ok_or(())?;
    let path = format!("/v1/earnings/public/{}/sources/{}", link.party, link.source);
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        service.earnings_call_from(
            reqwest::Method::GET,
            &path,
            None,
            &[],
            app.config.verse_reward_service.as_deref(),
        ),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())?;
    if response.status == 404 {
        return Ok(None);
    }
    if response.status != 200 {
        return Err(());
    }
    let published: Published = serde_json::from_value(response.body).map_err(|_| ())?;
    if published.events.len() > 8 || published.issuers.len() > 8 {
        return Err(());
    }
    let selected = earnings::selected(&published.events, &link.party, &published.issuers, now());
    let Some((event, receipt)) = selected.into_iter().find(|(_, r)| r.source == link.source) else {
        return Err(());
    };
    Ok(Some(RecordedCredit {
        receipt: event.id,
        source: receipt.source,
        amount: receipt.amount_msat.to_string(),
        unit: "msat".into(),
        state: match receipt.state {
            earnings::State::Pending => CreditState::Pending,
            earnings::State::Earned => CreditState::Confirmed,
            earnings::State::Paid => CreditState::Paid,
            earnings::State::Reversed => CreditState::Reversed,
        },
        recorded_unix: receipt.recorded_at,
        test_ledger: test,
    }))
}
async fn names(app: &App, headers: &HeaderMap, a: &Access) -> BTreeMap<String, Contributor> {
    let Some(service) = app.config.cloud.as_deref() else {
        return BTreeMap::new();
    };
    let path = format!("/v1/projects/{}/members", a.project.id);
    let result = if a.actor.cookie.is_some() {
        service
            .project_call(headers, reqwest::Method::GET, &path, None)
            .await
    } else {
        let Some(token) = bearer(headers) else {
            return BTreeMap::new();
        };
        service
            .project_call_as(token, reqwest::Method::GET, &path, None)
            .await
    };
    let Ok(value) = result else {
        return BTreeMap::new();
    };
    value["members"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let account = m["account"].as_str()?;
            let name = m["name"]
                .as_str()
                .filter(|n| !n.contains('@') && n.len() <= 120 && !n.chars().any(char::is_control))
                .unwrap_or("Contributor");
            let profile = m["login"]
                .as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 39
                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                })
                .map(|login| format!("https://github.com/{login}"));
            Some((
                account.to_string(),
                Contributor {
                    name: if account == a.actor.account_id {
                        "You".into()
                    } else {
                        name.into()
                    },
                    profile,
                },
            ))
        })
        .collect()
}
async fn history(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let k = match key(&a, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let book = match load(&app.config.chat_store, &k, ns).await {
        Ok((b, _)) => b,
        Err(e) => return stored(&e),
    };
    let names = names(&app, &headers, &a).await;
    let correlations = links(app.config.verse_reward_links.as_ref()).await;
    let mut rows = Vec::new();
    for (index, r) in book.revisions.iter().enumerate() {
        let contribution = contribution_id(&id, ns, &branch, r);
        let earnings = match &correlations {
            Err(()) => ContributionEarnings::Unavailable,
            Ok(Some(links))
                if links
                    .links
                    .iter()
                    .any(|l| l.contribution == contribution && l.actor == r.actor) =>
            {
                ContributionEarnings::Lookup
            }
            _ => ContributionEarnings::NotRecorded,
        };
        rows.push(Contribution {
            id: contribution,
            version: r.version,
            actor: names.get(&r.actor).cloned().unwrap_or(Contributor {
                name: if r.actor == a.actor.account_id {
                    "You"
                } else {
                    "Contributor"
                }
                .into(),
                profile: None,
            }),
            created_unix: r.created_unix,
            summary: contribution_summary(
                &r.document,
                &project_changes(
                    index.checked_sub(1).map(|i| &book.revisions[i].document),
                    &r.document,
                ),
            ),
            saved_summary: r.summary.clone(),
            source_state: source_state(&r.document).into(),
            application_state: "not_recorded".into(),
            document_sha256: r.document_sha256.clone(),
            undo_of: r.undo_of,
            earnings,
        });
    }
    answer(
        StatusCode::OK,
        json!({"project":id,"namespace":ns,"branch":branch,"data":rows,"application_scope":"Saved content is separate from a running editor. Editor results are shown in This visit."}),
    )
}
type AtVersion = Path<(String, String, String, u64)>;
async fn recorded_earnings(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch, version)): AtVersion,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let k = match key(&a, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let book = match load(&app.config.chat_store, &k, ns).await {
        Ok((b, _)) => b,
        Err(e) => return stored(&e),
    };
    let Some(r) = version
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| book.revisions.get(i))
    else {
        return no_project();
    };
    let id = contribution_id(&id, ns, &branch, r);
    let registry = links(app.config.verse_reward_links.as_ref()).await;
    let result = match registry {
        Err(()) => ContributionEarnings::Unavailable,
        Ok(None) => ContributionEarnings::NotRecorded,
        Ok(Some(book)) => {
            let lookups = book
                .links
                .iter()
                .filter(|l| l.contribution == id && l.actor == r.actor)
                .map(|link| credit(&app, link, book.test_ledger));
            let results = futures_util::future::join_all(lookups).await;
            if results.iter().any(Result::is_err) {
                ContributionEarnings::Unavailable
            } else {
                let credits = results
                    .into_iter()
                    .filter_map(Result::ok)
                    .flatten()
                    .collect::<Vec<_>>();
                if credits.is_empty() {
                    ContributionEarnings::NotRecorded
                } else {
                    ContributionEarnings::Recorded { credits }
                }
            }
        }
    };
    answer(StatusCode::OK, json!({"contribution":id,"earnings":result}))
}
async fn version(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch, version)): AtVersion,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let k = match key(&a, ns, &branch) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let book = match load(&app.config.chat_store, &k, ns).await {
        Ok((b, _)) => b,
        Err(e) => return stored(&e),
    };
    let Some(index) = version
        .checked_sub(1)
        .and_then(|i| usize::try_from(i).ok())
        .filter(|i| *i < book.revisions.len())
    else {
        return no_project();
    };
    let r = &book.revisions[index];
    let changes = project_changes(
        index.checked_sub(1).map(|i| &book.revisions[i].document),
        &r.document,
    );
    answer(
        StatusCode::OK,
        json!({"version":r.version,"document_sha256":r.document_sha256,"document":r.document,"changes":changes,"change_limit":48}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reward_links_deduplicate_exact_entries_and_reject_double_counted_sources_and_oversized_contributions()
     {
        let link = Link {
            contribution: "11".repeat(32),
            actor: "550e8400-e29b-41d4-a716-446655440000".into(),
            party: "22".repeat(32),
            source: "33".repeat(32),
        };
        let mut book = Links {
            schema: "openagents.verse.reward-links.v1".into(),
            rule: "operator-recorded-credit-link.v1".into(),
            test_ledger: true,
            links: vec![link.clone(), link.clone()],
        };
        book.validate().unwrap();
        assert_eq!(book.links.len(), 1);
        let mut stolen = link.clone();
        stolen.contribution = "44".repeat(32);
        book.links.push(stolen);
        assert!(book.validate().is_err());
        book.links = (1..=9)
            .map(|n| Link {
                source: format!("{n:064x}"),
                ..link.clone()
            })
            .collect();
        assert!(book.validate().is_err());
        book.links = vec![Link {
            actor: "untrusted actor".into(),
            ..link
        }];
        assert!(book.validate().is_err());
    }
}
