use gpui::{Context, Task};
use openagents_chat_view::settings_forms::{
    Page as Section,
    connections::{self as forms, Action, Features, Google, State, Terminal as TerminalState},
};
use openagents_web_client::store;
use openagents_web_page_host::{Host, Intent, Load, Page, Paints};
use rust_native::Node;
use serde_json::{Value, json};

pub type Connections = ConnectionPage<0>;
pub type Vault = ConnectionPage<1>;
pub type Terminal = ConnectionPage<2>;

pub struct ConnectionPage<const K: u8> {
    active: bool,
    state: State,
    read: Option<Task<()>>,
    timer: Option<Task<()>>,
}
type Cx<'a, const K: u8> = Context<'a, Host<ConnectionPage<K>>>;

fn section<const K: u8>() -> Section {
    match K {
        0 => Section::Connections,
        1 => Section::Vault,
        _ => Section::Terminal,
    }
}

impl<const K: u8> Page for ConnectionPage<K> {
    const NAME: &'static str = match K {
        0 => "connections",
        1 => "vault",
        _ => "terminal",
    };
    fn new() -> Self {
        Self {
            active: false,
            state: State::default(),
            read: None,
            timer: None,
        }
    }
    fn title(&self) -> String {
        section::<K>().label().into()
    }
    fn view(&self, paints: &Paints) -> Node<Intent> {
        forms::view(section::<K>(), &self.state, paints)
    }
    fn open(host: &mut Host<Self>, _: &str, cx: &mut Cx<K>) {
        Self::hide(host, cx);
        host.page.active = true;
        host.page.state = State::default();
        let client = host.client.clone();
        cx.spawn(async move |this, cx| {
            let features = client
                .get::<Features>("/v1/settings/connection-features")
                .await;
            let _ = this.update(cx, |host, cx| {
                if let Ok(features) = features {
                    host.page.state.features = features;
                }
                host.changed(cx);
            });
        })
        .detach();
        if K != 1 {
            read(host, cx);
        }
    }
    fn hide(host: &mut Host<Self>, _: &mut Cx<K>) {
        host.page.active = false;
        host.page.read = None;
        host.page.timer = None;
    }
    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx<K>) {
        let Intent::Connections(action) = intent else {
            return;
        };
        match action {
            Action::Reload => {
                host.page.state.notice = None;
                read(host, cx);
            }
            Action::ConfirmRemove(on) => host.page.state.confirm_remove = on,
            Action::Copy(text) => host.copy(text, cx),
            Action::OpenVault => store::signal("oa-vault-open", ""),
            action => change(host, action, cx),
        }
    }
}

fn read<const K: u8>(host: &mut Host<ConnectionPage<K>>, cx: &mut Cx<K>) {
    if !host.page.active {
        return;
    }
    let path = if K == 0 {
        "/v1/connections".into()
    } else {
        match host.page.state.terminal.ready() {
            Some(terminal) => {
                let mut path = format!("/v1/terminal?since={}", terminal.since);
                if let Some(computer) = &terminal.computer {
                    path.push_str(&format!(
                        "&computer={}",
                        js_sys::encode_uri_component(computer)
                            .as_string()
                            .unwrap_or_default()
                    ));
                }
                path
            }
            None => format!(
                "/v1/terminal{}",
                web_sys::window()
                    .and_then(|w| w.location().search().ok())
                    .unwrap_or_default()
            ),
        }
    };
    let client = host.client.clone();
    host.page.read = Some(cx.spawn(async move |this, cx| {
        let answer: Result<Value, _> = client.get(&path).await;
        let _ = this.update(cx, |host, cx| {
            let state = &mut host.page.state;
            if K == 0 {
                state.google = match answer {
                    Ok(answer) => serde_json::from_value::<Google>(answer["google"].clone())
                        .map(Load::Ready)
                        .unwrap_or_else(|_| Load::Failed(String::new())),
                    Err(_) => {
                        state.notice = Some("We couldn't read your connections. Try again.".into());
                        Load::Failed(String::new())
                    }
                };
            } else {
                state.terminal = match answer {
                    Ok(answer) => serde_json::from_value::<TerminalState>(answer)
                        .map(Load::Ready)
                        .unwrap_or_else(|_| Load::Failed(String::new())),
                    Err(_) => {
                        state.notice =
                            Some("We couldn't read your terminal sign-in. Try again.".into());
                        Load::Failed(String::new())
                    }
                };
                host.page.timer = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    let _ = this.update(cx, |host, cx| {
                        if !host.page.state.busy {
                            read(host, cx);
                        }
                    });
                }));
            }
            host.changed(cx);
        });
    }));
}

fn change<const K: u8>(host: &mut Host<ConnectionPage<K>>, action: Action, cx: &mut Cx<K>) {
    if host.page.state.busy {
        return;
    }
    host.page.state.busy = true;
    host.page.state.notice = None;
    host.page.read = None;
    host.page.timer = None;
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let (method, path, body) = match &action {
            Action::Connect => ("POST", "/v1/connections/google/connect", Some(json!({"return_to":"/settings/connections?ui=gpui"}))),
            Action::Remove => ("DELETE", "/v1/connections/google", None),
            Action::Decide { code, csrf, approve } => ("POST", "/v1/terminal/approve", Some(json!({"code":code,"csrf":csrf,"decision":if *approve {"approve"} else {"deny"}}))),
            Action::Sync { computer, csrf, choice } => ("POST", "/v1/terminal/sync", Some(json!({"computer":computer,"csrf":csrf,"choice":choice}))),
            _ => return,
        };
        let answer: Result<Value, _> = client.send(method, path, body.as_ref()).await;
        let _ = this.update(cx, |host, cx| {
            host.page.state.busy = false;
            if !host.page.active { return; }
            match answer {
                Ok(answer) => match action {
                    Action::Connect => if let Some(url) = answer["url"].as_str() { host.go(url); },
                    Action::Remove => { host.page.state.confirm_remove = false; read(host, cx); }
                    Action::Decide { approve, .. } => {
                        if let Some(terminal) = host.page.state.terminal.ready().cloned() {
                            host.page.state.terminal = Load::Ready(TerminalState { computer: answer["computer"].as_str().map(str::to_owned), ..terminal });
                        }
                        if !approve { host.page.state.notice = Some("Login denied.".into()); }
                        read(host, cx);
                    }
                    Action::Sync { .. } => read(host, cx),
                    _ => {}
                },
                Err(_) => {
                    host.page.state.notice = Some(if K == 0 { "We couldn't change your Google connection. Try again." } else { "That didn't work. Try again, or run the login command again if it expired." }.into());
                    if K == 2 { read(host, cx); }
                }
            }
            host.changed(cx);
        });
    }).detach();
}
