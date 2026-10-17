//! Public Home summaries and portable layouts. Reads never start execution.
use crate::{CancellationToken, Client, Deployment, Error, Session};
pub use openagents_dashboard::*;
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Filters for the Home issue list.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueFilter {
    pub scope: Scope,
    pub mine: bool,
    pub assignee: Option<String>,
    pub labels: Vec<String>,
    #[serde(default = "first_page")]
    pub page: u32,
}
fn first_page() -> u32 {
    1
}
/// The body of a request that saves the dashboard library.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveLibrary {
    pub request_id: String,
    pub expected_revision: u64,
    pub library: Library,
}
/// The body of a request that publishes a dashboard template.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    pub request_id: String,
    pub expected_revision: u64,
    pub package: Package,
    pub visibility: Visibility,
}
/// The body of a request that removes a published template.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remove {
    pub request_id: String,
    pub expected_revision: u64,
}
/// The body of a request that reports a published template.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub request_id: String,
    pub reason: String,
}
/// A request body tied to the account and deployment that made it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request<T> {
    account: String,
    deployment: Deployment,
    pub body: T,
}

/// Home reads and dashboard edits for one signed-in session.
pub struct Home<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    /// Returns the Home client for a session on this deployment.
    pub fn home<'a>(&'a self, session: &'a Session) -> Result<Home<'a>, Error> {
        self.check_session(session)?;
        Ok(Home {
            client: self,
            session,
        })
    }
}
impl Home<'_> {
    /// Ties a request body to this session's account and deployment.
    pub fn bind<T>(&self, body: T) -> Request<T> {
        Request {
            account: self.session.account().id.clone(),
            deployment: self.client.deployment().clone(),
            body,
        }
    }
    fn body<T: Serialize>(&self, request: &Request<T>) -> Result<serde_json::Value, Error> {
        if request.account != self.session.account().id {
            return Err(Error::WrongAccount);
        }
        if request.deployment != *self.client.deployment() {
            return Err(Error::WrongDeployment);
        }
        serde_json::to_value(&request.body).map_err(|_| Error::InvalidReference)
    }
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &[&str],
        query: &[(&str, String)],
        body: Option<serde_json::Value>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        self.client.check_session(self.session)?;
        tokio::select! { biased; _ = cancel.cancelled() => Err(Error::Cancelled), result = self.client.request(method, path, query, body, Some(self.session.token())) => result }
    }
    /// Lists the projects shown on Home.
    pub async fn projects(&self, cancel: &CancellationToken) -> Result<Vec<Project>, Error> {
        self.send(Method::GET, &["home", "projects"], &[], None, cancel)
            .await
    }
    /// Returns a page of issues that match a filter.
    pub async fn issues(
        &self,
        filter: &IssueFilter,
        cancel: &CancellationToken,
    ) -> Result<IssueSummary, Error> {
        filter
            .scope
            .validate()
            .map_err(|_| Error::InvalidReference)?;
        self.send(
            Method::POST,
            &["home", "issues"],
            &[],
            Some(serde_json::to_value(filter).map_err(|_| Error::InvalidReference)?),
            cancel,
        )
        .await
    }
    /// Returns a page of recent work, starting after an optional cursor.
    pub async fn work(
        &self,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<WorkPage, Error> {
        let query = after.map(|v| vec![("after", v.into())]).unwrap_or_default();
        self.send(Method::GET, &["home", "work"], &query, None, cancel)
            .await
    }
    /// Returns the saved dashboard library.
    pub async fn library(&self, cancel: &CancellationToken) -> Result<Library, Error> {
        self.send(Method::GET, &["home", "dashboards"], &[], None, cancel)
            .await
    }
    /// Checks a library and builds a request that saves it.
    pub fn save_request(&self, library: Library) -> Result<Request<SaveLibrary>, Error> {
        library.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(SaveLibrary {
            request_id: crate::agents::request_id()?,
            expected_revision: library.revision,
            library,
        }))
    }
    /// Saves the dashboard library and returns the saved copy.
    pub async fn save(
        &self,
        request: &Request<SaveLibrary>,
        cancel: &CancellationToken,
    ) -> Result<Library, Error> {
        self.send(
            Method::PUT,
            &["home", "dashboards"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Checks a package and builds a request that publishes it.
    pub fn publish_request(
        &self,
        package: Package,
        visibility: Visibility,
        expected_revision: u64,
    ) -> Result<Request<Publish>, Error> {
        package.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(Publish {
            request_id: crate::agents::request_id()?,
            expected_revision,
            package,
            visibility,
        }))
    }
    /// Publishes a template under an ID and returns its listing.
    pub async fn publish(
        &self,
        id: &str,
        request: &Request<Publish>,
        cancel: &CancellationToken,
    ) -> Result<Listing, Error> {
        if !crate::agents::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::PUT,
            &["home", "templates", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Searches the template directory, optionally only your own templates.
    pub async fn directory(
        &self,
        search: &str,
        after: Option<&str>,
        mine: bool,
        cancel: &CancellationToken,
    ) -> Result<Directory, Error> {
        if search.len() > 100 {
            return Err(Error::InvalidReference);
        }
        let mut query = vec![("q", search.into()), ("mine", mine.to_string())];
        if let Some(after) = after {
            query.push(("after", after.into()));
        }
        self.send(Method::GET, &["home", "templates"], &query, None, cancel)
            .await
    }
    /// Returns a template's listing, at a given version or the latest.
    pub async fn template(
        &self,
        id: &str,
        version: Option<u64>,
        cancel: &CancellationToken,
    ) -> Result<Listing, Error> {
        if !crate::agents::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        let query = version
            .map(|v| vec![("version", v.to_string())])
            .unwrap_or_default();
        self.send(
            Method::GET,
            &["home", "templates", id],
            &query,
            None,
            cancel,
        )
        .await
    }
    /// Lists templates published by one author.
    pub async fn directory_by_author(
        &self,
        author: &str,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<Directory, Error> {
        if !id(author) {
            return Err(Error::InvalidReference);
        }
        let mut query = vec![("author", author.into())];
        if let Some(after) = after {
            query.push(("after", after.into()));
        }
        self.send(Method::GET, &["home", "templates"], &query, None, cancel)
            .await
    }
    /// Builds a request that removes a published template.
    pub fn remove_request(&self, expected_revision: u64) -> Result<Request<Remove>, Error> {
        Ok(self.bind(Remove {
            request_id: crate::agents::request_id()?,
            expected_revision,
        }))
    }
    /// Removes a published template and returns its final listing.
    pub async fn remove(
        &self,
        id: &str,
        request: &Request<Remove>,
        cancel: &CancellationToken,
    ) -> Result<Listing, Error> {
        if !crate::agents::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::DELETE,
            &["home", "templates", id],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
    /// Reports a published template for review.
    pub async fn report(
        &self,
        id: &str,
        request: &Request<Report>,
        cancel: &CancellationToken,
    ) -> Result<serde_json::Value, Error> {
        if !crate::agents::valid_id(id) {
            return Err(Error::InvalidReference);
        }
        self.send(
            Method::POST,
            &["home", "templates", id, "report"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
}
