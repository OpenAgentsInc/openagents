//! Adapter from admitted host requests to the existing app RPC service.
use coder_access::{
    Code,
    app_rpc::{Answer, Handler, Request},
};
use openagents_rpc::{RpcError, RpcReply, RpcService};
use std::{sync::Arc, time::Duration};

/// A running app host. Dropping it closes admission before asynchronous cleanup.
pub struct Hosted {
    host: Option<coder_host::Running>,
    runtime: tokio::runtime::Handle,
    link: AccountLink,
    /// Follows the desktop identity's offer (`watch_account_link`).
    link_watch: Option<tokio::task::JoinHandle<()>>,
}

/// The account key that signs a phone's request to link this computer to
/// its account (#12000), once this computer has one, and the data directory
/// whose window offered it, where the answer is written back.
type AccountLink = Arc<std::sync::Mutex<Option<LinkSigner>>>;

#[derive(Clone)]
struct LinkSigner {
    signer: Arc<openagents_sync::account_link::Signer>,
    data: Option<std::path::PathBuf>,
}

impl LinkSigner {
    fn signed(&self) {
        use openagents_account::desktop::identity::link;
        let key = self.signer.key();
        match &self.data {
            Some(data) => link::signed_in(data, &key),
            None => link::signed(&key),
        }
    }
    fn linked(&self, key: &str) {
        use openagents_account::desktop::identity::link;
        match &self.data {
            Some(data) => link::linked_in(data, key),
            None => link::linked(key),
        }
    }
}

impl Hosted {
    pub async fn start(
        service: Arc<dyn RpcService>,
        config: coder_host::Config,
    ) -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Handle::current();
        let handler = AppRpc::new(service, runtime.clone())
            .map_err(|_| anyhow::anyhow!("This runtime can't serve the device connection."))?;
        let link = handler.link.clone();
        let host = coder_host::start(config, Arc::new(coder_host::NoTasks)).await?;
        host.set_app_rpc(Some(Arc::new(handler)));
        host.publish_reach().await;
        Ok(Self {
            host: Some(host),
            runtime,
            link,
            link_watch: None,
        })
    }

    /// Follow the desktop identity's link offer for `data_dir`: while the
    /// window offers a key, sign a scanning phone's link proof with it, and
    /// write what happened back beside the offer; otherwise sign nothing.
    ///
    /// The window may run in this process (`link::watch`) or in another
    /// one: the engine runs as its own background service, so it also polls
    /// the offer file the window writes (`link::offered_key`) and reads the
    /// key's secret from the Keychain item the file names
    /// (`link::load_offered`). The secret is never on disk. `extra` are
    /// key-link addresses beyond production's.
    pub fn watch_account_link(&mut self, data_dir: &std::path::Path, extra: Vec<String>) {
        use openagents_account::desktop::identity::link::{self, LinkState};
        use openagents_sync::account_link::Signer;
        let mut state = link::watch(data_dir);
        let slot = Arc::downgrade(&self.link);
        let data = data_dir.to_owned();
        let task = self.runtime.spawn(async move {
            // A key whose secret couldn't be read waits before the next try,
            // so a locked Keychain isn't asked every second.
            let mut failed: Option<(String, std::time::Instant)> = None;
            loop {
                let Some(slot) = slot.upgrade() else { return };
                let current = slot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                let current_key = current.as_ref().map(|c| c.signer.key());
                let here = state.borrow_and_update().clone();
                let next = match here {
                    // The window is in this process.
                    LinkState::Offered(identity) => {
                        if current_key.as_deref() == Some(identity.pubkey_hex().as_str()) {
                            current
                        } else {
                            Signer::from_nsec(&identity.nsec(), &extra).map(|signer| LinkSigner {
                                signer: Arc::new(signer),
                                data: Some(data.clone()),
                            })
                        }
                    }
                    // Keep the signer so the phone can say the link landed.
                    LinkState::Signed(key) => {
                        current.filter(|_| current_key.as_deref() == Some(key.as_str()))
                    }
                    LinkState::Linked(_) => None,
                    // Perhaps a window in another process: read its file.
                    LinkState::Idle => {
                        let at = coder_access::unix_time().unwrap_or(0);
                        match link::offered_key(&data, at) {
                            None => None,
                            Some(key) if current_key.as_deref() == Some(key.as_str()) => current,
                            Some(key)
                                if failed.as_ref().is_some_and(|(k, at)| {
                                    *k == key && at.elapsed() < std::time::Duration::from_secs(30)
                                }) =>
                            {
                                None
                            }
                            Some(key) => {
                                let (data, extra) = (data.clone(), extra.clone());
                                let loaded = tokio::task::spawn_blocking(move || {
                                    let identity = link::load_offered(&data, at)?;
                                    Signer::from_nsec(&identity.nsec(), &extra).map(|signer| {
                                        LinkSigner {
                                            signer: Arc::new(signer),
                                            data: Some(data),
                                        }
                                    })
                                })
                                .await
                                .ok()
                                .flatten();
                                failed = loaded.is_none().then(|| (key, std::time::Instant::now()));
                                loaded
                            }
                        }
                    }
                };
                *slot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
                drop(slot);
                tokio::select! {
                    changed = state.changed() => {
                        if changed.is_err() {
                            return;
                        }
                    }
                    () = tokio::time::sleep(link::POLL) => {}
                }
            }
        });
        if let Some(previous) = self.link_watch.replace(task) {
            previous.abort();
        }
    }

    /// Sign phones' account-link requests with `signer` (this computer's
    /// account key), or stop with `None`. The signer answers only a fresh
    /// connect-code grant, once (NIP-HOST "Account key link by QR").
    pub fn set_account_signer(&self, signer: Option<Arc<openagents_sync::account_link::Signer>>) {
        *self
            .link
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            signer.map(|signer| LinkSigner { signer, data: None });
    }
    pub async fn configure_sync(
        &self,
        service: Arc<dyn RpcService>,
        account: String,
        device: String,
        origin: String,
        bearer: String,
    ) -> anyhow::Result<()> {
        let secret = self.host().authority().signing_key()?;
        let public = coder_host::reach::pubkey(&secret);
        let url = format!("{}/device/{}/key", origin.trim_end_matches('/'), device);
        let body = serde_json::to_vec(&serde_json::json!({"publicKey": public}))?;
        let intent = coder_access::http_proof::Intent::new(&url, "POST", &body)?;
        let proof = coder_access::http_proof::sign(
            &secret,
            &intent,
            coder_access::unix_time()?,
            vec![("account", account.clone())],
        )?;
        let response = reqwest::Client::new()
            .post(&url)
            .timeout(Duration::from_secs(5))
            .bearer_auth(bearer)
            .header(
                coder_access::http_proof::DEVICE_HEADER,
                coder_access::http_proof::header(&proof)?,
            )
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("Couldn't register this computer's device key.");
        }
        let mut adapter = AppRpc::new(service, self.runtime.clone())
            .map_err(|_| anyhow::anyhow!("Couldn't start the device connection."))?;
        adapter.link = self.link.clone();
        adapter.sync = Some(SyncIssuer {
            secret,
            account,
            device,
            origin,
        });
        self.host().set_app_rpc(Some(Arc::new(adapter)));
        Ok(())
    }

    pub fn host(&self) -> &coder_host::Running {
        self.host.as_ref().unwrap()
    }
}
impl Drop for Hosted {
    fn drop(&mut self) {
        if let Some(watch) = self.link_watch.take() {
            watch.abort();
        }
        if let Some(host) = self.host.take() {
            host.set_app_rpc(None);
            host.stop_taking_requests();
            self.runtime.spawn(host.shutdown());
        }
    }
}

struct ProfileKeys {
    keys: Arc<dyn coder_host::serve::keys::AccountKeys>,
    scope: String,
}
impl ProfileKeys {
    fn name(&self, key: coder_host::serve::keys::KeyName) -> String {
        format!("app-device:{}:{}", self.scope, key.as_str())
    }
}
impl coder_host::serve::keys::KeySource for ProfileKeys {
    fn load(
        &self,
        key: coder_host::serve::keys::KeyName,
    ) -> openagents_connect::Result<Option<coder_host::serve::keys::Secret>> {
        self.keys.load_account(&self.name(key))
    }
    fn store(
        &self,
        key: coder_host::serve::keys::KeyName,
        secret: &coder_host::serve::keys::Secret,
    ) -> openagents_connect::Result<()> {
        self.keys.store_account(&self.name(key), secret)
    }
    fn delete(&self, key: coder_host::serve::keys::KeyName) -> openagents_connect::Result<()> {
        self.keys.delete_account(&self.name(key))
    }
}

pub(crate) fn configuration(
    root: &std::path::Path,
    label: String,
) -> anyhow::Result<coder_host::Config> {
    if cfg!(test) {
        anyhow::bail!("Tests must supply an isolated key source and host configuration.");
    }
    {
        use sha2::{Digest, Sha256};
        let generation = coder_service::generation::advance(root)?;
        let relay = openagents_brand::env_var("OPENAGENTS_NIP_HOST_RELAY")
            .unwrap_or_else(|_| "wss://relay.openagents.com".into());
        let mut config = coder_host::Config::new(root.join("access"), vec![relay], generation);
        config.label = label;
        config.telemetry = false;
        config.advertise_listener = false;
        // The iroh endpoint serves enrollment, so a connect code can pair a
        // phone without an account.
        config.iroh = Some(coder_host::config::Iroh::openagents());
        config.keys = Some(coder_host::serve::keys::Keys(
            if openagents_brand::env_var("OPENAGENTS_NIP_HOST_KEY_STORE").as_deref() == Ok("file") {
                Arc::new(coder_host::serve::keys::FileKeySource::new(
                    root.join("keys"),
                )) as Arc<dyn coder_host::serve::keys::KeySource>
            } else {
                Arc::new(ProfileKeys {
                    keys: coder_host::serve::keys::platform_accounts()?,
                    scope: format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes())),
                })
            },
        ));
        Ok(config)
    }
}

/// Install this on a Coder host with `Running::set_app_rpc`.
/// The host calls it on its blocking request worker after checking the grant.
pub struct AppRpc {
    service: Arc<dyn RpcService>,
    runtime: tokio::runtime::Handle,
    sync: Option<SyncIssuer>,
    link: AccountLink,
}

struct SyncIssuer {
    secret: secp256k1::SecretKey,
    account: String,
    device: String,
    origin: String,
}
impl AppRpc {
    pub fn new(
        service: Arc<dyn RpcService>,
        runtime: tokio::runtime::Handle,
    ) -> Result<Self, Code> {
        if runtime.runtime_flavor() != tokio::runtime::RuntimeFlavor::MultiThread {
            return Err(Code::Unsupported);
        }
        Ok(Self {
            service,
            runtime,
            sync: None,
            link: AccountLink::default(),
        })
    }
}
impl AppRpc {
    /// A paired phone asks this computer to sign its request to join the
    /// phone's account (#12000). Only a pairing grant may ask.
    fn link_account(
        &self,
        grant: Option<(&str, u64)>,
        request: &Request,
        admission: &coder_access::app_rpc::Admission<'_>,
    ) -> Result<Answer, Code> {
        request.validate().map_err(|error| error.code)?;
        if coder_access::Rights::pairing()
            .first_missing(admission.rights)
            .is_some()
        {
            return Err(Code::MissingRight);
        }
        let link = self
            .link
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or(Code::Unsupported)?;
        let now = coder_access::unix_time().map_err(|e| e.code)?;
        let (grant, _) = grant.ok_or(Code::Forbidden)?;
        let value = link
            .signer
            .sign(grant, admission.expires_at, &request.params, now)?;
        link.signed();
        let answer = Answer {
            method: request.method,
            value,
        };
        answer.validate().map_err(|error| error.code)?;
        Ok(answer)
    }

    /// The phone that got a link proof says our server accepted it: this
    /// computer's key may sign in now (#12000).
    fn account_linked(
        &self,
        grant: Option<(&str, u64)>,
        request: &Request,
        admission: &coder_access::app_rpc::Admission<'_>,
    ) -> Result<Answer, Code> {
        request.validate().map_err(|error| error.code)?;
        if coder_access::Rights::pairing()
            .first_missing(admission.rights)
            .is_some()
        {
            return Err(Code::MissingRight);
        }
        let link = self
            .link
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or(Code::Unsupported)?;
        let (grant, _) = grant.ok_or(Code::Forbidden)?;
        let key = link.signer.confirm(grant, &request.params)?;
        link.linked(&key);
        Ok(Answer {
            method: request.method,
            value: serde_json::json!({}),
        })
    }
}

impl Handler for AppRpc {
    fn call_admitted(
        &self,
        request_id: &str,
        device: &str,
        grant: Option<(&str, u64)>,
        request: &Request,
        admission: &coder_access::app_rpc::Admission<'_>,
    ) -> Result<Answer, Code> {
        if request.method == coder_access::app_rpc::Method::LinkAccount {
            return self.link_account(grant, request, admission);
        }
        if request.method == coder_access::app_rpc::Method::AccountLinked {
            return self.account_linked(grant, request, admission);
        }
        if request.method != coder_access::app_rpc::Method::AuthorizeSync {
            return self.call(request_id, device, grant, request);
        }
        let issuer = self.sync.as_ref().ok_or(Code::Unsupported)?;
        if coder_access::Rights::pairing()
            .first_missing(admission.rights)
            .is_some()
        {
            return Err(Code::MissingRight);
        }
        let intent: coder_access::http_proof::Intent =
            serde_json::from_value(request.params.clone()).map_err(|_| Code::Malformed)?;
        intent.validate().map_err(|e| e.code)?;
        if !intent.has_origin(&issuer.origin) {
            return Err(Code::Forbidden);
        }
        let now = coder_access::unix_time().map_err(|e| e.code)?;
        let expiry = admission.expires_at.min(now + 15);
        if expiry <= now {
            return Err(Code::Expired);
        }
        let signed = coder_access::http_proof::sign(
            &issuer.secret,
            &intent,
            now,
            vec![
                ("account", issuer.account.clone()),
                ("hostId", issuer.device.clone()),
                ("p", device.into()),
                ("expiration", expiry.to_string()),
            ],
        )
        .map_err(|e| e.code)?;
        Ok(Answer {
            method: request.method,
            value: serde_json::to_value(signed).map_err(|_| Code::Unavailable)?,
        })
    }

    fn call(
        &self,
        _: &str,
        _: &str,
        _: Option<(&str, u64)>,
        request: &Request,
    ) -> Result<Answer, Code> {
        request.validate().map_err(|error| error.code)?;
        let service = self.service.clone();
        let request = request.clone();
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let task = self.runtime.spawn(async move {
            let result = match service
                .handle(request.method.as_str(), request.params)
                .await
            {
                Ok(RpcReply::Value(value)) => {
                    let answer = Answer {
                        method: request.method,
                        value,
                    };
                    answer
                        .validate()
                        .map_err(|error| error.code)
                        .map(|()| answer)
                }
                Ok(RpcReply::Stream(_)) => Err(Code::Unavailable),
                Err(RpcError::UnknownMethod(_)) => Err(Code::Unsupported),
                Err(RpcError::BadParams(_)) => Err(Code::Malformed),
                Err(_) => Err(Code::Unavailable),
            };
            let _ = send.send(result);
        });
        match receive.recv_timeout(Duration::from_secs(8)) {
            Ok(result) => result,
            Err(_) => {
                task.abort();
                Err(Code::Unavailable)
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../crates/coder-control/src/tests/relay.rs"]
mod test_relay;

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::app_rpc::Method;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Service(Mutex<std::collections::BTreeSet<String>>);
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pairing_checks_computer_identity_and_revocation_and_closes_on_drop() {
        use coder_access::{RelayPolicy, Rights};
        use coder_host::serve::keys::{FileKeySource, Keys};
        let temp = tempfile::tempdir().unwrap();
        let (relay, relay_task, _) = super::test_relay::start().await;
        let mut config =
            coder_host::Config::new(temp.path().join("access"), vec![relay.clone()], 1);
        config.policy = RelayPolicy::LoopbackTest;
        config.telemetry = false;
        config.keys = Some(Keys(Arc::new(FileKeySource::new(temp.path().join("keys")))));
        let service = Arc::new(Service::default());
        let hosted = Hosted::start(service.clone(), config).await.unwrap();
        let invite = || {
            hosted
                .host()
                .authority()
                .local(|host, now| {
                    host.invite(&relay, Rights::pairing(), now, now + 3600)
                        .map(|i| i.code)
                })
                .unwrap()
        };
        assert!(
            openagents_sync::paired_device::pair_with_policy(
                &invite(),
                "fixture-app",
                &"0".repeat(64),
                RelayPolicy::LoopbackTest
            )
            .await
            .is_err()
        );
        assert!(
            openagents_sync::paired_device::pair_with_policy(
                &invite(),
                "another-app",
                hosted.host().host_key(),
                RelayPolicy::LoopbackTest
            )
            .await
            .is_err()
        );
        let channel = openagents_sync::paired_device::pair_with_policy(
            &invite(),
            "fixture-app",
            hosted.host().host_key(),
            RelayPolicy::LoopbackTest,
        )
        .await
        .unwrap();
        assert_eq!(
            channel.call("ListModels", json!({})).await.unwrap(),
            Some(json!(["fixture-model"]))
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let ws_task = tokio::spawn(openagents_rpc::serve_ws_listener(listener, service));
        let ws = openagents_rpc::connect_ws(&url).await.unwrap();
        let mut nip_times = Vec::new();
        let mut ws_times = Vec::new();
        for _ in 0..30 {
            let start = std::time::Instant::now();
            channel.call("ListModels", json!({})).await.unwrap();
            nip_times.push(start.elapsed().as_micros());
            let start = std::time::Instant::now();
            ws.call("ListModels", json!({})).await.unwrap();
            ws_times.push(start.elapsed().as_micros());
        }
        nip_times.sort();
        ws_times.sort();
        eprintln!(
            "Local loopback, 30 warm calls, microseconds: NIP-HOST relay p50={} p95={}; direct RPC WebSocket p50={} p95={}",
            nip_times[15], nip_times[28], ws_times[15], ws_times[28]
        );
        ws_task.abort();
        // The scan that paired also links this computer to the phone's
        // account (#12000). Without an account key the computer pairs only.
        use openagents_sync::account_link;
        let url = account_link::LINK_URLS[0];
        let link = json!({"url": url, "challenge": "fixture-challenge"});
        assert_eq!(
            channel.call("LinkAccount", link.clone()).await.unwrap(),
            None
        );
        let signer = Arc::new(account_link::Signer::new(
            secp256k1::SecretKey::new(&mut secp256k1::rand::rng()),
            &[],
        ));
        hosted.set_account_signer(Some(signer.clone()));
        // This fixture grant is an hour long, not a fresh connect-code
        // grant, so the computer refuses.
        let now = coder_access::unix_time().unwrap();
        assert!(
            account_link::request(&channel, url, "fixture-challenge", None, now)
                .await
                .is_err()
        );
        channel.revoke().await.unwrap();
        hosted
            .host()
            .authority()
            .local(|host, now| {
                for entry in host.devices(now)? {
                    host.revoke(&entry.device, now)?;
                }
                Ok(())
            })
            .unwrap();
        assert!(matches!(
            channel.call("ListModels", json!({})).await,
            Err(openagents_sync::device_channel::Failure::Refused(
                Code::Revoked
            ))
        ));
        let address = hosted.host().local_addr();
        drop(channel);
        drop(hosted);
        tokio::time::timeout(Duration::from_secs(5), async {
            while tokio::net::TcpStream::connect(address).await.is_ok() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        relay_task.abort();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sync_proofs_need_pairing_rights_and_expire_with_the_grant() {
        use coder_access::{
            Right, Rights,
            app_rpc::{Admission, Method},
            http_proof::{self, Intent},
        };
        let mut adapter = AppRpc::new(
            Arc::new(Service::default()),
            tokio::runtime::Handle::current(),
        )
        .unwrap();
        adapter.sync = Some(SyncIssuer {
            secret: secp256k1::SecretKey::new(&mut secp256k1::rand::rng()),
            account: "account".into(),
            device: "fixture-app".into(),
            origin: "https://sync.example".into(),
        });
        let now = coder_access::unix_time().unwrap();
        let intent = Intent::new(
            "https://sync.example/chat2/chat/ws?device=phone",
            "GET",
            b"",
        )
        .unwrap();
        let request = Request {
            method: Method::AuthorizeSync,
            params: serde_json::to_value(&intent).unwrap(),
        };
        let observe = Rights::new([Right::Observe]).unwrap();
        assert_eq!(
            adapter
                .call_admitted(
                    "request",
                    "phone-key",
                    None,
                    &request,
                    &Admission {
                        rights: &observe,
                        expires_at: now + 30
                    }
                )
                .unwrap_err(),
            Code::MissingRight
        );
        let all = Rights::pairing();
        let answer = adapter
            .call_admitted(
                "request",
                "phone-key",
                None,
                &request,
                &Admission {
                    rights: &all,
                    expires_at: now + 3,
                },
            )
            .unwrap();
        let event: nostr::domain::Event = serde_json::from_value(answer.value).unwrap();
        let event = http_proof::verify(&http_proof::header(&event).unwrap(), &intent, now).unwrap();
        assert_eq!(
            http_proof::tag(&event, "expiration").unwrap(),
            (now + 3).to_string()
        );
        assert_eq!(http_proof::tag(&event, "p").unwrap(), "phone-key");
        let other = Request {
            method: Method::AuthorizeSync,
            params: serde_json::to_value(
                Intent::new("https://other.example/chat2/chat/ws", "GET", b"").unwrap(),
            )
            .unwrap(),
        };
        assert_eq!(
            adapter
                .call_admitted(
                    "request",
                    "phone-key",
                    None,
                    &other,
                    &Admission {
                        rights: &all,
                        expires_at: now + 30
                    }
                )
                .unwrap_err(),
            Code::Forbidden
        );
    }
    #[async_trait::async_trait]
    impl RpcService for Service {
        async fn handle(
            &self,
            method: &str,
            params: serde_json::Value,
        ) -> Result<RpcReply, RpcError> {
            match method {
                openagents_rpc::methods::ENGINE_INFO => {
                    RpcReply::value(&json!({"deviceId":"fixture-app"}))
                }
                openagents_rpc::methods::LIST_MODELS => RpcReply::value(&json!(["fixture-model"])),
                openagents_rpc::methods::RELAY_COMMAND => {
                    let id = params["entry"]["id"].as_str().unwrap().to_owned();
                    let fresh = self.0.lock().unwrap().insert(id);
                    RpcReply::value(&json!({"fresh": fresh}))
                }
                _ => Err(RpcError::UnknownMethod(method.into())),
            }
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dispatch_keeps_the_method_and_the_original_command_id() {
        let service = Arc::new(Service::default());
        let adapter = AppRpc::new(service.clone(), tokio::runtime::Handle::current()).unwrap();
        tokio::task::spawn_blocking(move || {
            let read = Request {
                method: Method::ListModels,
                params: json!({}),
            };
            assert_eq!(
                adapter
                    .call("request", "device", None, &read)
                    .unwrap()
                    .value,
                json!(["fixture-model"])
            );
            let command = Request {
                method: Method::RelayCommand,
                params: json!({"chatId":"chat", "entry":{"id":"original-command"}}),
            };
            assert_eq!(
                adapter
                    .call("first", "device", None, &command)
                    .unwrap()
                    .value,
                json!({"fresh":true})
            );
            assert_eq!(
                adapter
                    .call("retry", "device", None, &command)
                    .unwrap()
                    .value,
                json!({"fresh":false})
            );
        })
        .await
        .unwrap();
        assert_eq!(
            *service.0.lock().unwrap(),
            std::collections::BTreeSet::from(["original-command".into()])
        );
    }
}
