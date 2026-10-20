//! Settings → Providers (#12283) over host RPC: the chosen computer's
//! agents (`ListHarnesses`), their update statuses live
//! (`WatchHarnessUpdates`), and its agent accounts (`ListAgentAccounts`);
//! each control's call goes to that computer, and its answer redraws the
//! page. A sign-in opens the agent's page in a new tab; the computer
//! finishes it by itself (polled with `PollAgentLogin`) or with the code
//! the person pastes (`CompleteAgentLogin`).

use futures::StreamExt as _;
use gpui::{Context, Task};
use openagents_chat_view::settings_forms::providers::{Action, Agent, Login};
use openagents_chat_view::settings_forms::{Load, providers};
use openagents_proto::{
    AgentAccountsSnapshot, AgentLoginMode, AgentLoginPoll, AgentLoginStart, AgentLoginStatus,
    HarnessUpdateStatus,
};
use openagents_web_client::computer;
use serde_json::{Value, json};

use super::{SettingsApp, plain};

mod methods {
    pub const LIST_HARNESSES: &str = "ListHarnesses";
    pub const INSTALL_HARNESS: &str = "InstallHarness";
    pub const CANCEL_INSTALL: &str = "CancelInstall";
    pub const SET_HARNESS_ENABLED: &str = "SetHarnessEnabled";
    pub const SET_HARNESS_UPDATE_POLICY: &str = "SetHarnessUpdatePolicy";
    pub const CHECK_HARNESS_UPDATES: &str = "CheckHarnessUpdates";
    pub const APPLY_HARNESS_UPDATE: &str = "ApplyHarnessUpdate";
    pub const CANCEL_HARNESS_UPDATE: &str = "CancelHarnessUpdate";
    pub const WATCH_HARNESS_UPDATES: &str = "WatchHarnessUpdates";
    pub const LIST_AGENT_ACCOUNTS: &str = "ListAgentAccounts";
    pub const START_AGENT_LOGIN: &str = "StartAgentLogin";
    pub const POLL_AGENT_LOGIN: &str = "PollAgentLogin";
    pub const COMPLETE_AGENT_LOGIN: &str = "CompleteAgentLogin";
    pub const CANCEL_AGENT_LOGIN: &str = "CancelAgentLogin";
    pub const ACTIVATE_AGENT_ACCOUNT: &str = "ActivateAgentAccount";
    pub const FORGET_AGENT_ACCOUNT: &str = "ForgetAgentAccount";
}

/// Opens `url` in a new tab, keeping Settings where it is.
fn open_tab(url: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
}

impl SettingsApp {
    /// Reads the chosen computer's agents and accounts, and follows its
    /// update statuses while the page shows.
    pub(super) fn load_providers(&mut self, cx: &mut Context<Self>) {
        let computers = self.context.computers.clone();
        if self
            .remote
            .computer
            .as_ref()
            .is_none_or(|id| !computers.iter().any(|c| &c.id == id))
        {
            self.remote.computer = computers.first().map(|c| c.id.clone());
        }
        self.remote.computers = computers;
        self.remote.providers = providers::State::default();
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        self.remote.providers.agents = Load::Loading;
        self.remote.providers.accounts = Load::Loading;
        let name = self.computer_name(&device);
        self.computer_task = Some(self.computer_calls(
            device.clone(),
            name,
            vec![
                (methods::LIST_HARNESSES, json!({})),
                (methods::LIST_AGENT_ACCOUNTS, json!({})),
            ],
            |this, mut answers, _| {
                let accounts = answers.pop().unwrap_or(Err(String::new()));
                let agents = answers.pop().unwrap_or(Err(String::new()));
                this.take_agents(agents);
                this.take_accounts(accounts);
            },
            cx,
        ));
        self.follow_updates(device, cx);
    }

    fn take_agents(&mut self, answer: Result<Value, String>) {
        self.remote.providers.agents = match answer {
            Ok(value) => {
                let list: Vec<Agent> = serde_json::from_value(value).unwrap_or_default();
                Load::Ready(
                    list.into_iter()
                        .filter(|a| a.id != openagents_proto::HarnessId::Mock)
                        .collect(),
                )
            }
            Err(error) => Load::Failed(error),
        };
    }

    fn take_accounts(&mut self, answer: Result<Value, String>) {
        self.remote.providers.accounts = match answer.and_then(|v| {
            serde_json::from_value::<AgentAccountsSnapshot>(v).map_err(|e| e.to_string())
        }) {
            Ok(snapshot) => Load::Ready(snapshot.accounts),
            Err(error) => Load::Failed(error),
        };
    }

    fn follow_updates(&mut self, device: String, cx: &mut Context<Self>) {
        let client = self.client.clone();
        self.providers_watch = Some(cx.spawn(async move |this, cx| {
            let Ok(open) = computer::connect(&client, &device, || {}).await else {
                return;
            };
            let Ok(mut watch) = open.watch(methods::WATCH_HARNESS_UPDATES, json!({})) else {
                return;
            };
            while let Some(Ok(item)) = watch.next().await {
                let Ok(statuses) = serde_json::from_value::<Vec<HarnessUpdateStatus>>(item) else {
                    continue;
                };
                let gone = this
                    .update(cx, |this, cx| {
                        if this.remote.computer.as_deref() != Some(device.as_str()) {
                            return true;
                        }
                        this.remote.providers.updates = statuses;
                        this.revision += 1;
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if gone {
                    break;
                }
            }
        }));
    }

    /// One call to the chosen computer; `then` takes its answer.
    fn provider_call(
        &mut self,
        method: &'static str,
        params: Value,
        then: impl FnOnce(&mut Self, Value, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Option<Task<()>> {
        let device = self.remote.computer.clone()?;
        let name = self.computer_name(&device);
        Some(self.computer_calls(
            device,
            name,
            vec![(method, params)],
            move |this, mut answers, cx| match answers.pop() {
                Some(Ok(value)) => {
                    this.remote.providers.notice = None;
                    then(this, value, cx)
                }
                Some(Err(error)) => this.remote.providers.notice = Some(error),
                None => {}
            },
            cx,
        ))
    }

    pub(super) fn providers_action(&mut self, action: Action, cx: &mut Context<Self>) {
        let task = match action {
            Action::Install(harness) => {
                self.remote.providers.installing = Some(harness);
                self.provider_call(
                    methods::INSTALL_HARNESS,
                    json!({ "harness": harness }),
                    |this, value, _| {
                        this.remote.providers.installing = None;
                        this.take_agents(Ok(value));
                    },
                    cx,
                )
            }
            Action::CancelInstall(harness) => {
                self.remote.providers.installing = None;
                self.provider_call(
                    methods::CANCEL_INSTALL,
                    json!({ "harness": harness }),
                    |_, _, _| {},
                    cx,
                )
            }
            Action::Enable(harness, enabled) => self.provider_call(
                methods::SET_HARNESS_ENABLED,
                json!({ "harness": harness, "enabled": enabled }),
                |this, value, _| this.take_agents(Ok(value)),
                cx,
            ),
            Action::Policy(harness, policy) => self.provider_call(
                methods::SET_HARNESS_UPDATE_POLICY,
                json!({ "harness": harness, "policy": policy }),
                |_, _, _| {},
                cx,
            ),
            Action::Check(harness) => self.provider_call(
                methods::CHECK_HARNESS_UPDATES,
                json!({ "harness": harness }),
                |this, value, _| {
                    if let Ok(statuses) = serde_json::from_value(value) {
                        this.remote.providers.updates = statuses;
                    }
                },
                cx,
            ),
            Action::Apply(harness) => self.provider_call(
                methods::APPLY_HARNESS_UPDATE,
                json!({ "harness": harness }),
                |_, _, _| {},
                cx,
            ),
            Action::CancelUpdate(harness) => self.provider_call(
                methods::CANCEL_HARNESS_UPDATE,
                json!({ "harness": harness }),
                |_, _, _| {},
                cx,
            ),
            Action::AddAccount(harness) => self.provider_call(
                methods::START_AGENT_LOGIN,
                json!({ "harness": harness }),
                move |this, value, cx| {
                    let Ok(start) = serde_json::from_value::<AgentLoginStart>(value) else {
                        return;
                    };
                    if !start.url.is_empty() {
                        open_tab(&start.url);
                    }
                    this.remote.providers.login = Some(Login {
                        harness,
                        login_id: start.login_id.clone(),
                        url: start.url,
                        code: start.mode == AgentLoginMode::PasteCode,
                        message: None,
                    });
                    this.poll_login(start.login_id, cx);
                },
                cx,
            ),
            Action::OpenLogin(url) => {
                open_tab(&url);
                None
            }
            Action::CancelLogin => {
                let login = self.remote.providers.login.take();
                self.providers_login = None;
                login.and_then(|login| {
                    self.provider_call(
                        methods::CANCEL_AGENT_LOGIN,
                        json!({ "loginId": login.login_id }),
                        |_, _, _| {},
                        cx,
                    )
                })
            }
            Action::Activate(harness, account) => self.provider_call(
                methods::ACTIVATE_AGENT_ACCOUNT,
                json!({ "harness": harness, "accountId": account }),
                |this, value, _| this.take_accounts(Ok(value)),
                cx,
            ),
            Action::ConfirmForget(pair) => {
                self.remote.providers.confirm_forget = pair;
                None
            }
            Action::Forget(harness, account) => {
                self.remote.providers.confirm_forget = None;
                self.provider_call(
                    methods::FORGET_AGENT_ACCOUNT,
                    json!({ "harness": harness, "accountId": account }),
                    |this, value, _| this.take_accounts(Ok(value)),
                    cx,
                )
            }
        };
        if let Some(task) = task {
            self.computer_task = Some(task);
        }
        self.revision += 1;
    }

    /// The pasted code for the sign-in under way.
    pub(super) fn complete_login(&mut self, code: String, cx: &mut Context<Self>) {
        let Some(login) = self.remote.providers.login.clone() else {
            return;
        };
        let code = code.trim().to_owned();
        if code.is_empty() {
            return;
        }
        if let Some(task) = self.provider_call(
            methods::COMPLETE_AGENT_LOGIN,
            json!({ "loginId": login.login_id, "code": code }),
            |this, value, _| {
                this.remote.providers.login = None;
                this.providers_login = None;
                this.take_accounts(Ok(value));
            },
            cx,
        ) {
            self.computer_task = Some(task);
        }
    }

    /// Asks the computer every two seconds whether the sign-in finished.
    fn poll_login(&mut self, login_id: String, cx: &mut Context<Self>) {
        let Some(device) = self.remote.computer.clone() else {
            return;
        };
        let client = self.client.clone();
        self.providers_login = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let answer = async {
                    let open = computer::connect(&client, &device, || {}).await?;
                    open.call(methods::POLL_AGENT_LOGIN, json!({ "loginId": login_id }))
                        .await
                }
                .await;
                let Ok(poll) = answer.map_err(|e| plain(&e)).and_then(|v| {
                    serde_json::from_value::<AgentLoginPoll>(v).map_err(|e| e.to_string())
                }) else {
                    continue;
                };
                let done = poll.status != AgentLoginStatus::Pending;
                let gone = this
                    .update(cx, |this, cx| {
                        let Some(login) = &mut this.remote.providers.login else {
                            return true;
                        };
                        if let Some(url) =
                            poll.url.filter(|u| login.url.is_empty() && !u.is_empty())
                        {
                            open_tab(&url);
                            login.url = url;
                        }
                        match poll.status {
                            AgentLoginStatus::Pending => {}
                            AgentLoginStatus::Done => {
                                this.remote.providers.login = None;
                                this.load_providers(cx);
                            }
                            AgentLoginStatus::Error => {
                                this.remote.providers.login = None;
                                this.remote.providers.notice =
                                    Some(poll.message.unwrap_or_else(|| {
                                        "The sign-in didn't finish. Try again.".into()
                                    }));
                            }
                        }
                        this.revision += 1;
                        cx.notify();
                        false
                    })
                    .unwrap_or(true);
                if gone || done {
                    break;
                }
            }
        }));
    }
}
