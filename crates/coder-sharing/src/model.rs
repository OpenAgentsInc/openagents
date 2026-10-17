//! One Rust Native form for owner controls and the recipient's shared projects.
use crate::{Computer, Error, Job, Recipient, Result, failed};
use coder_access::{
    Code,
    protocol::{DeviceEntry, DeviceState},
};
use rust_native::{Activation, Axis, Element, FieldChange, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize, Deserialize)]
enum Intent {
    Home,
    Connect(bool),
    ConnectNow,
    Code,
    Name,
    Recipient,
    Project(String),
    Write(bool),
    Days(u64),
    Manage(String),
    Open(String, String),
    Invite,
    CopyRecipient,
    CopyInvitation,
    Revoke(String),
    Path,
    Contents,
    Script,
    ReadFile,
    WriteFile,
    Run,
    Stop,
    Job(String),
    Forget(String),
    Refresh,
    HomePage(bool),
    ProjectPage(bool),
    SharePage(bool),
}
#[derive(Clone)]
pub enum Screen {
    Home,
    Connect { shared: bool },
    Manage { id: String },
    Project { id: String, project: String },
}
#[derive(Clone)]
pub enum Effect {
    Refresh,
    Connect {
        code: String,
        label: String,
        shared: bool,
    },
    Manage(String),
    Invite {
        id: String,
        operation: String,
        recipient: Recipient,
        projects: Vec<String>,
        can_run: bool,
        expires_at: u64,
    },
    Revoke {
        id: String,
        operation: String,
        grant: String,
    },
    Copy(String),
    ReadFile {
        id: String,
        project: String,
        path: String,
    },
    WriteFile {
        id: String,
        project: String,
        path: String,
        bytes: Vec<u8>,
    },
    Run {
        id: String,
        project: String,
        job: String,
        script: String,
    },
    Stop(String),
    Job(String),
    Forget(String),
}

pub struct Model {
    instance: String,
    revision: u64,
    pub screen: Screen,
    pub busy: bool,
    pub notice: Option<String>,
    recipient: Recipient,
    computers: Vec<Computer>,
    pub jobs: Vec<Job>,
    contacts: BTreeMap<String, String>,
    home_page: usize,
    project_page: usize,
    share_page: usize,
    code: String,
    label: String,
    recipient_code: String,
    projects: Vec<String>,
    selected: BTreeSet<String>,
    can_run: bool,
    days: u64,
    operation: String,
    shares: Vec<DeviceEntry>,
    invitation: Option<(String, u64)>,
    invite_expiry: Option<u64>,
    path: String,
    contents: String,
    script: String,
    pub output: String,
    pub active_job: Option<String>,
    pub running: bool,
}
impl Model {
    pub fn new(recipient: Recipient) -> Self {
        Self {
            instance: format!("sharing-{}", coder_access::protocol::random_id()),
            revision: 1,
            screen: Screen::Home,
            busy: false,
            notice: None,
            recipient,
            computers: vec![],
            jobs: vec![],
            contacts: BTreeMap::new(),
            home_page: 0,
            project_page: 0,
            share_page: 0,
            code: String::new(),
            label: "Computer".into(),
            recipient_code: String::new(),
            projects: vec![],
            selected: BTreeSet::new(),
            can_run: false,
            days: 1,
            operation: coder_access::protocol::random_id(),
            shares: vec![],
            invitation: None,
            invite_expiry: None,
            path: "README.md".into(),
            contents: String::new(),
            script: String::new(),
            output: String::new(),
            active_job: None,
            running: false,
        }
    }
    pub fn refresh(
        &mut self,
        computers: Vec<Computer>,
        jobs: Vec<Job>,
        contacts: BTreeMap<String, String>,
    ) {
        self.computers = computers;
        self.jobs = jobs;
        self.contacts = contacts;
        self.home_page = self
            .home_page
            .min(self.computers.len().saturating_sub(1) / 16);
        self.touch();
    }
    pub fn managed(&mut self, id: &str, projects: Vec<String>, shares: Vec<DeviceEntry>) {
        if matches!(&self.screen,Screen::Manage {id:current} if current==id) {
            self.projects = projects;
            self.shares = shares;
            self.project_page = self
                .project_page
                .min(self.projects.len().saturating_sub(1) / 32);
            self.share_page = self.share_page.min(
                self.shares
                    .iter()
                    .filter(|share| share.state == DeviceState::Active)
                    .count()
                    .saturating_sub(1)
                    / 16,
            );
            self.busy = false;
            self.touch();
        }
    }
    pub fn connected(&mut self) {
        self.screen = Screen::Home;
        self.code.clear();
        self.busy = false;
        self.touch();
    }
    pub fn invited(&mut self, code: String, expires_at: u64) {
        self.invitation = Some((code, expires_at));
        self.busy = false;
        self.touch();
    }
    pub fn failed(&mut self, error: &Error) {
        if error.code == Code::Expired {
            self.reset_request();
        }
        self.notice = Some(error.message.clone());
        self.busy = false;
        self.touch();
    }
    pub fn file(&mut self, bytes: &[u8]) {
        self.busy = false;
        match std::str::from_utf8(bytes) {
            Ok(text)
                if text.len() <= 32 * 1024
                    && !text
                        .chars()
                        .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t')) =>
            {
                self.contents = text.into()
            }
            _ => {
                self.contents.clear();
                self.notice = Some("Open this file on a computer to read or edit it.".into());
            }
        }
        self.touch();
    }
    pub fn command(&mut self, id: String) {
        self.active_job = Some(id);
        self.running = true;
        self.busy = false;
        self.output.clear();
        self.touch();
    }
    pub fn append_output(&mut self, out: &[u8], err: &[u8], finished: bool, detail: Option<&str>) {
        let remaining = (32 * 1024usize).saturating_sub(self.output.len());
        let value = String::from_utf8_lossy(out);
        self.output.extend(value.chars().scan(0, |used, ch| {
            *used += ch.len_utf8();
            (*used <= remaining).then_some(ch)
        }));
        let remaining = (32 * 1024usize).saturating_sub(self.output.len());
        let value = String::from_utf8_lossy(err);
        self.output.extend(value.chars().scan(0, |used, ch| {
            *used += ch.len_utf8();
            (*used <= remaining).then_some(ch)
        }));
        if finished {
            self.running = false;
            self.busy = false;
            self.operation = coder_access::protocol::random_id();
        }
        if let Some(detail) = detail {
            self.notice = Some(detail.into());
        }
        self.touch();
    }
    pub fn idle(&mut self) {
        self.busy = false;
        self.touch();
    }
    fn touch(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    fn reset_request(&mut self) {
        self.operation = coder_access::protocol::random_id();
        self.invitation = None;
        self.invite_expiry = None;
        self.notice = None;
    }
    fn computer(&self, id: &str) -> Option<&Computer> {
        self.computers.iter().find(|computer| computer.id == id)
    }
    fn editable(&self) -> bool {
        !self.busy && !self.running
    }
    fn view(&self, now: u64) -> Result<ValidatedView<Intent>> {
        let mut children = vec![];
        let editable = self.editable();
        if !matches!(self.screen, Screen::Home) {
            children.push(button("back", "Back", !self.busy, Intent::Home));
        }
        match &self.screen {
            Screen::Home => {
                children.push(text("heading", "Project sharing", TextRole::Heading));
                children.push(button(
                    "recipient",
                    "Copy my device code",
                    true,
                    Intent::CopyRecipient,
                ));
                children.push(button(
                    "accept",
                    "Accept a shared project",
                    editable,
                    Intent::Connect(true),
                ));
                children.push(button(
                    "connect",
                    "Connect a computer I manage",
                    editable,
                    Intent::Connect(false),
                ));
                children.push(button("refresh", "Refresh", !self.busy, Intent::Refresh));
                for (index, computer) in self
                    .computers
                    .iter()
                    .enumerate()
                    .skip(self.home_page * 16)
                    .take(16)
                {
                    children.push(text(
                        &format!("computer-{index}"),
                        &computer.label,
                        TextRole::Heading,
                    ));
                    let current = computer.expires_at > now;
                    if !current {
                        children.push(text(
                            &format!("expired-{index}"),
                            "Access expired",
                            TextRole::Status,
                        ));
                    }
                    if computer.shared {
                        for (project_index, project) in computer.projects.iter().enumerate() {
                            children.push(button(
                                &format!("open-{index}-{project_index}"),
                                project,
                                current && editable,
                                Intent::Open(computer.id.clone(), project.clone()),
                            ));
                        }
                    } else {
                        children.push(button(
                            &format!("manage-{index}"),
                            "Share projects",
                            current && editable,
                            Intent::Manage(computer.id.clone()),
                        ));
                    }
                    children.push(button(
                        &format!("forget-{index}"),
                        "Remove from list",
                        editable,
                        Intent::Forget(computer.id.clone()),
                    ));
                }
                children.push(button(
                    "previous-computers",
                    "Previous computers",
                    self.home_page > 0 && !self.busy,
                    Intent::HomePage(false),
                ));
                children.push(button(
                    "next-computers",
                    "More computers",
                    (self.home_page + 1) * 16 < self.computers.len() && !self.busy,
                    Intent::HomePage(true),
                ));
                for (index, job) in self
                    .jobs
                    .iter()
                    .filter(|job| !job.finished)
                    .take(32)
                    .enumerate()
                {
                    children.push(button(
                        &format!("job-{index}"),
                        &format!("Check command in {}", job.project),
                        !self.busy,
                        Intent::Job(job.id.clone()),
                    ));
                }
            }
            Screen::Connect { shared } => {
                children.push(text(
                    "heading",
                    if *shared {
                        "Accept a shared project"
                    } else {
                        "Connect your computer"
                    },
                    TextRole::Heading,
                ));
                children.push(field(
                    "code",
                    if *shared {
                        "Project invitation"
                    } else {
                        "Owner's connect code"
                    },
                    &self.code,
                    16 * 1024,
                    true,
                    editable,
                    Intent::Code,
                ));
                children.push(field(
                    "name",
                    "Computer name",
                    &self.label,
                    80,
                    false,
                    editable,
                    Intent::Name,
                ));
                children.push(button(
                    "connect-now",
                    "Connect",
                    editable && !self.code.trim().is_empty() && !self.label.trim().is_empty(),
                    Intent::ConnectNow,
                ));
            }
            Screen::Manage { id } => {
                let computer = self
                    .computer(id)
                    .ok_or_else(|| failed("Choose a connected computer."))?;
                children.push(text(
                    "heading",
                    &format!("Share projects on {}", computer.label),
                    TextRole::Heading,
                ));
                children.push(field(
                    "recipient-code",
                    "Recipient's device code",
                    &self.recipient_code,
                    2048,
                    true,
                    editable,
                    Intent::Recipient,
                ));
                for (index, project) in self
                    .projects
                    .iter()
                    .enumerate()
                    .skip(self.project_page * 32)
                    .take(32)
                {
                    children.push(choice(
                        &format!("project-{index}"),
                        project,
                        self.selected.contains(project),
                        editable && (self.selected.contains(project) || self.selected.len() < 16),
                        Intent::Project(project.clone()),
                    ));
                }
                children.push(button(
                    "previous-projects",
                    "Previous projects",
                    self.project_page > 0 && editable,
                    Intent::ProjectPage(false),
                ));
                children.push(button(
                    "next-projects",
                    "More projects",
                    (self.project_page + 1) * 32 < self.projects.len() && editable,
                    Intent::ProjectPage(true),
                ));
                children.push(choice(
                    "read",
                    "Read files",
                    !self.can_run,
                    editable,
                    Intent::Write(false),
                ));
                children.push(choice(
                    "write",
                    "Edit files and run commands",
                    self.can_run,
                    editable,
                    Intent::Write(true),
                ));
                for days in [1, 7, 30] {
                    let allowed = now.saturating_add(days * 86400) <= computer.expires_at;
                    children.push(choice(
                        &format!("days-{days}"),
                        &format!("{days} {}", if days == 1 { "day" } else { "days" }),
                        self.days == days,
                        editable && allowed,
                        Intent::Days(days),
                    ));
                }
                let recipient = Recipient::parse(&self.recipient_code).ok();
                if let Some(recipient) = &recipient {
                    children.push(text(
                        "person",
                        &format!("Share with {}", recipient.label),
                        TextRole::Body,
                    ));
                }
                let can_invite = editable
                    && !self.selected.is_empty()
                    && recipient.is_some()
                    && now.saturating_add(self.days * 86400) <= computer.expires_at;
                children.push(button(
                    "invite",
                    "Create invitation",
                    can_invite,
                    Intent::Invite,
                ));
                if let Some((_, expires)) = &self.invitation {
                    children.push(button(
                        "copy-invitation",
                        if now < *expires {
                            "Copy invitation"
                        } else {
                            "Invitation expired"
                        },
                        now < *expires && editable,
                        Intent::CopyInvitation,
                    ));
                }
                for (index, share) in self
                    .shares
                    .iter()
                    .filter(|share| share.state == DeviceState::Active)
                    .enumerate()
                    .skip(self.share_page * 16)
                    .take(16)
                {
                    let Some(scope) = &share.scope else { continue };
                    let person = self
                        .contacts
                        .get(&scope.account)
                        .map(String::as_str)
                        .unwrap_or(&scope.account);
                    children.push(text(
                        &format!("share-{index}"),
                        &format!(
                            "{}: {}",
                            person,
                            scope
                                .roots
                                .iter()
                                .map(|root| root.workspace.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        TextRole::Body,
                    ));
                    children.push(button(
                        &format!("revoke-{index}"),
                        "Remove access",
                        editable,
                        Intent::Revoke(share.grant.clone()),
                    ));
                }
                let count = self
                    .shares
                    .iter()
                    .filter(|share| share.state == DeviceState::Active)
                    .count();
                children.push(button(
                    "previous-shares",
                    "Previous people",
                    self.share_page > 0 && editable,
                    Intent::SharePage(false),
                ));
                children.push(button(
                    "next-shares",
                    "More people",
                    (self.share_page + 1) * 16 < count && editable,
                    Intent::SharePage(true),
                ));
            }
            Screen::Project { id, project } => {
                let computer = self
                    .computer(id)
                    .ok_or_else(|| failed("Choose a connected computer."))?;
                children.push(text("heading", project, TextRole::Heading));
                children.push(field(
                    "path",
                    "File path",
                    &self.path,
                    1024,
                    false,
                    editable,
                    Intent::Path,
                ));
                children.push(button(
                    "read-file",
                    "Read file",
                    editable && !self.path.is_empty(),
                    Intent::ReadFile,
                ));
                children.push(field(
                    "contents",
                    "File contents",
                    &self.contents,
                    32 * 1024,
                    true,
                    editable && computer.can_run,
                    Intent::Contents,
                ));
                if computer.can_run {
                    children.push(button(
                        "write-file",
                        "Save file",
                        editable && !self.path.is_empty(),
                        Intent::WriteFile,
                    ));
                    children.push(field(
                        "script",
                        "Command",
                        &self.script,
                        32 * 1024,
                        true,
                        editable,
                        Intent::Script,
                    ));
                    children.push(button(
                        "run",
                        "Run command",
                        editable && !self.script.trim().is_empty(),
                        Intent::Run,
                    ));
                    if self.running {
                        children.push(button("stop", "Stop command", !self.busy, Intent::Stop));
                    }
                }
                if !self.output.is_empty() {
                    let mut start = 0;
                    let mut index = 0;
                    while start < self.output.len() {
                        let mut end = (start + 60 * 1024).min(self.output.len());
                        while !self.output.is_char_boundary(end) {
                            end -= 1;
                        }
                        children.push(text(
                            &format!("output-{index}"),
                            &self.output[start..end],
                            TextRole::Code,
                        ));
                        start = end;
                        index += 1;
                    }
                }
            }
        }
        if self.busy {
            children.push(text("busy", "Working…", TextRole::Status));
        }
        if let Some(notice) = &self.notice {
            children.push(text("notice", notice, TextRole::Status));
        }
        View::new_v3(
            &self.instance,
            self.revision,
            node(
                "root",
                Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            ),
        )
        .validate()
        .map_err(|_| failed("The sharing form could not be displayed."))
    }
    pub fn json(&self, now: u64) -> Result<String> {
        String::from_utf8(
            self.view(now)?
                .to_json()
                .map_err(|_| failed("The sharing form could not be displayed."))?,
        )
        .map_err(|_| failed("The sharing form could not be displayed."))
    }
    pub fn change(&mut self, event: FieldChange, now: u64) -> Result<()> {
        let intent = self.view(now)?.change_field(&event).map_err(stale)?.clone();
        match intent {
            Intent::Code => self.code = event.value,
            Intent::Name => self.label = event.value,
            Intent::Recipient => self.recipient_code = event.value,
            Intent::Path => self.path = event.value,
            Intent::Contents => self.contents = event.value,
            Intent::Script => self.script = event.value,
            _ => return Err(stale(rust_native::ViewError::NotInteractive)),
        }
        self.reset_request();
        self.touch();
        Ok(())
    }
    pub fn activate(&mut self, event: Activation, now: u64) -> Result<Option<Effect>> {
        let intent = self.view(now)?.activate(&event).map_err(stale)?.clone();
        let effect = match intent {
            Intent::Home => {
                self.screen = Screen::Home;
                None
            }
            Intent::Connect(shared) => {
                self.screen = Screen::Connect { shared };
                self.code.clear();
                None
            }
            Intent::ConnectNow => {
                let Screen::Connect { shared } = self.screen else {
                    return Err(failed("Open the connection form."));
                };
                Some(Effect::Connect {
                    code: self.code.clone(),
                    label: self.label.clone(),
                    shared,
                })
            }
            Intent::Manage(id) => {
                self.screen = Screen::Manage { id: id.clone() };
                self.projects.clear();
                self.selected.clear();
                self.shares.clear();
                self.reset_request();
                Some(Effect::Manage(id))
            }
            Intent::Open(id, project) => {
                self.screen = Screen::Project { id, project };
                self.contents.clear();
                self.output.clear();
                self.active_job = None;
                None
            }
            Intent::Project(project) => {
                if !self.selected.remove(&project) {
                    self.selected.insert(project);
                }
                self.reset_request();
                None
            }
            Intent::Write(value) => {
                self.can_run = value;
                self.reset_request();
                None
            }
            Intent::Days(value) => {
                self.days = value;
                self.reset_request();
                None
            }
            Intent::Invite => {
                if self
                    .invitation
                    .as_ref()
                    .is_some_and(|(_, expiry)| now >= *expiry)
                {
                    self.reset_request();
                }
                let expires_at = *self
                    .invite_expiry
                    .get_or_insert(now.saturating_add(self.days * 86400));
                let Screen::Manage { id } = &self.screen else {
                    return Err(failed("Choose a computer to share."));
                };
                Some(Effect::Invite {
                    id: id.clone(),
                    operation: self.operation.clone(),
                    recipient: Recipient::parse(&self.recipient_code)?,
                    projects: self.selected.iter().cloned().collect(),
                    can_run: self.can_run,
                    expires_at,
                })
            }
            Intent::CopyRecipient => Some(Effect::Copy(self.recipient.encode())),
            Intent::CopyInvitation => self
                .invitation
                .as_ref()
                .map(|(code, _)| Effect::Copy(code.clone())),
            Intent::Revoke(grant) => {
                let Screen::Manage { id } = &self.screen else {
                    return Err(failed("Choose a computer to share."));
                };
                Some(Effect::Revoke {
                    id: id.clone(),
                    operation: coder_access::protocol::random_id(),
                    grant,
                })
            }
            Intent::ReadFile | Intent::WriteFile | Intent::Run => {
                let Screen::Project { id, project } = &self.screen else {
                    return Err(failed("Choose a shared project."));
                };
                Some(match intent {
                    Intent::ReadFile => Effect::ReadFile {
                        id: id.clone(),
                        project: project.clone(),
                        path: self.path.clone(),
                    },
                    Intent::WriteFile => Effect::WriteFile {
                        id: id.clone(),
                        project: project.clone(),
                        path: self.path.clone(),
                        bytes: self.contents.as_bytes().to_vec(),
                    },
                    _ => Effect::Run {
                        id: id.clone(),
                        project: project.clone(),
                        job: self.operation.clone(),
                        script: self.script.clone(),
                    },
                })
            }
            Intent::Stop => self.active_job.clone().map(Effect::Stop),
            Intent::Job(job) => {
                if let Some(record) = self.jobs.iter().find(|record| record.id == job) {
                    self.screen = Screen::Project {
                        id: record.connection.clone(),
                        project: record.project.clone(),
                    };
                    self.output.clear();
                    self.script = record.spec.command.clone();
                    self.operation = record.id.clone();
                    self.active_job = Some(job.clone());
                    self.running = !record.finished;
                }
                Some(Effect::Job(job))
            }
            Intent::Forget(id) => Some(Effect::Forget(id)),
            Intent::Refresh => Some(Effect::Refresh),
            Intent::HomePage(next) => {
                self.home_page = if next {
                    self.home_page + 1
                } else {
                    self.home_page.saturating_sub(1)
                };
                None
            }
            Intent::ProjectPage(next) => {
                self.project_page = if next {
                    self.project_page + 1
                } else {
                    self.project_page.saturating_sub(1)
                };
                None
            }
            Intent::SharePage(next) => {
                self.share_page = if next {
                    self.share_page + 1
                } else {
                    self.share_page.saturating_sub(1)
                };
                None
            }
            _ => None,
        };
        self.notice = None;
        if effect
            .as_ref()
            .is_some_and(|effect| !matches!(effect, Effect::Copy(_)))
        {
            self.busy = true;
        }
        self.touch();
        Ok(effect)
    }
}
fn node(key: &str, element: Element<Intent>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: &str, value: &str, role: TextRole) -> Node<Intent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}
fn button(key: &str, label: &str, enabled: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent,
        },
    )
}
fn field(
    key: &str,
    label: &str,
    value: &str,
    max_bytes: usize,
    multiline: bool,
    enabled: bool,
    on_change: Intent,
) -> Node<Intent> {
    node(
        key,
        Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: String::new(),
            secret: false,
            multiline,
            enabled,
            max_bytes,
            on_change,
        },
    )
}
fn choice(key: &str, label: &str, selected: bool, enabled: bool, intent: Intent) -> Node<Intent> {
    node(
        key,
        Element::Choice {
            label: label.into(),
            selected,
            enabled,
            intent,
            children: vec![],
        },
    )
}
fn stale(_: rust_native::ViewError) -> Error {
    Error {
        code: Code::Stale,
        message: "This form changed. Try the action again.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(write: bool) -> Model {
        let mut model = Model::new(Recipient {
            account: "alice".into(),
            label: "Alice".into(),
            device: "a".repeat(64),
        });
        model.refresh(
            vec![Computer {
                id: "computer".into(),
                label: "Laptop".into(),
                shared: true,
                projects: vec!["project".into()],
                expires_at: 10_000_000,
                can_run: write,
            }],
            vec![],
            Default::default(),
        );
        model
    }
    fn activate(model: &mut Model, key: &str, now: u64) -> Result<Option<Effect>> {
        model.activate(
            Activation {
                instance: model.instance.clone(),
                revision: model.revision,
                node: key.into(),
            },
            now,
        )
    }
    #[test]
    fn read_only_guests_have_no_write_command_or_owner_controls() {
        let mut model = model(false);
        activate(&mut model, "open-0-0", 100).unwrap();
        let json = model.json(100).unwrap();
        assert!(
            !json.contains("Save file")
                && !json.contains("Run command")
                && !json.contains("Remove access")
        );
        for key in ["run", "write-file", "manage-0", "contents"] {
            assert!(activate(&mut model, key, 100).is_err());
        }
        assert!(activate(&mut model, "read-file", 100).unwrap().is_some());
    }
    #[test]
    fn stale_native_events_cannot_act_on_a_new_form() {
        let mut model = model(true);
        let event = Activation {
            instance: model.instance.clone(),
            revision: model.revision,
            node: "open-0-0".into(),
        };
        model.refresh(vec![], vec![], Default::default());
        assert!(model.activate(event, 100).is_err());
        let other = Model::new(model.recipient.clone());
        assert!(
            model
                .activate(
                    Activation {
                        instance: other.instance,
                        revision: model.revision,
                        node: "recipient".into()
                    },
                    100
                )
                .is_err()
        );
    }
    #[test]
    fn uncertain_invites_retry_the_same_body_and_expiry_without_disclosing_the_code() {
        let mut model = model(true);
        model.computers[0].shared = false;
        activate(&mut model, "manage-0", 100).unwrap();
        model.managed("computer", vec!["project".into()], vec![]);
        model.recipient_code = model.recipient.encode();
        activate(&mut model, "project-0", 100).unwrap();
        let Some(Effect::Invite {
            operation,
            expires_at,
            ..
        }) = activate(&mut model, "invite", 100).unwrap()
        else {
            panic!()
        };
        model.failed(&failed("Try again later."));
        let Some(Effect::Invite {
            operation: second,
            expires_at: second_expiry,
            ..
        }) = activate(&mut model, "invite", 200).unwrap()
        else {
            panic!()
        };
        assert_eq!(operation, second);
        assert_eq!(expires_at, second_expiry);
        model.invited("secret-invitation".into(), 500);
        assert!(!model.json(200).unwrap().contains("secret-invitation"));
        assert!(
            matches!(activate(&mut model, "copy-invitation", 200).unwrap(), Some(Effect::Copy(code)) if code=="secret-invitation")
        );
        assert!(activate(&mut model, "copy-invitation", 500).is_err());
        let Some(Effect::Invite {
            operation: third, ..
        }) = activate(&mut model, "invite", 500).unwrap()
        else {
            panic!()
        };
        assert_ne!(operation, third);
    }
    #[test]
    fn sharing_expiry_cannot_outlive_the_owner_and_pages_keep_projects_reachable() {
        let mut model = model(true);
        model.computers[0].shared = false;
        model.computers[0].expires_at = 100 + 2 * 86400;
        activate(&mut model, "manage-0", 100).unwrap();
        model.managed(
            "computer",
            (0..70).map(|n| format!("project-{n}")).collect(),
            vec![],
        );
        assert!(activate(&mut model, "days-7", 100).is_err());
        activate(&mut model, "next-projects", 100).unwrap();
        activate(&mut model, "next-projects", 100).unwrap();
        assert!(model.json(100).unwrap().contains("project-69"));
        activate(&mut model, "project-69", 100).unwrap();
        assert!(model.selected.contains("project-69"));
    }
    #[test]
    fn reopening_an_unconfirmed_command_reuses_its_saved_identity() {
        let mut model = model(true);
        let id = "b".repeat(64);
        model.jobs.push(Job {
            id: id.clone(),
            connection: "computer".into(),
            project: "project".into(),
            computer: "attachment".into(),
            finished: false,
            spec: computer_driver::CommandSpec {
                id: id.clone(),
                command: "printf once".into(),
                cwd: ".".into(),
                credential_names: Default::default(),
                env: Default::default(),
                timeout_seconds: 300,
                digest: "c".repeat(64),
            },
        });
        assert!(
            matches!(activate(&mut model, "job-0", 100).unwrap(), Some(Effect::Job(job)) if job==id)
        );
        model.idle();
        model.running = false; // The host reports that this start is absent.
        let Some(Effect::Run { job, script, .. }) = activate(&mut model, "run", 100).unwrap()
        else {
            panic!()
        };
        assert_eq!(job, id);
        assert_eq!(script, "printf once");
    }
    #[test]
    fn file_and_command_output_stay_bounded_and_validated() {
        let mut model = model(true);
        activate(&mut model, "open-0-0", 100).unwrap();
        model.file(&[0xff]);
        assert!(model.contents.is_empty());
        model.append_output(&vec![b'x'; 100_000], b"more", true, None);
        assert_eq!(model.output.len(), 32 * 1024);
        assert!(model.json(100).unwrap().len() < 512 * 1024);
        let value: serde_json::Value = serde_json::from_str(&model.json(100).unwrap()).unwrap();
        fn check(node: &serde_json::Value) {
            if let Some(value) = node.as_object() {
                for (key, child) in value {
                    if ["label", "value", "placeholder"].contains(&key.as_str()) {
                        if let Some(text) = child.as_str() {
                            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
                        }
                    }
                    check(child);
                }
            } else if let Some(items) = node.as_array() {
                for item in items {
                    check(item);
                }
            }
        }
        check(&value);
    }
}
