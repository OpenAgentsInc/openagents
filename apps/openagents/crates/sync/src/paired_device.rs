//! A session-scoped pairing. Sign-out drops the link and its private key.
use crate::device_channel::{Answer, DeviceChannel, HostError, HostLink, Request};
use coder_access::{Code, RelayPolicy, app_rpc::Method};
use coder_host::client::{Connector, Device, Link};
use coder_link::{AttemptId, ConnectionId, Connector as _, HostKey, Report, Stage};
use coder_reach::hints::Locality;
use secp256k1::SecretKey;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Paired {
    link: Arc<Link>,
    secret: SecretKey,
    _owner: Owner,
}
struct Owner {
    connector: Mutex<Connector>,
    host: HostKey,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let connector = self
            .connector
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        connector.cancel(&self.host, AttemptId(1));
        connector.close(&self.host, ConnectionId(1));
    }
}
#[async_trait::async_trait]
impl HostLink for Paired {
    async fn revoke(&self) -> Result<(), HostError> {
        match self
            .link
            .call(coder_access::protocol::Operation::Revoke {
                device: self.link.device().key(),
            })
            .await?
        {
            coder_access::protocol::Outcome::Revoked { .. } => Ok(()),
            _ => Err(refused("Couldn't remove this pairing.")),
        }
    }

    async fn proof_headers(
        &self,
        intent: coder_access::http_proof::Intent,
        account: &str,
    ) -> Result<Vec<(String, String)>, HostError> {
        use coder_access::http_proof::{self, DEVICE_HEADER, HOST_HEADER};
        let answer = self
            .app_call(Request {
                method: Method::AuthorizeSync,
                params: serde_json::to_value(&intent)
                    .map_err(|_| refused("Invalid device request."))?,
            })
            .await?;
        if answer.method != Method::AuthorizeSync {
            return Err(refused("Invalid host proof."));
        }
        let host: nostr::domain::Event =
            serde_json::from_value(answer.value).map_err(|_| refused("Invalid host proof."))?;
        let host_header = http_proof::header(&host)?;
        let now = coder_access::unix_time()?;
        http_proof::verify(&host_header, &intent, now)?;
        if host.pubkey != self.link.device().host()
            || http_proof::tag(&host, "p")? != self.link.device().key()
            || http_proof::tag(&host, "account")? != account
        {
            return Err(refused("This host proof belongs to another device."));
        }
        let signed = http_proof::sign(
            &self.secret,
            &intent,
            now,
            vec![("account", account.into()), ("host", host.id.clone())],
        )?;
        Ok(vec![
            (HOST_HEADER.into(), host_header),
            (DEVICE_HEADER.into(), http_proof::header(&signed)?),
        ])
    }
    async fn app_call(&self, request: Request) -> Result<Answer, HostError> {
        self.link.app_call(request).await
    }
}

fn refused(message: &str) -> HostError {
    HostError::Access(coder_access::Error::new(Code::Forbidden, message))
}

/// Redeem an explicit connect code and verify the selected app computer.
/// Pairing is intentionally memory-only until a protected platform store is supplied.
pub async fn pair(
    code: &str,
    expected_device: &str,
    expected_host: &str,
) -> Result<DeviceChannel, HostError> {
    pair_with_policy(
        code,
        expected_device,
        expected_host,
        RelayPolicy::Production,
    )
    .await
}

pub async fn pair_with_policy(
    code: &str,
    expected_device: &str,
    expected_host: &str,
    policy: RelayPolicy,
) -> Result<DeviceChannel, HostError> {
    let invitation =
        coder_access::protocol::HostInvitation::parse(code, coder_access::unix_time()?, policy)?;
    if invitation.host() != expected_host {
        return Err(refused("This code belongs to another computer."));
    }
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let access = coder_access::client::redeem(code, &secret, policy).await?;
    let device = Arc::new(Device::new(access, secret, policy)?);
    // Enrollment commits before the next presence publication for this device.
    let reach = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            match coder_host::client::fetch_reach(&device, device.relay()).await {
                Ok(reach) => return Ok(reach),
                Err(HostError::Reach(error)) if error.code == coder_reach::Refusal::Stale => {
                    tokio::time::sleep(Duration::from_millis(250)).await
                }
                Err(error) => return Err(error),
            }
        }
    })
    .await
    .map_err(|_| refused("This computer hasn't published its device connection yet."))??;
    let capabilities = reach.presence.presence.capabilities;
    if !capabilities
        .iter()
        .any(|c| c == coder_access::app_rpc::CAPABILITY)
    {
        return Err(refused(
            "This computer doesn't support app requests over this connection.",
        ));
    }
    let (mut connector, mut reports) =
        Connector::new(tokio::runtime::Handle::current(), Locality::OtherMachine);
    let host = connector.add(device)?;
    connector.open(&host, AttemptId(1), Stage::Establishing);
    let owner = Owner {
        connector: Mutex::new(connector),
        host: host.clone(),
    };
    let established = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some((key, report)) = reports.recv().await {
            if key != host {
                continue;
            }
            match report {
                Report::Established(AttemptId(1)) => return true,
                Report::Failed(AttemptId(1), _) => return false,
                _ => {}
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    if !established {
        return Err(refused("Couldn't connect to this computer."));
    }
    let link = owner
        .connector
        .lock()
        .unwrap()
        .link(&host, ConnectionId(1))
        .ok_or_else(|| refused("The device connection closed."))?;
    let paired = Arc::new(Paired {
        link,
        secret,
        _owner: owner,
    });
    let identity = paired
        .app_call(Request {
            method: Method::EngineInfo,
            params: serde_json::json!({}),
        })
        .await?;
    if identity.method != Method::EngineInfo
        || identity.value["deviceId"].as_str() != Some(expected_device)
    {
        return Err(refused("This code belongs to another computer."));
    }
    Ok(DeviceChannel::new(paired, true, &capabilities))
}
