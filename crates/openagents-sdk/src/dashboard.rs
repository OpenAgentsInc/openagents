//! Public Home summaries and portable layouts. Reads never start execution.
use crate::{CancellationToken, Client, Deployment, Error, Session};
pub use openagents_dashboard::*;
use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveLibrary {
    pub request_id: String,
    pub expected_revision: u64,
    pub library: Library,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    pub request_id: String,
    pub expected_revision: u64,
    pub package: Package,
    pub visibility: Visibility,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remove {
    pub request_id: String,
    pub expected_revision: u64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub request_id: String,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request<T> {
    account: String,
    deployment: Deployment,
    pub body: T,
}

pub struct Home<'a> {
    client: &'a Client,
    session: &'a Session,
}
impl Client {
    pub fn home<'a>(&'a self, session: &'a Session) -> Result<Home<'a>, Error> {
        self.check_session(session)?;
        Ok(Home {
            client: self,
            session,
        })
    }
}
impl Home<'_> {
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
    pub async fn projects(&self, cancel: &CancellationToken) -> Result<Vec<Project>, Error> {
        self.send(Method::GET, &["home", "projects"], &[], None, cancel)
            .await
    }
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
    pub async fn work(
        &self,
        after: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<WorkPage, Error> {
        let query = after.map(|v| vec![("after", v.into())]).unwrap_or_default();
        self.send(Method::GET, &["home", "work"], &query, None, cancel)
            .await
    }
    pub async fn library(&self, cancel: &CancellationToken) -> Result<Library, Error> {
        self.send(Method::GET, &["home", "dashboards"], &[], None, cancel)
            .await
    }
    pub fn save_request(&self, library: Library) -> Result<Request<SaveLibrary>, Error> {
        library.validate().map_err(|_| Error::InvalidReference)?;
        Ok(self.bind(SaveLibrary {
            request_id: crate::agents::request_id()?,
            expected_revision: library.revision,
            library,
        }))
    }
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
    pub fn remove_request(&self, expected_revision: u64) -> Result<Request<Remove>, Error> {
        Ok(self.bind(Remove {
            request_id: crate::agents::request_id()?,
            expected_revision,
        }))
    }
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
