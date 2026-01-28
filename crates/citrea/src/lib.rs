mod address;
mod error;
mod keys;
mod rpc;
mod types;
mod util;

pub use address::{
    address_from_uncompressed_pubkey, create2_address, eoa_address_from_secret, format_address,
    keccak256,
};
pub use error::CitreaError;
pub use keys::{
    derive_agent_keypair, derive_keypair_full, sign_schnorr, verify_schnorr,
    xonly_pubkey_from_secret, SchnorrKeypair,
};
pub use rpc::{erc20_balance_of_data, erc20_transfer_data, RpcClient};
pub use types::{Address, BlockTag, Bytes32, Bytes64, RpcCallRequest};
pub use util::{
    format_hex_prefixed, parse_hex_bytes, parse_hex_u128, parse_hex_u64, parse_hex_vec,
    parse_u64_hex_or_dec, strip_0x,
};
