//! Disk listing, lookup and creation.

use std::sync::Arc;

use reqwest::Method;

use crate::disk::{Disk, exec_disk};
use crate::error::{Error, Result};
use crate::http::{Http, Req, enc};
use crate::retry::RetryMode;
use crate::types::{AuthorizedUser, CreateDiskRequest, DiskResponse, ExecResult};

/// Options for [`Disks::list`] and [`Disks::list_page`].
#[derive(Debug, Clone, Default)]
pub struct ListDisksOptions {
    /// Cap on the total number of disks returned.
    pub limit: Option<usize>,
    /// Resume listing from a previous page's `next_cursor`.
    pub cursor: Option<String>,
    /// Filter by name. A name filter returns a single page.
    pub name: Option<String>,
}

/// One page of disks.
#[derive(Debug, Clone)]
pub struct DiskListPage {
    pub disks: Vec<Disk>,
    /// Set when more disks remain; pass it back as `cursor`.
    pub next_cursor: Option<String>,
}

/// Server-side maximum page size for `GET /api/disks`.
const DISK_PAGE_LIMIT: usize = 100;

/// Result of [`Disks::create`].
#[derive(Debug, Clone)]
pub struct CreateDiskResult {
    pub disk: Disk,
    /// One-time mount token. Save it; it cannot be retrieved again.
    pub token: Option<String>,
    pub token_identifier: Option<String>,
    pub authorized_users: Vec<AuthorizedUser>,
}

/// Disk operations, from [`Archil::disks`](crate::Archil::disks).
#[derive(Debug, Clone)]
pub struct Disks {
    http: Arc<Http>,
}

impl Disks {
    pub(crate) fn new(http: Arc<Http>) -> Self {
        Self { http }
    }

    /// List the account's disks, following `next_cursor` until exhausted so
    /// the result is complete for large accounts. `limit` caps the total.
    pub async fn list(&self, opts: ListDisksOptions) -> Result<Vec<Disk>> {
        if opts.name.is_some() {
            return Ok(self.list_page(opts).await?.disks);
        }
        let limit = opts.limit;
        let mut cursor = opts.cursor;
        let mut disks: Vec<Disk> = Vec::new();
        loop {
            let remaining = limit.map(|l| l.saturating_sub(disks.len()));
            if remaining == Some(0) {
                return Ok(disks);
            }
            let page_limit = remaining.map_or(DISK_PAGE_LIMIT, |r| r.min(DISK_PAGE_LIMIT));
            let page = self
                .list_page(ListDisksOptions {
                    limit: Some(page_limit),
                    cursor: cursor.clone(),
                    name: None,
                })
                .await?;
            // A server that predates pagination ignores `limit` and returns
            // everything; truncate so the cap still holds.
            let take = remaining.unwrap_or(usize::MAX);
            disks.extend(page.disks.into_iter().take(take));
            // A repeated cursor means no forward progress: never loop forever.
            match page.next_cursor {
                Some(next) if Some(&next) != cursor.as_ref() => cursor = Some(next),
                _ => return Ok(disks),
            }
        }
    }

    /// Fetch a single page of disks.
    pub async fn list_page(&self, opts: ListDisksOptions) -> Result<DiskListPage> {
        let req = Req::get("/api/disks")
            .query_opt("limit", opts.limit)
            .query_opt("cursor", opts.cursor)
            .query_opt("name", opts.name);
        // An empty account serializes as JSON null.
        let (data, next_cursor): (Option<Vec<DiskResponse>>, Option<String>) =
            self.http.call_page(req).await?;
        let disks = data
            .unwrap_or_default()
            .into_iter()
            .map(|d| Disk::new(d, self.http.clone()))
            .collect();
        Ok(DiskListPage {
            disks,
            next_cursor: next_cursor.filter(|c| !c.is_empty()),
        })
    }

    pub async fn get(&self, id: &str) -> Result<Disk> {
        let data: DiskResponse = self
            .http
            .call(Req::get(format!("/api/disks/{}", enc(id))))
            .await?;
        Ok(Disk::new(data, self.http.clone()))
    }

    /// Create a disk with an auto-generated mount token.
    ///
    /// Returns the disk, the one-time token, and its identifier for later
    /// management.
    pub async fn create(&self, req: CreateDiskRequest) -> Result<CreateDiskResult> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Created {
            disk_id: Option<String>,
            authorized_users: Option<Vec<AuthorizedUser>>,
        }
        let created: Created = self
            .http
            .call(Req::new(Method::POST, "/api/disks", RetryMode::Connect).json(req))
            .await?;
        let Some(disk_id) = created.disk_id.filter(|s| !s.is_empty()) else {
            return Err(Error::Decode("API returned success but no diskId".into()));
        };
        let authorized_users = created.authorized_users.unwrap_or_default();
        let token_user = authorized_users.iter().find(|u| u.token.is_some());
        let token = token_user.and_then(|u| u.token.clone());
        let token_identifier = token_user.and_then(|u| u.identifier.clone());
        let disk = self.get(&disk_id).await?;
        Ok(CreateDiskResult {
            disk,
            token,
            token_identifier,
            authorized_users,
        })
    }

    /// Execute a command on a disk by id without fetching the disk first.
    pub async fn exec(&self, id: &str, command: &str) -> Result<ExecResult> {
        exec_disk(&self.http, id, command).await
    }
}
