//! The settings `coder host init` records so `coder host serve`, and the host
//! service that starts it with no arguments, serves the same relays,
//! workspaces, and listeners every time.
//!
//! The file is `serve.json` in the host root, mode `0600`. Options given to
//! `coder host serve` replace the recorded value for that start only.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use coder_reach::hints::Class;
use serde::{Deserialize, Serialize};

use crate::config::{Advertised, WebsocketTls};
use crate::{Error, Result};

/// The file name in the host root.
pub const FILE: &str = "serve.json";
/// The settings schema. Fields added after the first release are optional,
/// so an earlier file still reads.
pub const SCHEMA: &str = "openagents.coder.host-serve-settings.v1";

/// What `coder host serve` uses when no option replaces it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServeSettings {
    pub schema: String,
    /// Relays to serve. The first is the primary relay.
    pub relays: Vec<String>,
    /// Workspace labels and their roots.
    pub workspaces: BTreeMap<String, PathBuf>,
    /// The WebSocket direct-channel listener, such as a tailnet address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_websocket: Option<SocketAddr>,
    /// Permit a listener on a LAN or tailnet address.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_nonloopback: bool,
    /// Extra endpoints to advertise, as `--advertise` takes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advertise: Vec<AdvertiseSetting>,
    /// TLS for the WebSocket listener.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub websocket_tls: Option<TlsSetting>,
    /// Tailnet admission: devices of this machine's own Tailscale user get
    /// invitations with these rights. Absent is off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailnet_admission: Option<TailnetSetting>,
}

/// The recorded `--tailnet-admission RIGHTS` and `--no-tailnet-chats`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailnetSetting {
    /// A rights list as `--rights` takes it, such as `standard`.
    pub rights: String,
    /// Hand out no chat invitations.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_chats: bool,
}

/// One recorded `--advertise CLASS=ADDRESS`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvertiseSetting {
    /// `lan`, `tailnet`, or `public`.
    pub class: String,
    /// `host:port`, or a `ws` or `wss` URL.
    pub address: String,
}

/// The recorded `--websocket-tls-cert`, `--websocket-tls-key`, and
/// `--websocket-name`. The files are read when the host starts, never here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsSetting {
    pub cert: PathBuf,
    pub key: PathBuf,
    pub name: String,
}

impl ServeSettings {
    /// Empty settings with the current schema.
    #[must_use]
    pub fn new(relays: Vec<String>, workspaces: BTreeMap<String, PathBuf>) -> Self {
        Self {
            schema: SCHEMA.into(),
            relays,
            workspaces,
            ..Self::default()
        }
    }

    /// Read `root/serve.json`. A missing file is empty settings.
    ///
    /// # Errors
    /// Reports an unreadable or malformed file, or another schema.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(FILE);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let settings: Self = serde_json::from_slice(&bytes)
                    .map_err(|_| Error::Config(format!("{} is malformed", path.display())))?;
                if settings.schema != SCHEMA {
                    return Err(Error::Config(format!(
                        "{} has an unsupported schema",
                        path.display()
                    )));
                }
                // Check every recorded value now, so a bad file refuses before
                // anything binds.
                settings.advertised()?;
                Ok(settings)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(Error::Config(format!("cannot read {}", path.display()))),
        }
    }

    /// Write `root/serve.json`, mode `0600`, replacing it atomically.
    ///
    /// # Errors
    /// Refuses a bad advertised endpoint and reports a failed write.
    pub fn save(&self, root: &Path) -> Result<()> {
        self.advertised()?;
        let mut settings = self.clone();
        settings.schema = SCHEMA.into();
        let bytes = serde_json::to_vec_pretty(&settings)
            .map_err(|_| Error::Config("settings cannot be encoded".into()))?;
        crate::serve::write_private(&root.join(FILE), &bytes)
    }

    /// The recorded endpoints as the host configuration takes them.
    ///
    /// # Errors
    /// Refuses an unknown class.
    pub fn advertised(&self) -> Result<Vec<Advertised>> {
        self.advertise
            .iter()
            .map(|setting| {
                Ok(Advertised {
                    class: parse_class(&setting.class)?,
                    address: setting.address.clone(),
                })
            })
            .collect()
    }

    /// The recorded TLS files as the host configuration takes them.
    #[must_use]
    pub fn tls(&self) -> Option<WebsocketTls> {
        self.websocket_tls.as_ref().map(|tls| WebsocketTls {
            cert: tls.cert.clone(),
            key: tls.key.clone(),
            name: tls.name.clone(),
        })
    }
}

/// `lan`, `tailnet`, or `public`.
///
/// # Errors
/// Refuses any other class; loopback and relay hints are never advertised.
pub fn parse_class(class: &str) -> Result<Class> {
    match class {
        "lan" => Ok(Class::Lan),
        "tailnet" => Ok(Class::Tailnet),
        "public" => Ok(Class::Public),
        _ => Err(Error::Config(
            "usage: --advertise class is lan, tailnet, or public".into(),
        )),
    }
}

/// Parse `CLASS=HOST:PORT` or `CLASS=URL`.
///
/// # Errors
/// Refuses a value without `=` or with an unknown class.
pub fn parse_advertise(entry: &str) -> Result<AdvertiseSetting> {
    let (class, address) = entry.split_once('=').ok_or_else(|| {
        Error::Config("usage: --advertise takes CLASS=HOST:PORT or CLASS=URL".into())
    })?;
    parse_class(class)?;
    Ok(AdvertiseSetting {
        class: class.to_owned(),
        address: address.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_earlier_file_without_listeners_still_reads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            format!(
                r#"{{"schema":"{SCHEMA}","relays":["wss://relay.example/"],"workspaces":{{}}}}"#
            ),
        )
        .unwrap();
        let settings = ServeSettings::load(dir.path()).unwrap();
        assert_eq!(settings.relays, ["wss://relay.example/"]);
        assert_eq!(settings.listen_websocket, None);
        assert!(settings.advertise.is_empty());
    }

    #[test]
    fn listeners_round_trip_and_the_file_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let mut settings = ServeSettings::new(
            vec!["wss://relay.example/".into()],
            BTreeMap::from([("checkout".into(), PathBuf::from("/work/checkout"))]),
        );
        settings.listen_websocket = Some("100.101.102.103:47101".parse().unwrap());
        settings.allow_nonloopback = true;
        settings.advertise =
            vec![parse_advertise("tailnet=wss://box.example.ts.net:47101/").unwrap()];
        settings.websocket_tls = Some(TlsSetting {
            cert: "/tls/chain.pem".into(),
            key: "/tls/key.pem".into(),
            name: "box.example.ts.net".into(),
        });
        settings.save(dir.path()).unwrap();
        assert_eq!(ServeSettings::load(dir.path()).unwrap(), settings);
        let advertised = settings.advertised().unwrap();
        assert_eq!(advertised[0].class, Class::Tailnet);
        assert_eq!(settings.tls().unwrap().name, "box.example.ts.net");
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn unknown_classes_and_fields_refuse() {
        assert!(parse_advertise("loopback=127.0.0.1:1").is_err());
        assert!(parse_advertise("tailnet").is_err());
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE),
            format!(r#"{{"schema":"{SCHEMA}","relays":[],"workspaces":{{}},"extra":1}}"#),
        )
        .unwrap();
        assert!(ServeSettings::load(dir.path()).is_err());
        std::fs::write(
            dir.path().join(FILE),
            format!(
                r#"{{"schema":"{SCHEMA}","relays":[],"workspaces":{{}},"advertise":[{{"class":"relay","address":"x"}}]}}"#
            ),
        )
        .unwrap();
        assert!(ServeSettings::load(dir.path()).is_err());
    }
}
