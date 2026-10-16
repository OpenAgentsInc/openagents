//! Account-independent Home configuration. Packages carry settings, never authority.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const MAX_PACKAGE: usize = 256 * 1024;
pub const MAX_WIDGETS: usize = 32;
pub const SCHEMA: &str = "openagents.dashboard.v1";
pub const PACKAGE_SCHEMA: &str = "openagents.dashboard-template.v1";
pub const TYPES: &[(&str, &str)] = &[
    ("issues", "Open issues"),
    ("my_issues", "My open issues"),
    ("agents", "Agents"),
    ("attention", "Needs attention"),
    ("recent", "Recent work"),
    ("computers", "Computers"),
];
pub fn label(kind: &str) -> &str {
    TYPES
        .iter()
        .find(|(id, _)| *id == kind)
        .map_or("Unavailable widget", |(_, name)| *name)
}
pub fn short(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
pub fn id(value: &str) -> bool {
    short(value, 96)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
pub fn repository(value: &str) -> bool {
    let Some((owner, repo)) = value.split_once('/') else {
        return false;
    };
    [owner, repo].iter().all(|v| {
        !v.is_empty()
            && v.len() <= 100
            && v.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    })
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub project: Option<String>,
    #[serde(default)]
    pub repositories: Vec<String>,
    pub board: Option<Board>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub owner: String,
    pub number: u32,
}
impl Scope {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.project.as_ref().is_some_and(|p| !id(p))
            || self.repositories.len() > 20
            || self.repositories.iter().any(|r| !repository(r))
            || self
                .board
                .as_ref()
                .is_some_and(|b| !id(&b.owner) || b.number == 0)
        {
            return Err("Choose a project, repository, or board again.");
        }
        Ok(())
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    Compact,
    #[default]
    Full,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Widget {
    pub id: String,
    pub kind: String,
    pub version: u32,
    pub size: Size,
    pub scope: Option<Scope>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub assignee: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_hours: Option<u32>,
}
impl Widget {
    pub fn new(id: impl Into<String>, kind: &str) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            version: 1,
            size: Size::Full,
            scope: None,
            labels: vec![],
            assignee: None,
            agent: None,
            host: None,
            window_hours: None,
        }
    }
    pub fn supported(&self) -> bool {
        self.version == 1 && TYPES.iter().any(|(kind, _)| *kind == self.kind)
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateRef {
    pub id: String,
    pub version: u64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dashboard {
    pub schema: String,
    pub id: String,
    pub revision: u64,
    pub name: String,
    pub scope: Scope,
    pub widgets: Vec<Widget>,
    pub source: Option<TemplateRef>,
}
impl Dashboard {
    pub fn starter(id: impl Into<String>) -> Self {
        let mut issues = Widget::new("issues", "issues");
        issues.size = Size::Compact;
        let mut mine = Widget::new("mine", "my_issues");
        mine.size = Size::Compact;
        let mut agents = Widget::new("agents", "agents");
        agents.scope = Some(Scope::default());
        Self {
            schema: SCHEMA.into(),
            id: id.into(),
            revision: 1,
            name: "My work".into(),
            scope: Scope::default(),
            widgets: vec![issues, mine, agents],
            source: None,
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA
            || !id(&self.id)
            || self.id.len() > 64
            || self.source.as_ref().is_some_and(|s| {
                s.id.len() != 64 || !s.id.bytes().all(|b| b.is_ascii_hexdigit()) || s.version == 0
            })
            || self.revision == 0
            || !short(&self.name, 80)
            || self.widgets.len() > MAX_WIDGETS
        {
            return Err("This dashboard isn't supported. Choose another layout.");
        }
        self.scope.validate()?;
        let mut ids = BTreeSet::new();
        for w in &self.widgets {
            if !id(&w.id)
                || w.id.len() > 64
                || !ids.insert(&w.id)
                || !id(&w.kind)
                || w.version == 0
                || w.labels.len() > 10
                || w.labels.iter().any(|l| !short(l, 80) || l.contains('"'))
                || w.assignee.as_ref().is_some_and(|a| !id(a))
                || w.agent.as_ref().is_some_and(|v| !short(v, 80))
                || w.host.as_ref().is_some_and(|v| !short(v, 80))
                || w.window_hours.is_some_and(|h| h == 0 || h > 720)
            {
                return Err("Check the widget settings and try again.");
            }
            if let Some(scope) = &w.scope {
                scope.validate()?;
            }
        }
        Ok(())
    }
    /// Public export removes every resource binding and free-form filter.
    pub fn export(&self, title: String) -> Result<Package, &'static str> {
        self.validate()?;
        let widgets = self
            .widgets
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let _ = i;
                let mut out = Widget::new(
                    format!(
                        "widget-{}",
                        &format!("{:x}", Sha256::digest(w.id.as_bytes()))[..16]
                    ),
                    &w.kind,
                );
                out.version = w.version;
                out.size = w.size;
                out.scope = w
                    .scope
                    .as_ref()
                    .filter(|s| **s == Scope::default())
                    .cloned();
                out
            })
            .collect();
        let mut p = Package {
            schema: PACKAGE_SCHEMA.into(),
            version: 1,
            title,
            description: String::new(),
            tags: vec![],
            widgets,
            inputs: vec!["project".into(), "repositories".into(), "board".into()],
            hash: String::new(),
        };
        p.hash = p.content_hash();
        p.validate()?;
        Ok(p)
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package {
    pub schema: String,
    pub version: u32,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub widgets: Vec<Widget>,
    pub inputs: Vec<String>,
    pub hash: String,
}
impl Package {
    /// Keep personal edits and order while applying changes to untouched widgets.
    pub fn upgrade(
        &self,
        previous: &Package,
        local: &Dashboard,
    ) -> Result<Dashboard, &'static str> {
        self.validate()?;
        previous.validate()?;
        local.validate()?;
        let mut out = local.clone();
        out.widgets.clear();
        for widget in &local.widgets {
            match previous.widgets.iter().find(|w| w.id == widget.id) {
                Some(old) if old == widget => {
                    if let Some(new) = self.widgets.iter().find(|w| w.id == widget.id) {
                        out.widgets.push(new.clone());
                    }
                }
                _ => out.widgets.push(widget.clone()),
            }
        }
        for widget in &self.widgets {
            if !previous.widgets.iter().any(|w| w.id == widget.id)
                && !out.widgets.iter().any(|w| w.id == widget.id)
            {
                out.widgets.push(widget.clone());
            }
        }
        out.validate()?;
        Ok(out)
    }
    pub fn content_hash(&self) -> String {
        let mut p = self.clone();
        p.hash.clear();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&p).unwrap_or_default())
        )
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != PACKAGE_SCHEMA
            || self.version != 1
            || !short(&self.title, 80)
            || self.description.len() > 512
            || self.description.chars().any(char::is_control)
            || self.tags.len() > 8
            || self.tags.iter().any(|t| !short(t, 32))
            || self.inputs.len() > 16
            || self
                .inputs
                .iter()
                .any(|s| !["project", "repositories", "board"].contains(&s.as_str()))
            || self.hash != self.content_hash()
            || self.widgets.iter().any(|w| {
                w.scope.as_ref().is_some_and(|s| *s != Scope::default())
                    || !w.labels.is_empty()
                    || w.assignee.is_some()
                    || w.agent.is_some()
                    || w.host.is_some()
                    || w.window_hours.is_some()
            })
        {
            return Err("This layout file isn't supported.");
        }
        let dashboard = Dashboard {
            schema: SCHEMA.into(),
            id: "preview".into(),
            revision: 1,
            name: self.title.clone(),
            scope: Scope::default(),
            widgets: self.widgets.clone(),
            source: None,
        };
        dashboard.validate()?;
        if serde_json::to_vec(self)
            .map_err(|_| "Couldn't read this layout.")?
            .len()
            > MAX_PACKAGE
        {
            return Err("This layout file is too large.");
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() > MAX_PACKAGE {
            return Err("This layout file is too large.");
        }
        let p: Self =
            serde_json::from_slice(bytes).map_err(|_| "Couldn't read this layout file.")?;
        p.validate()?;
        Ok(p)
    }
    pub fn install(
        &self,
        id: String,
        scope: Scope,
        source: Option<TemplateRef>,
    ) -> Result<Dashboard, &'static str> {
        self.validate()?;
        scope.validate()?;
        let d = Dashboard {
            schema: SCHEMA.into(),
            id,
            revision: 1,
            name: self.title.clone(),
            scope,
            widgets: self.widgets.clone(),
            source,
        };
        d.validate()?;
        Ok(d)
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Library {
    pub revision: u64,
    pub default: Option<String>,
    pub dashboards: Vec<Dashboard>,
}
impl Library {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.dashboards.len() > 32
            || serde_json::to_vec(self).map_or(true, |v| v.len() > 900 * 1024)
        {
            return Err("Remove a dashboard before adding another.");
        }
        let mut ids = BTreeSet::new();
        for d in &self.dashboards {
            d.validate()?;
            if !ids.insert(&d.id) {
                return Err("This file contains the same dashboard twice.");
            }
        }
        if self.default.as_ref().is_some_and(|id| !ids.contains(id)) {
            return Err("Choose a default dashboard.");
        }
        Ok(())
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub repositories: Vec<String>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueSummary {
    pub total: u64,
    pub complete: bool,
    pub observed_at: u64,
    pub query: String,
    pub items: Vec<Issue>,
    pub unavailable: Vec<String>,
    #[serde(default)]
    pub next_page: Option<u32>,
    #[serde(default)]
    pub board_url: Option<String>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub repository: String,
    pub number: u64,
    pub title: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    pub id: String,
    pub session: Option<String>,
    #[serde(default)]
    pub destination: Option<WorkTarget>,
    pub parent: Option<String>,
    pub agent: String,
    pub title: String,
    pub project: Option<String>,
    pub host: String,
    pub state: String,
    pub online: bool,
    pub observed_at: u64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkTarget {
    Session {
        id: String,
    },
    Chat {
        id: String,
    },
    Work {
        id: String,
    },
    MacJob {
        id: String,
    },
    AgentBoard,
    #[serde(other)]
    Unknown,
}
impl Work {
    pub fn available(&self, now: u64) -> bool {
        self.online && now.saturating_sub(self.observed_at) <= 90
    }
    pub fn state_label(&self) -> &str {
        match self.state.as_str() {
            "working" | "running" | "busy" => "Working",
            "awaiting" | "waiting_for_user" | "needs_input" | "approval" | "asking" => "Needs you",
            "queued" | "pending" => "Queued",
            "idle" => "Idle",
            "done" | "completed" | "succeeded" => "Finished",
            "failed" | "error" => "Failed",
            "cancelled" | "canceled" | "stopped" => "Stopped",
            "waiting" | "blocked" => "Waiting",
            _ => "Status unavailable",
        }
    }
    pub fn priority(&self) -> u8 {
        match self.state_label() {
            "Needs you" | "Failed" => 0,
            "Working" => 1,
            "Queued" => 2,
            "Waiting" => 3,
            "Idle" => 4,
            _ => 5,
        }
    }
}
pub fn work_rows(mut rows: Vec<Work>, project: Option<&str>) -> Vec<Work> {
    rows.retain(|w| w.parent.is_none() && project.is_none_or(|p| w.project.as_deref() == Some(p)));
    rows.sort_by(|a, b| b.observed_at.cmp(&a.observed_at));
    let mut seen = BTreeSet::new();
    rows.retain(|w| {
        seen.insert(
            w.session
                .clone()
                .unwrap_or_else(|| format!("{}:{}", w.host, w.id)),
        )
    });
    rows.sort_by_key(|w| (w.priority(), std::cmp::Reverse(w.observed_at), w.id.clone()));
    rows
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Computer {
    pub name: String,
    pub online: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkPage {
    pub data: Vec<Work>,
    pub next: Option<String>,
    pub complete: bool,
    pub computers: Vec<Computer>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Private,
    Unlisted,
    Public,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub id: String,
    pub version: u64,
    pub author: String,
    #[serde(default)]
    pub author_id: String,
    #[serde(default)]
    pub owned: bool,
    pub visibility: Visibility,
    pub package: Package,
    pub removed: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Directory {
    pub data: Vec<Listing>,
    pub next: Option<String>,
}

#[cfg(feature = "views")]
pub mod view;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrades_preserve_bindings_reordering_removal_and_custom_settings() {
        let base = Dashboard::starter("base").export("Layout".into()).unwrap();
        let mut local = base
            .install(
                "mine".into(),
                Scope {
                    project: Some("prj_personal".into()),
                    ..Scope::default()
                },
                None,
            )
            .unwrap();
        local.widgets[0].size = Size::Full;
        local.widgets.swap(0, 2);
        local.widgets.remove(1);
        let expected = local.widgets.clone();
        let mut update = base.clone();
        update.widgets.push(Widget::new("new-widget", "computers"));
        update.hash = update.content_hash();
        let upgraded = update.upgrade(&base, &local).unwrap();
        assert_eq!(&upgraded.widgets[..expected.len()], expected.as_slice());
        assert_eq!(upgraded.widgets.last().unwrap().kind, "computers");
        assert_eq!(upgraded.scope.project.as_deref(), Some("prj_personal"));
    }
    #[test]
    fn exports_strip_bindings_and_filters_and_import_is_atomic() {
        let mut d = Dashboard::starter("private");
        d.scope.project = Some("prj_private".into());
        d.scope.repositories = vec!["secret/private".into()];
        d.widgets[0].labels = vec!["private-label".into()];
        d.widgets[0].assignee = Some("private-user".into());
        let p = d.export("My layout".into()).unwrap();
        let bytes = serde_json::to_vec(&p).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        for secret in [
            "prj_private",
            "secret/private",
            "private-label",
            "private-user",
        ] {
            assert!(!text.contains(secret));
        }
        let imported = Package::decode(&bytes)
            .unwrap()
            .install(
                "other".into(),
                Scope {
                    project: Some("prj_other".into()),
                    ..Scope::default()
                },
                None,
            )
            .unwrap();
        assert_eq!(imported.scope.project.as_deref(), Some("prj_other"));
        let mut invalid = p.clone();
        invalid.widgets.push(invalid.widgets[0].clone());
        invalid.hash = invalid.content_hash();
        assert!(invalid.validate().is_err());
        assert!(Package::decode(&vec![b' '; MAX_PACKAGE + 1]).is_err());
        assert!(Package::decode(br#"{"schema":"unknown"}"#).is_err());
    }
    #[test]
    fn stale_work_retains_reported_state_and_mirrors_do_not_double_count() {
        let a = Work {
            id: "local".into(),
            session: Some("same".into()),
            state: "working".into(),
            observed_at: 1,
            ..Work::default()
        };
        assert_eq!(a.state_label(), "Working");
        let b = Work {
            id: "remote".into(),
            observed_at: 2,
            ..a.clone()
        };
        let child = Work {
            id: "child".into(),
            parent: Some("same".into()),
            ..a.clone()
        };
        let rows = work_rows(vec![a, b, child], None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "remote");
        assert!(!rows[0].online);
    }
    #[test]
    fn unknown_widgets_remain_inert_and_versions_are_checked() {
        let mut d = Dashboard::starter("home");
        d.widgets[0].kind = "future".into();
        assert!(d.validate().is_ok());
        assert!(!d.widgets[0].supported());
        d.widgets[0].version = 0;
        assert!(d.validate().is_err());
        assert!(!repository("owner/repo is:closed"));
        assert!(!repository("owner/repo/extra"));
    }
}
