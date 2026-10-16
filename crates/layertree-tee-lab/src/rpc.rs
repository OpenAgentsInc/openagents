//! Public research RPC values. No private key crosses this boundary.
use crate::{
    enclave::Config,
    rules::{Op, TreeParams, TreeState},
    tree::{Leaf, Tree, tagged_hash},
};
use bitcoin::hex::FromHex;
use bitcoin::{
    OutPoint, Transaction,
    consensus::encode::{deserialize_hex, serialize_hex},
    key::XOnlyPublicKey,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const HOSTNAME: &str = "layertree-signet.internal";
pub const MAX_BODY: usize = 1_048_576;

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn bytes(s: &str) -> Result<Vec<u8>, String> {
    Vec::<u8>::from_hex(s).map_err(|_| "invalid hex".into())
}
pub fn array<const N: usize>(s: &str) -> Result<[u8; N], String> {
    bytes(s)?.try_into().map_err(|_| "wrong hex length".into())
}
pub fn transaction(s: &str) -> Result<Transaction, String> {
    deserialize_hex(s).map_err(|_| "invalid transaction".into())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MirrorConfig {
    pub key: String,
    pub url: String,
}

/// Compiled into the measured image; runtime metadata cannot change it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceConfig {
    pub operator_key: String,
    pub tree_id: String,
    pub depth: u32,
    pub dust_min: u64,
    pub unroll_delay: u16,
    pub radix: usize,
    pub mirrors: Vec<MirrorConfig>,
    pub mirror_threshold: usize,
}
impl ServiceConfig {
    pub fn enclave(&self) -> Result<Config, String> {
        if self.depth == 0
            || self.depth > 12
            || self.dust_min < 330
            || self.unroll_delay == 0
            || !(2..=128).contains(&self.radix)
            || self.mirror_threshold < 2
            || self.mirror_threshold > self.mirrors.len()
            || self.mirrors.len() > 8
        {
            return Err("invalid research configuration".into());
        }
        let operator_key = array(&self.operator_key)?;
        XOnlyPublicKey::from_slice(&operator_key).map_err(|_| "invalid operator key")?;
        let mut keys = std::collections::HashSet::new();
        for mirror in &self.mirrors {
            let key = mirror
                .key
                .parse::<XOnlyPublicKey>()
                .map_err(|_| "invalid mirror key")?;
            if !keys.insert(key) {
                return Err("duplicate mirror key".into());
            }
        }
        Ok(Config {
            operator_key,
            tree_id: array(&self.tree_id)?,
            depth: self.depth,
            dust_min: self.dust_min,
            unroll_delay: self.unroll_delay,
            migration_m: None,
            radix: self.radix,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    pub enclave_key: String,
    pub operator_key: String,
    pub tree_id: String,
    pub depth: u32,
    pub dust_min: u64,
    pub unroll_delay: u16,
}
impl From<&TreeParams> for Parameters {
    fn from(p: &TreeParams) -> Self {
        Self {
            enclave_key: p.enclave_key.to_string(),
            operator_key: hex(&p.operator_key),
            tree_id: hex(&p.tree_id),
            depth: p.depth,
            dust_min: p.dust_min,
            unroll_delay: p.unroll_delay,
        }
    }
}
impl Parameters {
    pub fn params(&self) -> Result<TreeParams, String> {
        Ok(TreeParams {
            enclave_key: self
                .enclave_key
                .parse()
                .map_err(|_| "invalid enclave key")?,
            operator_key: array(&self.operator_key)?,
            tree_id: array(&self.tree_id)?,
            depth: self.depth,
            dust_min: self.dust_min,
            unroll_delay: self.unroll_delay,
            migration_m: None,
        })
    }
}

/// Canonical 116-byte leaves, encoded as hex, with explicit positions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    pub pos: u64,
    pub leaf: String,
}
impl Placed {
    pub fn decode(&self) -> Result<(u64, Leaf), String> {
        let leaf = Leaf::parse(&bytes(&self.leaf)?).map_err(|_| "invalid leaf")?;
        leaf.validate().map_err(|_| "invalid leaf")?;
        Ok((self.pos, leaf))
    }
}
pub fn tree(depth: u32, leaves: &[Placed]) -> Result<Tree, String> {
    if leaves.is_empty() || leaves.len() > 4096 {
        return Err("invalid member count".into());
    }
    let mut tree = Tree::new(depth).map_err(|_| "invalid depth")?;
    for item in leaves {
        let (pos, leaf) = item.decode()?;
        tree.insert(pos, leaf)
            .map_err(|_| "invalid leaf position")?;
    }
    Ok(tree)
}
pub fn leaves(state: &TreeState) -> Vec<Placed> {
    state
        .tree
        .leaves()
        .iter()
        .map(|(pos, leaf)| Placed {
            pos: *pos,
            leaf: hex(&leaf.serialize()),
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Join {
        height: u32,
        index: u64,
        leaves: Vec<Placed>,
    },
    Update {
        pos: u64,
        leaf: String,
    },
    Exit {
        pos: u64,
        evict: bool,
    },
    Refresh,
}
impl Operation {
    pub fn op(&self) -> Result<Op, String> {
        Ok(match self {
            Self::Join {
                height,
                index,
                leaves,
            } => Op::Join {
                height: *height,
                index: *index,
                leaves: leaves
                    .iter()
                    .map(Placed::decode)
                    .collect::<Result<_, _>>()?,
            },
            Self::Update { pos, leaf } => Op::Update {
                pos: *pos,
                new_leaf: Placed {
                    pos: *pos,
                    leaf: leaf.clone(),
                }
                .decode()?
                .1,
            },
            Self::Exit { pos, evict } => Op::Exit {
                pos: *pos,
                evict: *evict,
            },
            Self::Refresh => Op::Refresh,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authorized {
    pub timestamp: u64,
    pub nonce: String,
    pub signature: String,
    pub payload: Value,
}
pub fn authorization(path: &str, timestamp: u64, nonce: &str, payload: &Value) -> [u8; 32] {
    tagged_hash(
        "LayerTreeTEE/rpc/v1",
        &[
            path.as_bytes(),
            &timestamp.to_le_bytes(),
            nonce.as_bytes(),
            &serde_json::to_vec(payload).expect("JSON value"),
        ],
    )
}

/// Authenticate the exact route and JSON value, with a bounded replay window.
pub fn authenticate(
    path: &str,
    signed: &Authorized,
    operator: &XOnlyPublicKey,
    now: u64,
) -> Result<[u8; 32], String> {
    if now.abs_diff(signed.timestamp) > 120 || array::<32>(&signed.nonce).is_err() {
        return Err("request has expired or has an invalid nonce".into());
    }
    let digest = authorization(path, signed.timestamp, &signed.nonce, &signed.payload);
    let signature = signed
        .signature
        .parse()
        .map_err(|_| "invalid operator signature")?;
    bitcoin::secp256k1::Secp256k1::verification_only()
        .verify_schnorr(
            &signature,
            &bitcoin::secp256k1::Message::from_digest(digest),
            operator,
        )
        .map_err(|_| "operator signature refused")?;
    Ok(digest)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    pub transaction: String,
    pub leaves: Vec<Placed>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepare {
    pub transaction: String,
    pub operation: Operation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    pub transaction_id: String,
    pub signatures: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepared {
    pub transaction: String,
    pub record: String,
    pub unroll: Option<String>,
    pub sighash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    pub record: String,
    pub unroll: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub mirror: String,
    pub signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub params: Parameters,
    pub outpoint: Option<String>,
    pub leaves: Vec<Placed>,
    pub unroll_key: Option<String>,
    pub output: Option<String>,
    pub unroll: Option<String>,
    pub halted: bool,
    pub exit_requests: Vec<(u64, u32)>,
    pub height: Option<u32>,
    pub last_transaction: Option<String>,
    pub ephemeral_keys: usize,
}
pub fn snapshot(
    enclave: &crate::enclave::Enclave,
    unroll: Option<&crate::unroll::UnrollPackage>,
    height: Option<u32>,
    last: Option<&Transaction>,
) -> Snapshot {
    let state = enclave.state();
    Snapshot {
        params: enclave.params().into(),
        outpoint: state.map(|s| s.outpoint.to_string()),
        leaves: state.map(leaves).unwrap_or_default(),
        unroll_key: state.map(|s| s.output.unroll_key.to_string()),
        output: state.map(|s| {
            serialize_hex(&s.txout(
                &bitcoin::secp256k1::Secp256k1::verification_only(),
                enclave.params(),
            ))
        }),
        unroll: unroll.map(|p| hex(&p.encode())),
        halted: enclave.halted(),
        exit_requests: enclave.exit_requests().to_vec(),
        height,
        last_transaction: last.map(serialize_hex),
        ephemeral_keys: enclave.ephemeral_keys_held(),
    }
}

pub fn outpoint(s: &str) -> Result<OutPoint, String> {
    s.parse().map_err(|_| "invalid outpoint".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authorization_binds_route_clock_nonce_and_payload() {
        use bitcoin::secp256k1::{Keypair, Message, Secp256k1, SecretKey};
        let secp = Secp256k1::new();
        let key = Keypair::from_secret_key(&secp, &SecretKey::from_slice(&[42; 32]).unwrap());
        let mut input = Authorized {
            timestamp: 1000,
            nonce: hex(&[7; 32]),
            signature: String::new(),
            payload: serde_json::json!({"kind":"refresh"}),
        };
        input.signature = secp
            .sign_schnorr(
                &Message::from_digest(authorization(
                    "/prepare",
                    input.timestamp,
                    &input.nonce,
                    &input.payload,
                )),
                &key,
            )
            .to_string();
        let public = key.x_only_public_key().0;
        assert!(authenticate("/prepare", &input, &public, 1000).is_ok());
        assert!(authenticate("/finish", &input, &public, 1000).is_err());
        assert!(authenticate("/prepare", &input, &public, 1121).is_err());
        let mut other = input.clone();
        other.payload = serde_json::json!({"kind":"exit"});
        assert!(authenticate("/prepare", &other, &public, 1000).is_err());
        let mut other = input.clone();
        other.timestamp += 1;
        assert!(authenticate("/prepare", &other, &public, 1000).is_err());
        let mut other = input.clone();
        other.nonce = hex(&[8; 32]);
        assert!(authenticate("/prepare", &other, &public, 1000).is_err());
        assert!(array::<32>(&"é".repeat(32)).is_err());
    }
}
