//! The resident node: one `wallet serve` process holds the node open and
//! answers other wallet commands over a Unix socket at `home/control.sock`.
//!
//! One JSON request per connection, one JSON response, each on a single
//! line. A `RemoteWallet` speaks that protocol and implements
//! `LightningWallet`, so a command that finds the socket acts through the
//! resident instead of opening the store itself; the store has one writer.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::LightningWallet;
use crate::model::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof, WalletError};

pub const SOCKET_FILE: &str = "control.sock";

/// How long a client waits for a reply that is not itself a bounded wait.
const REPLY_WAIT: Duration = Duration::from_secs(60);
/// Slack a client adds to a payment's own wait before giving up on the reply.
/// The resident's send may still settle after that; `lookup` tells.
const PAY_SLACK: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Status,
    NodeId,
    ReceiveExact {
        amount_msat: u64,
        request_hash: String,
        expiry_secs: u32,
    },
    Pay {
        invoice: String,
        max_fee_msat: u64,
        wait_secs: u64,
    },
    Lookup {
        payment_hash: String,
    },
    Balance,
    Channels,
    FundingAddress,
    OpenChannel {
        node_id: String,
        address: String,
        amount_sats: u64,
        announce: bool,
    },
    BuyChannel {
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    },
    ChannelOrder {
        order_id: String,
    },
    SendOnchain {
        address: String,
        amount_sats: u64,
    },
    CloseChannel {
        user_channel_id: String,
        counterparty: String,
        force: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    Ok(serde_json::Value),
    Err(WalletError),
}

/// What the resident says about itself.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resident {
    pub pid: u32,
    pub started_at: u64,
    pub uptime_secs: u64,
    pub node: serde_json::Value,
}

pub fn socket_path(home: &Path) -> PathBuf {
    home.join(SOCKET_FILE)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn io_error(context: &str, error: std::io::Error) -> WalletError {
    WalletError::Node(format!("resident {context}: {error}"))
}

/// What the resident needs from the node beyond `LightningWallet`.
pub trait Served: LightningWallet {
    fn status(&self) -> serde_json::Value;
    fn buy_channel(
        &self,
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    ) -> Result<serde_json::Value, WalletError>;
    fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError>;
    fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError>;
}

/// The serving side: owns the socket and answers until `stop` is set.
pub struct Server {
    listener: UnixListener,
    path: PathBuf,
    started_at: u64,
    stop: Arc<AtomicBool>,
}

impl Server {
    /// Bind `home/control.sock`. A leftover socket that nobody answers is
    /// removed; one that answers means another resident holds the store.
    pub fn bind(home: &Path) -> Result<Self, WalletError> {
        let path = socket_path(home);
        if path.exists() {
            if RemoteWallet::probe(home).is_some() {
                return Err(WalletError::Setup(format!(
                    "another wallet serve already answers at {}",
                    path.display()
                )));
            }
            std::fs::remove_file(&path).map_err(|error| io_error("remove stale socket", error))?;
        }
        std::fs::create_dir_all(home)
            .map_err(|error| WalletError::Setup(format!("{}: {error}", home.display())))?;
        let listener = UnixListener::bind(&path).map_err(|error| io_error("bind", error))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| io_error("bind", error))?;
        Ok(Self {
            listener,
            path,
            started_at: now(),
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    /// Accept connections until the stop flag is set, answering each on
    /// its own thread. Returns when stopped.
    pub fn run<W: Served + Send + Sync + 'static>(&self, wallet: Arc<W>) {
        while !self.stop.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let wallet = Arc::clone(&wallet);
                    let started_at = self.started_at;
                    std::thread::spawn(move || answer(stream, &*wallet, started_at));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(100)),
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn answer<W: Served>(stream: UnixStream, wallet: &W, started_at: u64) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return;
    }
    let response = match serde_json::from_str::<Request>(line.trim()) {
        Ok(request) => match handle(request, wallet, started_at) {
            Ok(value) => Response::Ok(value),
            Err(error) => Response::Err(error),
        },
        Err(error) => Response::Err(WalletError::Invalid(format!("request: {error}"))),
    };
    let mut text = serde_json::to_string(&response).unwrap_or_default();
    text.push('\n');
    let mut stream = &stream;
    let _ = stream.write_all(text.as_bytes());
}

fn handle<W: Served>(
    request: Request,
    wallet: &W,
    started_at: u64,
) -> Result<serde_json::Value, WalletError> {
    match request {
        Request::Ping => Ok(serde_json::json!({ "pid": std::process::id() })),
        Request::Status => value(Resident {
            pid: std::process::id(),
            started_at,
            uptime_secs: now().saturating_sub(started_at),
            node: wallet.status(),
        }),
        Request::NodeId => Ok(serde_json::Value::String(wallet.node_id())),
        Request::ReceiveExact {
            amount_msat,
            request_hash,
            expiry_secs,
        } => value(wallet.receive_exact(
            amount_msat,
            crate::parse_hash32(&request_hash)?,
            expiry_secs,
        )?),
        Request::Pay {
            invoice,
            max_fee_msat,
            wait_secs,
        } => value(wallet.pay(&invoice, max_fee_msat, Duration::from_secs(wait_secs))?),
        Request::Lookup { payment_hash } => {
            value(wallet.lookup(crate::parse_hash32(&payment_hash)?)?)
        }
        Request::Balance => value(wallet.balance()?),
        Request::Channels => value(wallet.channels()?),
        Request::FundingAddress => Ok(serde_json::Value::String(wallet.funding_address()?)),
        Request::OpenChannel {
            node_id,
            address,
            amount_sats,
            announce,
        } => Ok(serde_json::Value::String(wallet.open_channel(
            &node_id,
            &address,
            amount_sats,
            announce,
        )?)),
        Request::BuyChannel {
            lsp_balance_sat,
            client_balance_sat,
            channel_expiry_blocks,
            announce,
        } => wallet.buy_channel(
            lsp_balance_sat,
            client_balance_sat,
            channel_expiry_blocks,
            announce,
        ),
        Request::ChannelOrder { order_id } => wallet.channel_order(&order_id),
        Request::SendOnchain {
            address,
            amount_sats,
        } => Ok(serde_json::Value::String(
            wallet.send_onchain(&address, amount_sats)?,
        )),
        Request::CloseChannel {
            user_channel_id,
            counterparty,
            force,
        } => {
            wallet.close_channel(&user_channel_id, &counterparty, force)?;
            Ok(serde_json::Value::Null)
        }
    }
}

fn value<T: Serialize>(v: T) -> Result<serde_json::Value, WalletError> {
    serde_json::to_value(v).map_err(|error| WalletError::Node(error.to_string()))
}

/// A client of the resident node. Each call opens one connection.
pub struct RemoteWallet {
    path: PathBuf,
    node_id: String,
}

impl RemoteWallet {
    /// Connect to the resident under `home`, if one answers.
    pub fn probe(home: &Path) -> Option<Self> {
        let path = socket_path(home);
        let mut remote = Self {
            path,
            node_id: String::new(),
        };
        let value = remote.call(&Request::NodeId, Duration::from_secs(5)).ok()?;
        remote.node_id = value.as_str()?.to_owned();
        Some(remote)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn buy_channel(
        &self,
        lsp_balance_sat: u64,
        client_balance_sat: u64,
        channel_expiry_blocks: u32,
        announce: bool,
    ) -> Result<serde_json::Value, WalletError> {
        self.call(
            &Request::BuyChannel {
                lsp_balance_sat,
                client_balance_sat,
                channel_expiry_blocks,
                announce,
            },
            REPLY_WAIT,
        )
    }

    pub fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError> {
        self.call(
            &Request::ChannelOrder {
                order_id: order_id.to_owned(),
            },
            REPLY_WAIT,
        )
    }

    pub fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError> {
        let value = self.call(
            &Request::SendOnchain {
                address: address.to_owned(),
                amount_sats,
            },
            REPLY_WAIT,
        )?;
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| WalletError::Node("resident returned no txid".to_string()))
    }

    /// The resident's account of itself and its node.
    pub fn status(&self) -> Result<Resident, WalletError> {
        let value = self.call(&Request::Status, REPLY_WAIT)?;
        serde_json::from_value(value).map_err(|error| WalletError::Node(error.to_string()))
    }

    fn call(&self, request: &Request, wait: Duration) -> Result<serde_json::Value, WalletError> {
        let mut stream = UnixStream::connect(&self.path)
            .map_err(|error| io_error(&format!("connect {}", self.path.display()), error))?;
        stream
            .set_read_timeout(Some(wait))
            .map_err(|error| io_error("connect", error))?;
        let mut text =
            serde_json::to_string(request).map_err(|error| WalletError::Node(error.to_string()))?;
        text.push('\n');
        stream
            .write_all(text.as_bytes())
            .map_err(|error| io_error("send", error))?;
        let mut line = String::new();
        BufReader::new(&stream)
            .read_line(&mut line)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
                    WalletError::Pending {
                        payment_hash: "unknown".into(),
                        waited_secs: wait.as_secs(),
                    }
                }
                _ => io_error("reply", error),
            })?;
        if line.trim().is_empty() {
            return Err(WalletError::Node("resident closed without a reply".into()));
        }
        match serde_json::from_str::<Response>(line.trim())
            .map_err(|error| WalletError::Node(format!("resident reply: {error}")))?
        {
            Response::Ok(value) => Ok(value),
            Response::Err(error) => Err(error),
        }
    }

    fn typed<T: serde::de::DeserializeOwned>(
        &self,
        request: &Request,
        wait: Duration,
    ) -> Result<T, WalletError> {
        let value = self.call(request, wait)?;
        serde_json::from_value(value).map_err(|error| WalletError::Node(error.to_string()))
    }
}

impl LightningWallet for RemoteWallet {
    fn node_id(&self) -> String {
        self.node_id.clone()
    }

    fn receive_exact(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        self.typed(
            &Request::ReceiveExact {
                amount_msat,
                request_hash: hex::encode(request_hash),
                expiry_secs,
            },
            REPLY_WAIT,
        )
    }

    fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof, WalletError> {
        self.typed(
            &Request::Pay {
                invoice: invoice.to_owned(),
                max_fee_msat,
                wait_secs: wait.as_secs(),
            },
            wait + PAY_SLACK,
        )
    }

    fn lookup(&self, payment_hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        self.typed(
            &Request::Lookup {
                payment_hash: hex::encode(payment_hash),
            },
            REPLY_WAIT,
        )
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        self.typed(&Request::Balance, REPLY_WAIT)
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        self.typed(&Request::Channels, REPLY_WAIT)
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        self.typed(&Request::FundingAddress, REPLY_WAIT)
    }

    fn open_channel(
        &self,
        node_id: &str,
        address: &str,
        amount_sats: u64,
        announce: bool,
    ) -> Result<String, WalletError> {
        self.typed(
            &Request::OpenChannel {
                node_id: node_id.to_owned(),
                address: address.to_owned(),
                amount_sats,
                announce,
            },
            REPLY_WAIT,
        )
    }

    fn close_channel(
        &self,
        user_channel_id: &str,
        counterparty: &str,
        force: bool,
    ) -> Result<(), WalletError> {
        self.call(
            &Request::CloseChannel {
                user_channel_id: user_channel_id.to_owned(),
                counterparty: counterparty.to_owned(),
                force,
            },
            REPLY_WAIT,
        )
        .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;

    impl LightningWallet for Fake {
        fn node_id(&self) -> String {
            "02".repeat(33)
        }
        fn receive_exact(
            &self,
            amount_msat: u64,
            request_hash: [u8; 32],
            expiry_secs: u32,
        ) -> Result<IssuedInvoice, WalletError> {
            Ok(IssuedInvoice {
                bolt11: "lntb1fake".into(),
                payment_hash: "00".repeat(32),
                amount_msat,
                description_hash: hex::encode(request_hash),
                expiry_secs,
                pay_to: self.node_id(),
            })
        }
        fn pay(&self, invoice: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
            Err(WalletError::Failed {
                payment_hash: "11".repeat(32),
                reason: format!("no route for {invoice}"),
            })
        }
        fn lookup(&self, _: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
            Ok(None)
        }
        fn balance(&self) -> Result<Balance, WalletError> {
            Ok(Balance {
                onchain_total_sats: 1,
                onchain_spendable_sats: 2,
                lightning_total_sats: 3,
                anchor_reserve_sats: 4,
            })
        }
        fn channels(&self) -> Result<Vec<Channel>, WalletError> {
            Ok(Vec::new())
        }
        fn funding_address(&self) -> Result<String, WalletError> {
            Ok("tb1qfake".into())
        }
        fn open_channel(
            &self,
            node_id: &str,
            _: &str,
            _: u64,
            _: bool,
        ) -> Result<String, WalletError> {
            Ok(format!("channel-to-{node_id}"))
        }

        fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
            Ok(())
        }
    }

    impl Served for Fake {
        fn status(&self) -> serde_json::Value {
            serde_json::json!({ "running": true })
        }

        fn buy_channel(
            &self,
            lsp_balance_sat: u64,
            client_balance_sat: u64,
            channel_expiry_blocks: u32,
            announce: bool,
        ) -> Result<serde_json::Value, WalletError> {
            Ok(serde_json::json!({
                "order_id": "order-1",
                "lsp_balance_sat": lsp_balance_sat,
                "client_balance_sat": client_balance_sat,
                "channel_expiry_blocks": channel_expiry_blocks,
                "announce_channel": announce,
            }))
        }

        fn channel_order(&self, order_id: &str) -> Result<serde_json::Value, WalletError> {
            Ok(serde_json::json!({ "order_id": order_id, "channel": null }))
        }

        fn send_onchain(&self, address: &str, amount_sats: u64) -> Result<String, WalletError> {
            Ok(format!("txid:{address}:{amount_sats}"))
        }
    }

    fn temp_home(tag: &str) -> PathBuf {
        let home =
            std::env::temp_dir().join(format!("oa-wallet-resident-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        home
    }

    #[test]
    fn remote_wallet_round_trips_every_operation() {
        let home = temp_home("ops");
        let server = Server::bind(&home).unwrap();
        let stop = server.stop_flag();
        let path = server.path().to_path_buf();
        let thread = std::thread::spawn(move || server.run(Arc::new(Fake)));

        let remote = RemoteWallet::probe(&home).expect("resident answers");
        assert_eq!(remote.node_id(), "02".repeat(33));
        let issued = remote.receive_exact(1000, [7; 32], 60).unwrap();
        assert_eq!(issued.description_hash, "07".repeat(32));
        assert_eq!(remote.balance().unwrap().anchor_reserve_sats, 4);
        assert!(remote.channels().unwrap().is_empty());
        assert_eq!(remote.funding_address().unwrap(), "tb1qfake");
        assert_eq!(remote.lookup([1; 32]).unwrap(), None);
        assert_eq!(
            remote.open_channel("03ab", "h:1", 5, false).unwrap(),
            "channel-to-03ab"
        );
        let order = remote.buy_channel(100_000, 0, 13_000, false).unwrap();
        assert_eq!(order["order_id"], "order-1");
        assert_eq!(order["lsp_balance_sat"], 100_000);
        assert_eq!(
            remote.channel_order("order-1").unwrap()["order_id"],
            "order-1"
        );
        assert_eq!(remote.send_onchain("tb1qx", 7).unwrap(), "txid:tb1qx:7");
        let status = remote.status().unwrap();
        assert_eq!(status.pid, std::process::id());
        assert_eq!(status.node["running"], true);
        match remote.pay("lntb1x", 10, Duration::from_secs(1)) {
            Err(WalletError::Failed { reason, .. }) => assert_eq!(reason, "no route for lntb1x"),
            other => panic!("{other:?}"),
        }

        assert!(
            Server::bind(&home).is_err(),
            "a second resident must not bind over a live one"
        );

        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        assert!(!path.exists(), "socket removed on stop");
        assert!(RemoteWallet::probe(&home).is_none());
    }

    #[test]
    fn stale_socket_is_replaced() {
        let home = temp_home("stale");
        let path = socket_path(&home);
        std::fs::write(&path, b"").unwrap();
        let server = Server::bind(&home).unwrap();
        assert!(server.path().exists());
        drop(server);
        assert!(!path.exists());
    }
}
