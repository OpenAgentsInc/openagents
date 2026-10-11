//! The actor runtime in the website (#11253, step 0): `crates/actors` bound
//! to our accounts and workspaces, stored in the account database.
//!
//! - **Storage.** The account database (Cloud SQL, PostgreSQL 18), reached
//!   through the Cloud SQL connector's socket like the gateway, in its own
//!   `actor` schema, applied by the crate's own migration at start
//!   (`OPENAGENTS_WEB_ACTORS_DATABASE_URL`, the same secret as the gateway's).
//! - **Authority.** [`Auth`]: an app's own token (`Bearer sess_…`) or, for
//!   reads only, the browser's session cookie, checked with the account
//!   service; then the account's membership of the workspace in the URL,
//!   read from `workspace.memberships` (the account store's own table). An
//!   owner or admin is an [`Role::Owner`], a member a [`Role::Member`]. The
//!   principal is `account:<id>`. No HTTP caller is ever a service or an
//!   administrator: those are the host's own pages ([`Host::host_caller`])
//!   and `actors-admin`.
//! - **Executors.** A linked Mac names itself with `X-OpenAgents-Computer`
//!   on its own token; its grant is exactly the Mac-jobs queue and that
//!   Mac's target, one claim at a time, for an hour.
//! - **Revalidation.** Queued messages and alarms run under the saved
//!   caller only while that account is still an active member of the
//!   workspace.
//! - **Runtime.** The inbox, alarm, and expiry workers run in this process;
//!   any number of processes may run them against one database.
//! - **Routes.** `/v1/w/{workspace}/…` and `/v1/actors/contract.json`
//!   ([`actors::http::router`]). A request with a cookie may only read:
//!   cookies are removed from other methods, so a page can't be made to
//!   act for its visitor (CSRF).
//!
//! Operators reach the same tables with `actors-admin` through the Cloud
//! SQL proxy (docs/deployment/actors.md).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use actors::http::Authenticator;
use actors::{ActorError, Caller, ExecutorGrant, PgStore, Pool, Registry, Role, RuntimeHandle};
use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderMap, Method, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use futures_util::future::BoxFuture;
use sha2::{Digest, Sha256};

use crate::cloud::session::CloudSession;

/// The connection string of the account database for the actor store.
pub const DATABASE_ENV: &str = "OPENAGENTS_WEB_ACTORS_DATABASE_URL";
/// `1`: Mac jobs run through the `mac.job` actor (#11253 step 1).
pub const MAC_JOBS_ENV: &str = "OPENAGENTS_WEB_MAC_JOBS_ACTORS";
/// The header a linked Mac names itself with (percent-encoded).
pub const COMPUTER_HEADER: &str = "x-openagents-computer";
/// Connections the actor store keeps (plus one for its listener). The
/// staging instance is small; this stays well inside it.
const POOL: usize = 4;
/// How long an executor grant lasts; the Mac's next request gets a new one.
const GRANT_MS: i64 = 3_600_000;
/// How long a checked token or membership is reused.
const REMEMBER: Duration = Duration::from_secs(20);

/// The actor runtime as this website runs it.
pub struct Host {
    pub store: PgStore,
    pool: Pool,
    auth: Arc<Auth>,
    /// Mac jobs go through actors.
    pub mac_jobs: bool,
    runtime: Mutex<Option<RuntimeHandle>>,
}

impl Host {
    /// Connect, migrate, and start the workers. `None` (and a log line)
    /// when the database can't be used: the site runs without actors.
    pub async fn start(
        dsn: &str,
        cloud: Option<Arc<CloudSession>>,
        mac_jobs: bool,
    ) -> Option<Arc<Self>> {
        match Self::open(dsn, cloud, mac_jobs).await {
            Ok(host) => Some(host),
            Err(error) => {
                eprintln!("actors: not started: {}", error.message);
                None
            }
        }
    }

    async fn open(
        dsn: &str,
        cloud: Option<Arc<CloudSession>>,
        mac_jobs: bool,
    ) -> Result<Arc<Self>, ActorError> {
        let pool = Pool::new(dsn, POOL)?;
        let mut registry = Registry::new();
        ::mac_jobs::actor::register(&mut registry)?;
        let auth = Arc::new(Auth {
            cloud,
            pool: pool.clone(),
            tokens: Mutex::default(),
            members: Mutex::default(),
        });
        let store = actors::http::with_authenticator(
            PgStore::new(pool.clone(), Arc::new(registry)),
            auth.clone(),
        );
        store.migrate().await?;
        let runtime = actors::Runtime::new(store.clone()).start()?;
        eprintln!("actors: started (mac jobs through actors: {mac_jobs})");
        Ok(Arc::new(Self {
            store,
            pool,
            auth,
            mac_jobs,
            runtime: Mutex::new(Some(runtime)),
        }))
    }

    /// A host on `dsn` with no account service (tests: the workers and the
    /// membership checks, without HTTP sign-in).
    #[cfg(test)]
    pub(crate) async fn open_for_test(dsn: &str) -> Arc<Self> {
        Self::open(dsn, None, true).await.expect("actors host")
    }

    /// Check `caller` again as a queued message would be.
    #[cfg(test)]
    pub(crate) async fn revalidate(&self, caller: &Caller) -> actors::Result<Caller> {
        self.auth.revalidate(caller).await
    }

    /// The HTTP and SSE routes, behind our authentication.
    pub fn router<S: Clone + Send + Sync + 'static>(&self) -> Router<S> {
        actors::http::router::<S>(self.store.clone(), self.auth.clone())
            .layer(middleware::from_fn(reads_only_with_cookies))
    }

    /// Stop the workers (on shutdown). Unfinished work stays durable.
    pub async fn shutdown(&self) {
        let runtime = self.runtime.lock().ok().and_then(|mut r| r.take());
        if let Some(runtime) = runtime {
            runtime.shutdown().await;
        }
    }

    /// The account's own (personal) workspace, where its records live.
    pub async fn home(&self, account: &str) -> Result<String, ActorError> {
        self.auth.home(account).await
    }

    /// The site itself acting for `account` in `workspace`: the authority
    /// the host's own pages (the job pages, the phone's board) use for
    /// what no HTTP caller may do, such as answering an approval.
    pub fn host_caller(account: &str, workspace: &str) -> Caller {
        Caller {
            principal: principal(account),
            workspace_id: workspace.to_owned(),
            account_id: Some(account.to_owned()),
            role: Role::Service,
            executor: None,
        }
    }

    /// The account itself (its owner role), for reads and cancels.
    pub fn owner_caller(account: &str, workspace: &str) -> Caller {
        Caller {
            role: Role::Owner,
            ..Self::host_caller(account, workspace)
        }
    }

    /// The linked Mac `computer` of `account`: its executor grant.
    pub fn mac_caller(account: &str, workspace: &str, computer: &str) -> Caller {
        Caller {
            executor: Some(mac_grant(computer)),
            ..Self::owner_caller(account, workspace)
        }
    }

    /// The pool, for the host's own reads.
    pub fn pool(&self) -> &Pool {
        &self.pool
    }
}

fn principal(account: &str) -> String {
    format!("account:{account}")
}

fn mac_grant(computer: &str) -> ExecutorGrant {
    let target = ::mac_jobs::actor::target(computer);
    ExecutorGrant {
        id: target.clone(),
        queues: vec![::mac_jobs::actor::QUEUE.into()],
        targets: vec![target],
        generation: 1,
        expires_at: now_ms().saturating_add(GRANT_MS),
        max_claims: 1,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Cookies only for reads: a request that changes something must carry the
/// app's own token, which no other site can make a browser send.
async fn reads_only_with_cookies(mut request: Request, next: Next) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        request.headers_mut().remove(header::COOKIE);
    }
    next.run(request).await
}

/// Who a request is, by our accounts and workspaces.
pub struct Auth {
    cloud: Option<Arc<CloudSession>>,
    pool: Pool,
    tokens: Mutex<HashMap<[u8; 32], (String, Instant)>>,
    members: Mutex<HashMap<(String, String), (Option<Role>, Instant)>>,
}

fn unauthorized() -> ActorError {
    ActorError::new("unauthorized", "Sign in first.")
}

fn unavailable() -> ActorError {
    ActorError::retry("unavailable", "Accounts can't be checked right now.")
}

/// A percent-encoded header value, decoded.
fn decoded(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

impl Auth {
    async fn account(&self, headers: &HeaderMap) -> Result<(String, bool), ActorError> {
        let cloud = self.cloud.as_deref().ok_or_else(unauthorized)?;
        if let Some(token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        {
            let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
            if let Some((account, at)) = self.tokens.lock().ok().and_then(|t| t.get(&key).cloned())
                && at.elapsed() < REMEMBER
            {
                return Ok((account, true));
            }
            return match cloud.app_account(token).await {
                Ok(account) => {
                    if let Ok(mut tokens) = self.tokens.lock() {
                        tokens.retain(|_, (_, at)| at.elapsed() < REMEMBER);
                        tokens.insert(key, (account.clone(), Instant::now()));
                    }
                    Ok((account, true))
                }
                Err(crate::cloud::session::SessionError::Unauthenticated) => Err(unauthorized()),
                Err(_) => Err(unavailable()),
            };
        }
        if headers.contains_key(header::COOKIE) {
            return match cloud.authenticate(headers).await {
                Ok(viewer) => Ok((viewer.account_id, false)),
                Err(crate::cloud::session::SessionError::Unauthenticated) => Err(unauthorized()),
                Err(_) => Err(unavailable()),
            };
        }
        Err(unauthorized())
    }

    /// The account's current role in `workspace`, from the account store's
    /// own membership table; `None` when it isn't an active member.
    async fn role(&self, account: &str, workspace: &str) -> Result<Option<Role>, ActorError> {
        let key = (account.to_owned(), workspace.to_owned());
        if let Some((role, at)) = self.members.lock().ok().and_then(|m| m.get(&key).cloned())
            && at.elapsed() < REMEMBER
        {
            return Ok(role);
        }
        let connection = self.pool.acquire().await?;
        let row = connection
            .query_opt(
                "SELECT role, status FROM workspace.memberships WHERE workspace_id=$1 AND account_id=$2",
                &[&workspace, &account],
            )
            .await
            .map_err(|_| unavailable())?;
        drop(connection);
        let role = row.and_then(|row| {
            let role: Option<String> = row.get(0);
            let status: Option<String> = row.get(1);
            (status.as_deref() == Some("active")).then(|| match role.as_deref() {
                Some("owner" | "admin") => Role::Owner,
                _ => Role::Member,
            })
        });
        if let Ok(mut members) = self.members.lock() {
            members.retain(|_, (_, at)| at.elapsed() < REMEMBER);
            members.insert(key, (role.clone(), Instant::now()));
        }
        Ok(role)
    }

    async fn home(&self, account: &str) -> Result<String, ActorError> {
        let connection = self.pool.acquire().await?;
        let row = connection
            .query_opt(
                "SELECT m.workspace_id FROM workspace.memberships m \
                 JOIN workspace.workspaces w ON w.id = m.workspace_id \
                 WHERE m.account_id=$1 AND m.status='active' AND m.role='owner' AND w.kind='personal' \
                 ORDER BY m.workspace_id LIMIT 1",
                &[&account],
            )
            .await
            .map_err(|_| unavailable())?;
        row.map(|row| row.get::<_, String>(0))
            .ok_or_else(|| ActorError::new("not_found", "This account has no workspace."))
    }
}

impl Authenticator for Auth {
    fn authenticate<'a>(
        &'a self,
        headers: &'a HeaderMap,
        workspace: &'a str,
    ) -> BoxFuture<'a, actors::Result<Caller>> {
        Box::pin(async move {
            let (account, token) = self.account(headers).await?;
            let role = self
                .role(&account, workspace)
                .await?
                .ok_or_else(|| ActorError::new("not_found", "The requested item was not found."))?;
            // Only an app's own token names an executor; a page never does.
            let executor = headers
                .get(COMPUTER_HEADER)
                .filter(|_| token)
                .and_then(|v| v.to_str().ok())
                .and_then(decoded)
                .map(|name| crate::coder_sync::line(&name, 64))
                .filter(|name| !name.is_empty())
                .map(|name| mac_grant(&name));
            Ok(Caller {
                principal: principal(&account),
                workspace_id: workspace.to_owned(),
                account_id: Some(account),
                role,
                executor,
            })
        })
    }

    fn revalidate<'a>(&'a self, caller: &'a Caller) -> BoxFuture<'a, actors::Result<Caller>> {
        Box::pin(async move {
            let account = caller
                .account_id
                .as_deref()
                .filter(|account| caller.principal == principal(account))
                .ok_or_else(|| ActorError::new("forbidden", "Access was removed."))?;
            let role = self
                .role(account, &caller.workspace_id)
                .await?
                .ok_or_else(|| ActorError::new("forbidden", "Access was removed."))?;
            Ok(Caller {
                // The site's own scheduled work keeps the site's authority
                // while the account still belongs; anyone else gets the
                // role they hold now.
                role: if caller.role == Role::Service {
                    Role::Service
                } else {
                    role
                },
                executor: None,
                ..caller.clone()
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computer_names_decode_and_grants_are_exact() {
        assert_eq!(decoded("Chris%27s%20Mac").as_deref(), Some("Chris's Mac"));
        assert_eq!(decoded("bad%2"), None);
        let grant = mac_grant("Chris's Mac");
        assert_eq!(grant.queues, [::mac_jobs::actor::QUEUE]);
        assert_eq!(grant.targets, [::mac_jobs::actor::target("Chris's Mac")]);
        assert_eq!(grant.max_claims, 1);
        let host = Host::host_caller("acct_1", "ws_1");
        assert_eq!(host.role, Role::Service);
        assert_eq!(host.principal, "account:acct_1");
    }
}
