use alloy_primitives::{Address, FixedBytes, U256};
use alloy_sol_types::{SolCall, SolEvent, SolValue, sol};

use crate::api::types::QuoteCalldata;
use crate::error::BoltzError;

// ─── Router contract (boltz-core v2, VERSION = 2) ────────────────────────

sol! {
    /// Cooperative ERC20 claim data. The `v/r/s` fields are the `ERC20Swap`
    /// cooperative claim EIP-712 signature (signed by the gas signer / claimAddress).
    struct Erc20Claim {
        bytes32 preimage;
        uint256 amount;
        address tokenAddress;
        address refundAddress;
        uint256 timelock;
        uint8 v;
        bytes32 r;
        bytes32 s;
    }

    /// A single call for the Router to execute (DEX swap calls).
    /// NOTE: Boltz encode API returns `{to, value, data}` but the Router contract
    /// uses `{target, value, callData}`. Use `Call::from_quote_calldata` to map.
    struct Call {
        address target;
        uint256 value;
        bytes callData;
    }

    /// Same-chain: claim tBTC + DEX swap + sweep output token to `destination`.
    /// The trailing v/r/s is the Router EIP-712 Claim signature.
    function claimERC20Execute(
        Erc20Claim calldata claim,
        Call[] calldata calls,
        address token,
        uint256 minAmountOut,
        address destination,
        uint8 v,
        bytes32 r,
        bytes32 s
    );

    /// OFT send parameters for cross-chain bridging via `LayerZero`.
    struct SendData {
        uint32 dstEid;
        bytes32 to;
        bytes extraOptions;
        bytes composeMsg;
        bytes oftCmd;
    }

    /// Authorization for cross-chain Router.claimERC20ExecuteOft.
    struct ClaimSendAuthorization {
        uint256 minAmountLd;
        uint256 lzTokenFee;
        address refundAddress;
        uint8 v;
        bytes32 r;
        bytes32 s;
    }

    /// Cross-chain: claim + DEX swap + OFT bridge to another chain.
    function claimERC20ExecuteOft(
        Erc20Claim calldata claim,
        Call[] calldata calls,
        address token,
        address oft,
        SendData calldata sendData,
        ClaimSendAuthorization calldata auth
    );

    /// CCTP burn parameters for cross-chain bridging via Circle CCTP v2.
    struct CctpData {
        uint32 destinationDomain;
        bytes32 mintRecipient;
        bytes32 destinationCaller;
        uint256 maxFee;
        uint32 minFinalityThreshold;
        bytes hookData;
    }

    /// Authorization for cross-chain Router.claimERC20ExecuteCctp.
    /// `minAmount` is the USDC floor that must remain for the CCTP burn after
    /// the DEX `calls` execute (the end-to-end delivered-amount floor).
    struct ClaimCctpAuthorization {
        uint256 minAmount;
        uint8 v;
        bytes32 r;
        bytes32 s;
    }

    /// Cross-chain: claim + DEX swap (tBTC -> USDC) + CCTP burn to another chain.
    function claimERC20ExecuteCctp(
        Erc20Claim calldata claim,
        Call[] calldata calls,
        address token,
        address tokenMessenger,
        CctpData calldata cctpData,
        ClaimCctpAuthorization calldata auth
    );

    // ─── ERC20Swap contract ──────────────────────────────────────────────

    /// Direct claim (non-Router fallback). Anyone can call; tokens go to claimAddress.
    function claim(
        bytes32 preimage,
        uint256 amount,
        address tokenAddress,
        address claimAddress,
        address refundAddress,
        uint256 timelock
    );

    /// `ERC20Swap` version — used for EIP-712 domain (currently returns 6).
    function version() external view returns (uint64);

    /// Hash all values of a swap to get the on-chain state key.
    function hashValues(
        bytes32 preimageHash,
        uint256 amount,
        address tokenAddress,
        address claimAddress,
        address refundAddress,
        uint256 timelock
    ) external pure returns (bytes32);

    /// Check whether a swap is still locked (true = funds present, false = already claimed/refunded).
    function swaps(bytes32 hash) external view returns (bool);

    // ─── ERC20 ───────────────────────────────────────────────────────────

    function transfer(address to, uint256 amount) returns (bool);
    function balanceOf(address account) returns (uint256);
    function approve(address spender, uint256 amount) returns (bool);
    function allowance(address owner, address spender) returns (uint256);

    // ─── Router read functions ───────────────────────────────────────────

    function TYPEHASH_SEND_DATA() external view returns (bytes32);
    function TYPEHASH_CCTP_DATA() external view returns (bytes32);

    // ─── OFT Contract (LayerZero USDT0) ──────────────────────────────

    struct OftSendParam {
        uint32 dstEid;
        bytes32 to;
        uint256 amountLD;
        uint256 minAmountLD;
        bytes extraOptions;
        bytes composeMsg;
        bytes oftCmd;
    }

    struct OftLimit {
        uint256 minAmountLD;
        uint256 maxAmountLD;
    }

    struct OftReceipt {
        uint256 amountSentLD;
        uint256 amountReceivedLD;
    }

    struct OftFeeDetail {
        int256 feeAmountLD;
        string description;
    }

    struct MessagingFee {
        uint256 nativeFee;
        uint256 lzTokenFee;
    }

    function quoteOFT(OftSendParam calldata sendParam)
        external view
        returns (OftLimit, OftFeeDetail[], OftReceipt);

    function quoteSend(OftSendParam calldata sendParam, bool payInLzToken)
        external view
        returns (MessagingFee);

    /// True when the OFT is an Adapter that debits via `transferFrom` and
    /// therefore requires an ERC20 allowance from `msg.sender`. Mint/burn
    /// variants (e.g. Arbitrum's native-mesh USDT0 OFT) return false.
    function approvalRequired() external view returns (bool);

    // ─── Events ──────────────────────────────────────────────────────────

    /// Standard ERC20 transfer event.
    event Transfer(address indexed from, address indexed to, uint256 value);

    /// `LayerZero` v2 OFT send event. Emitted on the source chain by both
    /// the native-mesh and legacy-mesh USDT0 OFT contracts on Arbitrum with
    /// the same signature. `amountReceivedLD` is the amount that will be
    /// credited on the destination chain (for USDT0 under LZ v2, equal to
    /// what arrives).
    event OFTSent(
        bytes32 indexed guid,
        uint32 dstEid,
        address indexed fromAddress,
        uint256 amountSentLD,
        uint256 amountReceivedLD
    );

    /// Circle CCTP v2 `MessageTransmitter` event. Emitted once per
    /// `depositForBurn`; `message` is the full CCTP message whose burn body
    /// carries the burned amount and (on the destination) the executed fee.
    event MessageSent(bytes message);

    /// Circle CCTP v2 `TokenMinter` event on the destination chain.
    /// `mintRecipient` and `mintToken` are indexed; `amount` and
    /// `feeCollected` are in data. The authoritative delivered amount.
    event MintAndWithdraw(
        address indexed mintRecipient,
        uint256 amount,
        address indexed mintToken,
        uint256 feeCollected
    );

    /// `ERC20Swap` claim. Emitted only on a successful claim — the contract
    /// verifies `sha256(preimage) == preimageHash` before emitting — so a log
    /// for our (indexed) `preimageHash` is positive proof the lockup was
    /// *claimed*, not refunded. `swaps(hash)` alone cannot tell the two apart.
    event Claim(bytes32 indexed preimageHash, bytes32 preimage);

    /// `ERC20Swap` refund. Emitted when a lockup is refunded (post-timeout).
    /// The counterpart to `Claim` for distinguishing a refunded lockup.
    event Refund(bytes32 indexed preimageHash);
}

/// Convert a Boltz encode API `QuoteCalldata` to a Router `Call`.
/// Maps: `to` -> `target`, `data` -> `callData`.
///
/// TRUST NOTE: The `calls` array is NOT covered by the Router's EIP-712 signature
/// (which only binds `preimage, token, minAmountOut, destination`). We trust Boltz
/// to provide honest DEX routing. The Router contract enforces `minAmountOut` as a
/// floor on the output, bounding any manipulation to the slippage tolerance.
pub fn quote_calldata_to_call(qc: &QuoteCalldata) -> Result<Call, BoltzError> {
    let target = parse_address(&qc.to)?;
    let value = parse_u256(&qc.value)?;
    let call_data = parse_hex_bytes(&qc.data)?;

    Ok(Call {
        target,
        value,
        callData: call_data.into(),
    })
}

/// Encode `claimERC20Execute` calldata for same-chain delivery.
#[expect(clippy::too_many_arguments)]
pub fn encode_claim_erc20_execute(
    claim: &Erc20Claim,
    calls: &[Call],
    token: Address,
    min_amount_out: U256,
    destination: Address,
    router_sig_v: u8,
    router_sig_r: [u8; 32],
    router_sig_s: [u8; 32],
) -> Vec<u8> {
    let call = claimERC20ExecuteCall {
        claim: claim.clone(),
        calls: calls.to_vec(),
        token,
        minAmountOut: min_amount_out,
        destination,
        v: router_sig_v,
        r: router_sig_r.into(),
        s: router_sig_s.into(),
    };
    call.abi_encode()
}

/// Encode `claimERC20ExecuteOft` calldata for cross-chain delivery.
pub fn encode_claim_erc20_execute_oft(
    claim: &Erc20Claim,
    calls: &[Call],
    token: Address,
    oft: Address,
    send_data: &SendData,
    auth: &ClaimSendAuthorization,
) -> Vec<u8> {
    let call = claimERC20ExecuteOftCall {
        claim: claim.clone(),
        calls: calls.to_vec(),
        token,
        oft,
        sendData: send_data.clone(),
        auth: auth.clone(),
    };
    call.abi_encode()
}

/// Encode `claimERC20ExecuteCctp` calldata for cross-chain USDC delivery via
/// Circle CCTP v2.
pub fn encode_claim_erc20_execute_cctp(
    claim: &Erc20Claim,
    calls: &[Call],
    token: Address,
    token_messenger: Address,
    cctp_data: &CctpData,
    auth: &ClaimCctpAuthorization,
) -> Vec<u8> {
    let call = claimERC20ExecuteCctpCall {
        claim: claim.clone(),
        calls: calls.to_vec(),
        token,
        tokenMessenger: token_messenger,
        cctpData: cctp_data.clone(),
        auth: auth.clone(),
    };
    call.abi_encode()
}

/// Compute the EIP-712 struct hash for `CctpData`.
///
/// `hash = keccak256(abi.encode(TYPEHASH, destinationDomain, mintRecipient,
/// destinationCaller, maxFee, minFinalityThreshold, keccak256(hookData)))`.
/// `hookData` is hashed (it is `bytes` in the struct but `bytes32` in the
/// typehash); the `uint32` fields are encoded as full 32-byte words.
pub fn hash_cctp_data(typehash: [u8; 32], cctp_data: &CctpData) -> [u8; 32] {
    use alloy_primitives::keccak256;

    let hook_data_hash = keccak256(cctp_data.hookData.as_ref());

    let encoded = (
        FixedBytes::<32>::from(typehash),
        U256::from(cctp_data.destinationDomain),
        cctp_data.mintRecipient,
        cctp_data.destinationCaller,
        cctp_data.maxFee,
        U256::from(cctp_data.minFinalityThreshold),
        hook_data_hash,
    )
        .abi_encode();

    keccak256(&encoded).into()
}

/// Encode `version()` calldata for reading `ERC20Swap` version.
pub fn encode_version_call() -> Vec<u8> {
    versionCall {}.abi_encode()
}

/// Decode `version()` return value.
pub fn decode_version_return(data: &[u8]) -> Result<u64, BoltzError> {
    let decoded = <(u64,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode version return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0)
}

/// Encode `balanceOf(address)` calldata.
pub fn encode_balance_of(account: Address) -> Vec<u8> {
    balanceOfCall { account }.abi_encode()
}

/// Decode `balanceOf` return value.
pub fn decode_balance_of(data: &[u8]) -> Result<U256, BoltzError> {
    let decoded = <(U256,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode balanceOf return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0)
}

/// Encode `approve(spender, amount)` calldata.
pub fn encode_approve(spender: Address, amount: U256) -> Vec<u8> {
    approveCall { spender, amount }.abi_encode()
}

/// Encode `allowance(owner, spender)` calldata.
pub fn encode_allowance(owner: Address, spender: Address) -> Vec<u8> {
    allowanceCall { owner, spender }.abi_encode()
}

/// Decode `allowance` return value.
pub fn decode_allowance_return(data: &[u8]) -> Result<U256, BoltzError> {
    let decoded = <(U256,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode allowance return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0)
}

/// Encode `approvalRequired()` calldata.
pub fn encode_approval_required() -> Vec<u8> {
    approvalRequiredCall {}.abi_encode()
}

/// Decode `approvalRequired()` return value.
pub fn decode_approval_required_return(data: &[u8]) -> Result<bool, BoltzError> {
    let decoded = <(bool,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode approvalRequired return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0)
}

/// Encode the direct `claim()` calldata (non-Router fallback).
pub fn encode_direct_claim(
    preimage: [u8; 32],
    amount: U256,
    token_address: Address,
    claim_address: Address,
    refund_address: Address,
    timelock: U256,
) -> Vec<u8> {
    let call = claimCall {
        preimage: preimage.into(),
        amount,
        tokenAddress: token_address,
        claimAddress: claim_address,
        refundAddress: refund_address,
        timelock,
    };
    call.abi_encode()
}

/// Encode `TYPEHASH_SEND_DATA()` calldata.
pub fn encode_typehash_send_data_call() -> Vec<u8> {
    TYPEHASH_SEND_DATACall {}.abi_encode()
}

/// Encode `TYPEHASH_CCTP_DATA()` calldata.
pub fn encode_typehash_cctp_data_call() -> Vec<u8> {
    TYPEHASH_CCTP_DATACall {}.abi_encode()
}

/// Decode `TYPEHASH_CCTP_DATA()` return value.
pub fn decode_typehash_cctp_data(data: &[u8]) -> Result<[u8; 32], BoltzError> {
    let decoded = <(FixedBytes<32>,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode TYPEHASH_CCTP_DATA return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0.into())
}

/// Decode `TYPEHASH_SEND_DATA()` return value.
pub fn decode_typehash_send_data(data: &[u8]) -> Result<[u8; 32], BoltzError> {
    let decoded = <(FixedBytes<32>,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode TYPEHASH_SEND_DATA return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0.into())
}

// ─── OFT helpers ─────────────────────────────────────────────────────────

/// Build an `OftSendParam` for quoting.
///
/// `to` is the transport-encoded 32-byte recipient. Callers compute it via
/// `crate::evm::recipient::encode_oft_recipient` so the same encoding feeds
/// both `OftSendParam.to` and `SendData.to`.
///
/// `extra_options` is the `LayerZero` v2 executor options blob. Empty for
/// plain EVM / Tron destinations; populated via `crate::evm::lz_options` when
/// the destination is Solana and the recipient's `Associated Token Account`
/// needs pre-funded creation. The same bytes must be used for every
/// `quoteOFT` / `quoteSend` / `SendData` call on a swap — the router signs
/// over them, so any divergence from what is submitted on-chain will fail
/// signature verification.
///
/// `composeMsg` and `oftCmd` are always empty — no compose messages in scope.
pub fn build_oft_send_param(
    dst_eid: u32,
    to: FixedBytes<32>,
    amount_ld: U256,
    min_amount_ld: U256,
    extra_options: alloy_primitives::Bytes,
) -> OftSendParam {
    OftSendParam {
        dstEid: dst_eid,
        to,
        amountLD: amount_ld,
        minAmountLD: min_amount_ld,
        extraOptions: extra_options,
        composeMsg: vec![].into(),
        oftCmd: vec![].into(),
    }
}

/// Left-pad a 20-byte EVM address to 32 bytes (`bytes32`), as required by OFT `to` field.
pub fn address_to_bytes32(addr: Address) -> FixedBytes<32> {
    let mut bytes = [0u8; 32];
    bytes[12..32].copy_from_slice(addr.as_slice());
    FixedBytes::from(bytes)
}

/// Encode `quoteOFT(OftSendParam)` calldata.
pub fn encode_quote_oft(send_param: &OftSendParam) -> Vec<u8> {
    quoteOFTCall {
        sendParam: send_param.clone(),
    }
    .abi_encode()
}

/// Decode `quoteOFT` return value: `(OftLimit, OftFeeDetail[], OftReceipt)`.
pub fn decode_quote_oft_return(
    data: &[u8],
) -> Result<(OftLimit, Vec<OftFeeDetail>, OftReceipt), BoltzError> {
    let decoded = quoteOFTCall::abi_decode_returns(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode quoteOFT return: {e}"),
        tx_hash: None,
    })?;
    Ok((decoded._0, decoded._1, decoded._2))
}

/// Encode `quoteSend(OftSendParam, bool)` calldata.
pub fn encode_quote_send(send_param: &OftSendParam, pay_in_lz_token: bool) -> Vec<u8> {
    quoteSendCall {
        sendParam: send_param.clone(),
        payInLzToken: pay_in_lz_token,
    }
    .abi_encode()
}

/// Decode `quoteSend` return value: `MessagingFee`.
pub fn decode_quote_send_return(data: &[u8]) -> Result<MessagingFee, BoltzError> {
    let decoded = quoteSendCall::abi_decode_returns(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode quoteSend return: {e}"),
        tx_hash: None,
    })?;
    Ok(MessagingFee {
        nativeFee: decoded.nativeFee,
        lzTokenFee: decoded.lzTokenFee,
    })
}

/// Compute the EIP-712 struct hash for `SendData`.
///
/// `hash = keccak256(abi.encode(TYPEHASH, dstEid, to, keccak256(extraOptions), keccak256(composeMsg), keccak256(oftCmd)))`
pub fn hash_send_data(typehash: [u8; 32], send_data: &SendData) -> [u8; 32] {
    use alloy_primitives::keccak256;

    let extra_options_hash = keccak256(send_data.extraOptions.as_ref());
    let compose_msg_hash = keccak256(send_data.composeMsg.as_ref());
    let oft_cmd_hash = keccak256(send_data.oftCmd.as_ref());

    let encoded = (
        FixedBytes::<32>::from(typehash),
        U256::from(send_data.dstEid),
        send_data.to,
        extra_options_hash,
        compose_msg_hash,
        oft_cmd_hash,
    )
        .abi_encode();

    keccak256(&encoded).into()
}

// ─── ERC20Swap lockup state ──────────────────────────────────────────────

/// The fields that identify an `ERC20Swap` lockup, used to recompute its
/// on-chain hash and check whether it is still locked.
#[derive(Debug, Clone)]
pub struct DecodedLockupEvent {
    pub preimage_hash: [u8; 32],
    pub amount: U256,
    pub token_address: Address,
    pub claim_address: Address,
    pub refund_address: Address,
    pub timelock: U256,
}

/// Encode `hashValues(...)` calldata.
pub fn encode_hash_values(
    preimage_hash: [u8; 32],
    amount: U256,
    token_address: Address,
    claim_address: Address,
    refund_address: Address,
    timelock: U256,
) -> Vec<u8> {
    hashValuesCall {
        preimageHash: preimage_hash.into(),
        amount,
        tokenAddress: token_address,
        claimAddress: claim_address,
        refundAddress: refund_address,
        timelock,
    }
    .abi_encode()
}

/// Decode `hashValues` return value (bytes32).
pub fn decode_hash_values_return(data: &[u8]) -> Result<[u8; 32], BoltzError> {
    let decoded = <(FixedBytes<32>,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode hashValues return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0.into())
}

/// Encode `swaps(bytes32)` calldata.
pub fn encode_swaps_check(hash: [u8; 32]) -> Vec<u8> {
    swapsCall { hash: hash.into() }.abi_encode()
}

/// Decode `swaps(bytes32)` return value (bool).
pub fn decode_swaps_check_return(data: &[u8]) -> Result<bool, BoltzError> {
    let decoded = <(bool,)>::abi_decode(data).map_err(|e| BoltzError::Evm {
        reason: format!("Failed to decode swaps return: {e}"),
        tx_hash: None,
    })?;
    Ok(decoded.0)
}

/// `eth_getLogs` topic0 for the `ERC20Swap` `Claim` event.
pub fn claim_event_topic0() -> String {
    format!("0x{}", hex::encode(Claim::SIGNATURE_HASH.as_slice()))
}

/// `eth_getLogs` topic0 for the `ERC20Swap` `Refund` event.
pub fn refund_event_topic0() -> String {
    format!("0x{}", hex::encode(Refund::SIGNATURE_HASH.as_slice()))
}

/// Format a 32-byte value as an indexed-`bytes32` event topic (e.g. a swap's
/// `preimageHash`, indexed by `Claim`/`Refund`).
pub fn bytes32_to_topic(value: &[u8; 32]) -> String {
    format!("0x{}", hex::encode(value))
}

/// Left-pad a 20-byte address to 32 bytes for use as an indexed event topic filter.
pub fn address_to_topic(address: &[u8; 20]) -> String {
    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(address);
    format!("0x{}", hex::encode(padded))
}

// ─── Helpers ─────────────────────────────────────────────────────────────

/// Parse a hex address string (with or without 0x prefix) into an `Address`.
pub fn parse_address(hex_str: &str) -> Result<Address, BoltzError> {
    let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    let bytes = hex::decode(clean).map_err(|e| BoltzError::Evm {
        reason: format!("Invalid address hex '{hex_str}': {e}"),
        tx_hash: None,
    })?;
    if bytes.len() != 20 {
        return Err(BoltzError::Evm {
            reason: format!("Address must be 20 bytes, got {}", bytes.len()),
            tx_hash: None,
        });
    }
    Ok(Address::from_slice(&bytes))
}

/// Parse a decimal or hex string into a `U256`.
pub fn parse_u256(s: &str) -> Result<U256, BoltzError> {
    if let Some(hex_str) = s.strip_prefix("0x") {
        U256::from_str_radix(hex_str, 16)
    } else {
        U256::from_str_radix(s, 10)
    }
    .map_err(|e| BoltzError::Evm {
        reason: format!("Invalid U256 value '{s}': {e}"),
        tx_hash: None,
    })
}

/// Parse hex-encoded bytes (with or without 0x prefix).
pub fn parse_hex_bytes(hex_str: &str) -> Result<Vec<u8>, BoltzError> {
    let clean = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    hex::decode(clean).map_err(|e| BoltzError::Evm {
        reason: format!("Invalid hex bytes '{hex_str}': {e}"),
        tx_hash: None,
    })
}

// ─── Delivered-amount decoding ──────────────────────────────────────────

/// How to interpret the claim tx receipt when extracting the delivered amount.
#[derive(Debug, Clone)]
pub enum DeliveredAmountSource {
    /// Same-chain (Arbitrum) delivery: find a `Transfer(_, user, value)` log
    /// on the USDT token. `user` is the final recipient EVM address;
    /// `token` is the USDT token contract address.
    ArbitrumTransfer { token: Address, user: Address },
    /// Bridged delivery via `LayerZero` OFT: find an `OFTSent` log emitted
    /// by the mesh-appropriate source OFT contract and read
    /// `amountReceivedLD`.
    OftSent { oft_contract: Address },
    /// Bridged delivery via Circle CCTP: find a `MessageSent` log emitted by
    /// the `MessageTransmitter` and read the burned amount (less the executed
    /// fee) from the burn-message body. This is a source-side estimate; the
    /// authoritative amount is the destination `MintAndWithdraw`.
    Cctp { message_transmitter: Address },
}

/// Result of decoding the delivered amount from a claim receipt's logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredAmount {
    /// Amount delivered on the destination chain (token base units).
    pub amount: u64,
    /// `LayerZero` message GUID as `0x`-prefixed hex, present only for
    /// bridged OFT swaps (`OFTSent` path).
    pub lz_guid: Option<String>,
    /// Circle CCTP source domain, present only for CCTP swaps. The caller
    /// synthesizes the guid `"<domain>:<source_tx_hash>"` (it needs the tx
    /// hash, which is not available to the log decoder).
    pub cctp_source_domain: Option<u32>,
}

/// Decode the delivered amount from a successful claim transaction's logs.
///
/// Returns `None` when no matching log is found; callers should treat this as
/// unknown rather than failure (logs at warn level and leaves the field
/// unset).
pub fn decode_delivered_from_logs(
    logs: &[crate::evm::provider::LogEntry],
    source: &DeliveredAmountSource,
) -> Option<DeliveredAmount> {
    match source {
        DeliveredAmountSource::ArbitrumTransfer { token, user } => {
            decode_arbitrum_transfer(logs, *token, *user)
        }
        DeliveredAmountSource::OftSent { oft_contract } => decode_oft_sent(logs, *oft_contract),
        DeliveredAmountSource::Cctp {
            message_transmitter,
        } => decode_cctp_sent(logs, *message_transmitter),
    }
}

// CCTP v2 message byte offsets (Circle MessageV2.sol + BurnMessageV2.sol).
// Outer header: sourceDomain (uint32) at byte 4; messageBody starts at 148.
// Burn body: amount (uint256) at body+68, feeExecuted (uint256) at body+164.
const CCTP_SOURCE_DOMAIN_OFFSET: usize = 4;
/// Offset of the 32-byte `nonce` in the CCTP v2 message header.
const CCTP_NONCE_OFFSET: usize = 12;
const CCTP_BODY_OFFSET: usize = 148;
const CCTP_BURN_AMOUNT_OFFSET: usize = CCTP_BODY_OFFSET + 68; // 216
const CCTP_BURN_FEE_OFFSET: usize = CCTP_BODY_OFFSET + 164; // 312

fn decode_cctp_sent(
    logs: &[crate::evm::provider::LogEntry],
    message_transmitter: Address,
) -> Option<DeliveredAmount> {
    let topic0 = format!("0x{}", hex::encode(MessageSent::SIGNATURE_HASH.as_slice()));

    for log in logs {
        if !addresses_equal(&log.address, &message_transmitter) {
            continue;
        }
        if log.topics.first().is_none_or(|t| !topics_equal(t, &topic0)) {
            continue;
        }

        // `MessageSent(bytes)` is non-indexed: data is abi.encode(bytes), i.e.
        // the raw CCTP message with an offset+length prefix.
        //
        // A matching-but-undecodable log is SKIPPED (not aborted on): the
        // MessageTransmitter emits `MessageSent` for any message type, so a
        // non-burn message (whose body is shorter than the burn offsets) may
        // precede the real burn in the same receipt. `continue` keeps scanning
        // so a valid later log is still found; `None` means "no matching log".
        let Ok(data_bytes) = parse_hex_bytes(&log.data) else {
            continue;
        };
        let Ok(message) = <alloy_primitives::Bytes>::abi_decode(&data_bytes) else {
            continue;
        };
        let msg = message.as_ref();

        let Some(source_domain) = read_u32_be(msg, CCTP_SOURCE_DOMAIN_OFFSET) else {
            continue;
        };
        let Some(amount) = read_u256_be(msg, CCTP_BURN_AMOUNT_OFFSET) else {
            continue;
        };
        let Some(fee_executed) = read_u256_be(msg, CCTP_BURN_FEE_OFFSET) else {
            continue;
        };
        // feeExecuted is 0 at the source (the fee is taken on the destination),
        // but guard anyway. Delivered estimate = burned amount - executed fee.
        let delivered = amount.saturating_sub(fee_executed);
        let Ok(amount_u64) = u64::try_from(delivered) else {
            continue;
        };

        return Some(DeliveredAmount {
            amount: amount_u64,
            lz_guid: None,
            cctp_source_domain: Some(source_domain),
        });
    }
    None
}

/// Parse the **authoritative** delivered amount from an attested CCTP message
/// (the `message` hex returned by Circle's Iris `/v2/messages`, NOT a log).
///
/// Unlike the source-chain `MessageSent` log — where `feeExecuted` is 0 — the
/// attested message has the finalized `feeExecuted` filled in by Circle, so
/// `delivered = burnAmount - feeExecuted` is exactly what gets minted on the
/// destination. This is what lets the client report the real delivered amount
/// without a destination-chain RPC. Returns `None` on a malformed/short
/// message or if the fee exceeds the amount.
pub fn decode_cctp_delivered_from_message(message_hex: &str) -> Option<u64> {
    let msg = parse_hex_bytes(message_hex).ok()?;
    let amount = read_u256_be(&msg, CCTP_BURN_AMOUNT_OFFSET)?;
    let fee = read_u256_be(&msg, CCTP_BURN_FEE_OFFSET)?;
    if fee > amount {
        return None;
    }
    amount.saturating_sub(fee).try_into().ok()
}

/// Extract the 32-byte CCTP v2 message `nonce` (header offset 12) from the
/// attested `message` hex. Used to derive the destination `used_nonce` PDA that
/// proves the mint landed, so completion needs no separate Iris `eventNonce`
/// field (same bytes, provably consistent with the attestation).
pub fn decode_cctp_nonce_from_message(message_hex: &str) -> Option<[u8; 32]> {
    let msg = parse_hex_bytes(message_hex).ok()?;
    let end = CCTP_NONCE_OFFSET.checked_add(32)?;
    msg.get(CCTP_NONCE_OFFSET..end)?.try_into().ok()
}

/// Read a big-endian `uint32` at `offset` from a raw byte message.
fn read_u32_be(msg: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = msg.get(offset..end)?;
    Some(u32::from_be_bytes(slice.try_into().ok()?))
}

/// Read a big-endian `uint256` at `offset` from a raw byte message.
fn read_u256_be(msg: &[u8], offset: usize) -> Option<U256> {
    let end = offset.checked_add(32)?;
    let slice = msg.get(offset..end)?;
    Some(U256::from_be_slice(slice))
}

fn decode_arbitrum_transfer(
    logs: &[crate::evm::provider::LogEntry],
    token: Address,
    user: Address,
) -> Option<DeliveredAmount> {
    let topic0 = format!("0x{}", hex::encode(Transfer::SIGNATURE_HASH.as_slice()));
    let user_topic = address_to_topic(&user.into_array());

    for log in logs {
        if !addresses_equal(&log.address, &token) {
            continue;
        }
        if log.topics.len() < 3 {
            continue;
        }
        if !topics_equal(&log.topics[0], &topic0) {
            continue;
        }
        if !topics_equal(&log.topics[2], &user_topic) {
            continue;
        }
        // Skip a matching-but-undecodable log rather than aborting the scan.
        let Ok(data_bytes) = parse_hex_bytes(&log.data) else {
            continue;
        };
        let Ok(value) = <U256>::abi_decode(&data_bytes) else {
            continue;
        };
        let Ok(amount) = u64::try_from(value) else {
            continue;
        };
        return Some(DeliveredAmount {
            amount,
            lz_guid: None,
            cctp_source_domain: None,
        });
    }
    None
}

fn decode_oft_sent(
    logs: &[crate::evm::provider::LogEntry],
    oft_contract: Address,
) -> Option<DeliveredAmount> {
    let topic0 = format!("0x{}", hex::encode(OFTSent::SIGNATURE_HASH.as_slice()));

    for log in logs {
        if !addresses_equal(&log.address, &oft_contract) {
            continue;
        }
        if log.topics.len() < 3 {
            continue;
        }
        if !topics_equal(&log.topics[0], &topic0) {
            continue;
        }
        // Skip a matching-but-undecodable log rather than aborting the scan.
        let Ok(data_bytes) = parse_hex_bytes(&log.data) else {
            continue;
        };
        // data = abi.encode(uint32 dstEid, uint256 amountSentLD, uint256 amountReceivedLD)
        let Ok((_dst_eid, _amount_sent, amount_received)) =
            <(u32, U256, U256)>::abi_decode(&data_bytes)
        else {
            continue;
        };
        let Ok(amount) = u64::try_from(amount_received) else {
            continue;
        };
        let guid_hex = log.topics[1]
            .strip_prefix("0x")
            .unwrap_or(&log.topics[1])
            .to_lowercase();
        return Some(DeliveredAmount {
            amount,
            lz_guid: Some(format!("0x{guid_hex}")),
            cctp_source_domain: None,
        });
    }
    None
}

fn addresses_equal(hex_addr: &str, addr: &Address) -> bool {
    let Ok(parsed) = parse_address(hex_addr) else {
        return false;
    };
    parsed == *addr
}

fn topics_equal(a: &str, b: &str) -> bool {
    let a_clean = a.strip_prefix("0x").unwrap_or(a);
    let b_clean = b.strip_prefix("0x").unwrap_or(b);
    a_clean.eq_ignore_ascii_case(b_clean)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use alloy_sol_types::SolCall;

    #[macros::test_all]
    fn claim_refund_event_topics_match_signatures() {
        use alloy_primitives::keccak256;
        // Pins the event declarations to Boltz's ERC20Swap ABI: a typo in the
        // event signature would silently make every Claim/Refund log query miss.
        assert_eq!(
            claim_event_topic0(),
            format!("0x{}", hex::encode(keccak256(b"Claim(bytes32,bytes32)")))
        );
        assert_eq!(
            refund_event_topic0(),
            format!("0x{}", hex::encode(keccak256(b"Refund(bytes32)")))
        );
    }

    #[macros::test_all]
    fn test_parse_address() {
        let addr = parse_address("0x1234567890AbCdEf1234567890aBcDeF12345678").unwrap();
        let expected_bytes = hex::decode("1234567890AbCdEf1234567890aBcDeF12345678").unwrap();
        assert_eq!(addr.as_slice(), &expected_bytes);
    }

    #[macros::test_all]
    fn test_parse_address_no_prefix() {
        let addr = parse_address("1234567890AbCdEf1234567890aBcDeF12345678").unwrap();
        let expected_bytes = hex::decode("1234567890AbCdEf1234567890aBcDeF12345678").unwrap();
        assert_eq!(addr.as_slice(), &expected_bytes);
    }

    #[macros::test_all]
    fn test_parse_address_invalid_length() {
        let result = parse_address("0x1234");
        assert!(result.is_err());
    }

    #[macros::test_all]
    fn test_parse_u256_decimal() {
        let val = parse_u256("1000000000000000000").unwrap();
        assert_eq!(val, U256::from(1_000_000_000_000_000_000u64));
    }

    #[macros::test_all]
    fn test_parse_u256_hex() {
        let val = parse_u256("0xde0b6b3a7640000").unwrap();
        assert_eq!(val, U256::from(1_000_000_000_000_000_000u64));
    }

    #[macros::test_all]
    fn test_parse_u256_zero() {
        let val = parse_u256("0").unwrap();
        assert_eq!(val, U256::ZERO);
    }

    #[macros::test_all]
    fn test_quote_calldata_to_call() {
        let qc = QuoteCalldata {
            to: "0x0000000000000000000000000000000000000042".to_string(),
            value: "0".to_string(),
            data: "0xabcdef".to_string(),
        };
        let call = quote_calldata_to_call(&qc).unwrap();
        assert_eq!(
            call.target,
            parse_address("0x0000000000000000000000000000000000000042").unwrap()
        );
        assert_eq!(call.value, U256::ZERO);
        assert_eq!(call.callData.as_ref(), &[0xab, 0xcd, 0xef]);
    }

    #[macros::test_all]
    fn test_version_call_selector() {
        let encoded = encode_version_call();
        // function selector for `version()` = keccak256("version()")[..4]
        // = 0x54fd4d50
        assert_eq!(&encoded[..4], &[0x54, 0xfd, 0x4d, 0x50]);
    }

    #[macros::test_all]
    fn test_decode_version() {
        // ABI-encode uint64 value 6
        let encoded = U256::from(6).abi_encode();
        let version = decode_version_return(&encoded).unwrap();
        assert_eq!(version, 6);
    }

    #[macros::test_all]
    fn test_balance_of_call_selector() {
        let addr = parse_address("0x0000000000000000000000000000000000000001").unwrap();
        let encoded = encode_balance_of(addr);
        // function selector for `balanceOf(address)` = 0x70a08231
        assert_eq!(&encoded[..4], &[0x70, 0xa0, 0x82, 0x31]);
    }

    #[macros::test_all]
    fn test_decode_balance_of() {
        let encoded = U256::from(1_000_000u64).abi_encode();
        let balance = decode_balance_of(&encoded).unwrap();
        assert_eq!(balance, U256::from(1_000_000u64));
    }

    #[macros::test_all]
    fn test_direct_claim_call_selector() {
        let encoded = encode_direct_claim(
            [0u8; 32],
            U256::from(100u64),
            Address::ZERO,
            Address::ZERO,
            Address::ZERO,
            U256::from(1000u64),
        );
        // function selector for claim(bytes32,uint256,address,address,address,uint256)
        // = keccak256("claim(bytes32,uint256,address,address,address,uint256)")[..4]
        let selector = &encoded[..4];
        let expected_selector = &claimCall::SELECTOR;
        assert_eq!(selector, expected_selector);
    }

    #[macros::test_all]
    fn test_claim_erc20_execute_encodes() {
        let claim = Erc20Claim {
            preimage: [1u8; 32].into(),
            amount: U256::from(100_000_000_000_000u64),
            tokenAddress: parse_address("0x6c84a8f1c29108F47a79964b5Fe888D4f4D0dE40").unwrap(),
            refundAddress: parse_address("0x0000000000000000000000000000000000000002").unwrap(),
            timelock: U256::from(12345u64),
            v: 27,
            r: [2u8; 32].into(),
            s: [3u8; 32].into(),
        };

        let calls = vec![Call {
            target: parse_address("0x0000000000000000000000000000000000000003").unwrap(),
            value: U256::ZERO,
            callData: vec![0xab, 0xcd].into(),
        }];

        let token = parse_address("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9").unwrap();
        let min_amount_out = U256::from(71_000_000u64);
        let destination = parse_address("0x0000000000000000000000000000000000000004").unwrap();

        let encoded = encode_claim_erc20_execute(
            &claim,
            &calls,
            token,
            min_amount_out,
            destination,
            28,
            [4u8; 32],
            [5u8; 32],
        );

        // Verify it starts with the correct function selector
        let expected_selector = &claimERC20ExecuteCall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
        // Encoded data should be non-trivial (claim struct + dynamic calls array + trailing params)
        assert!(encoded.len() > 500);
    }

    #[macros::test_all]
    fn test_claim_erc20_execute_oft_encodes() {
        let claim = Erc20Claim {
            preimage: [1u8; 32].into(),
            amount: U256::from(100_000_000_000_000u64),
            tokenAddress: Address::ZERO,
            refundAddress: Address::ZERO,
            timelock: U256::from(100u64),
            v: 27,
            r: [0u8; 32].into(),
            s: [0u8; 32].into(),
        };

        let send_data = SendData {
            dstEid: 30101,
            to: [0xaa; 32].into(),
            extraOptions: vec![].into(),
            composeMsg: vec![].into(),
            oftCmd: vec![].into(),
        };

        let auth = ClaimSendAuthorization {
            minAmountLd: U256::from(1000u64),
            lzTokenFee: U256::ZERO,
            refundAddress: Address::ZERO,
            v: 28,
            r: [0u8; 32].into(),
            s: [0u8; 32].into(),
        };

        let encoded = encode_claim_erc20_execute_oft(
            &claim,
            &[],
            Address::ZERO,
            Address::ZERO,
            &send_data,
            &auth,
        );

        let expected_selector = &claimERC20ExecuteOftCall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
        assert!(encoded.len() > 200);
    }

    #[macros::test_all]
    fn test_typehash_send_data_call_selector() {
        let encoded = encode_typehash_send_data_call();
        let expected_selector = &TYPEHASH_SEND_DATACall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
    }

    #[macros::test_all]
    fn test_parse_hex_bytes() {
        let bytes = parse_hex_bytes("0xabcdef").unwrap();
        assert_eq!(bytes, vec![0xab, 0xcd, 0xef]);

        let bytes_no_prefix = parse_hex_bytes("abcdef").unwrap();
        assert_eq!(bytes_no_prefix, vec![0xab, 0xcd, 0xef]);

        let empty = parse_hex_bytes("0x").unwrap();
        assert!(empty.is_empty());
    }

    // ─── OFT tests ───────────────────────────────────────────────────

    #[macros::test_all]
    fn test_address_to_bytes32() {
        let addr = parse_address("0x0000000000000000000000000000000000000042").unwrap();
        let b32 = address_to_bytes32(addr);
        // First 12 bytes should be zero-padding
        assert_eq!(&b32[..12], &[0u8; 12]);
        // Last 20 bytes should be the address
        assert_eq!(&b32[12..], addr.as_slice());
    }

    #[macros::test_all]
    fn test_address_to_bytes32_zero() {
        let b32 = address_to_bytes32(Address::ZERO);
        assert_eq!(b32, FixedBytes::<32>::ZERO);
    }

    #[macros::test_all]
    fn test_quote_oft_call_selector() {
        let send_param = build_oft_send_param(
            30101,
            FixedBytes::<32>::ZERO,
            U256::ZERO,
            U256::ZERO,
            alloy_primitives::Bytes::new(),
        );
        let encoded = encode_quote_oft(&send_param);
        let expected_selector = &quoteOFTCall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
    }

    #[macros::test_all]
    fn test_quote_send_call_selector() {
        let send_param = build_oft_send_param(
            30101,
            FixedBytes::<32>::ZERO,
            U256::ZERO,
            U256::ZERO,
            alloy_primitives::Bytes::new(),
        );
        let encoded = encode_quote_send(&send_param, false);
        let expected_selector = &quoteSendCall::SELECTOR;
        assert_eq!(&encoded[..4], expected_selector);
    }

    #[macros::test_all]
    fn test_approve_call_selector_and_roundtrip() {
        let spender = parse_address("0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92").unwrap();
        let amount = U256::from(0x1234_5678_u64);
        let encoded = encode_approve(spender, amount);

        assert_eq!(&encoded[..4], &approveCall::SELECTOR);
        // ERC20.approve(address,uint256) — well-known selector.
        assert_eq!(&encoded[..4], &[0x09, 0x5e, 0xa7, 0xb3]);
        // calldata = selector(4) + address(32) + uint256(32) = 68 bytes.
        assert_eq!(encoded.len(), 68);

        // Full roundtrip: decode the args and confirm they survived.
        let decoded = approveCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.spender, spender);
        assert_eq!(decoded.amount, amount);
    }

    #[macros::test_all]
    fn test_approve_max_uint256_roundtrip() {
        let spender = parse_address("0x77652D5aba086137b595875263FC200182919B92").unwrap();
        let encoded = encode_approve(spender, U256::MAX);
        let decoded = approveCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.spender, spender);
        assert_eq!(decoded.amount, U256::MAX);
    }

    #[macros::test_all]
    fn test_allowance_call_selector() {
        let owner = parse_address("0x6EA68e965fcd19b6fbC6553BABbF87a5018F9B28").unwrap();
        let spender = parse_address("0x77652D5aba086137b595875263FC200182919B92").unwrap();
        let encoded = encode_allowance(owner, spender);
        assert_eq!(&encoded[..4], &allowanceCall::SELECTOR);
        assert_eq!(&encoded[..4], &[0xdd, 0x62, 0xed, 0x3e]);
    }

    #[macros::test_all]
    fn test_decode_allowance_return() {
        // ABI-encoded uint256(0xdead_beef).
        let mut buf = [0u8; 32];
        buf[28..].copy_from_slice(&0xdead_beef_u32.to_be_bytes());
        let decoded = decode_allowance_return(&buf).unwrap();
        assert_eq!(decoded, U256::from(0xdead_beef_u64));
    }

    #[macros::test_all]
    fn test_approval_required_call_selector() {
        let encoded = encode_approval_required();
        assert_eq!(&encoded[..4], &approvalRequiredCall::SELECTOR);
        // Zero-arg calldata = 4 bytes.
        assert_eq!(encoded.len(), 4);
    }

    #[macros::test_all]
    fn test_decode_approval_required_return() {
        let mut true_bytes = [0u8; 32];
        true_bytes[31] = 1;
        assert!(decode_approval_required_return(&true_bytes).unwrap());

        let false_bytes = [0u8; 32];
        assert!(!decode_approval_required_return(&false_bytes).unwrap());
    }

    #[macros::test_all]
    fn test_hash_send_data_deterministic() {
        let typehash = [1u8; 32];
        let send_data = SendData {
            dstEid: 30101,
            to: [0xaa; 32].into(),
            extraOptions: vec![].into(),
            composeMsg: vec![].into(),
            oftCmd: vec![].into(),
        };

        let hash1 = hash_send_data(typehash, &send_data);
        let hash2 = hash_send_data(typehash, &send_data);
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, [0u8; 32]);
    }

    #[macros::test_all]
    fn test_hash_send_data_different_eid() {
        let typehash = [1u8; 32];
        let sd1 = SendData {
            dstEid: 30101,
            to: [0xaa; 32].into(),
            extraOptions: vec![].into(),
            composeMsg: vec![].into(),
            oftCmd: vec![].into(),
        };
        let sd2 = SendData {
            dstEid: 30111,
            ..sd1.clone()
        };
        assert_ne!(
            hash_send_data(typehash, &sd1),
            hash_send_data(typehash, &sd2)
        );
    }

    fn sample_cctp_data() -> CctpData {
        CctpData {
            destinationDomain: 6,
            mintRecipient: [0xaa; 32].into(),
            destinationCaller: [0u8; 32].into(),
            maxFee: U256::from(1_000u64),
            minFinalityThreshold: 1000,
            hookData: vec![].into(),
        }
    }

    #[macros::test_all]
    fn test_encode_claim_erc20_execute_cctp_selector() {
        let claim = Erc20Claim {
            preimage: [0u8; 32].into(),
            amount: U256::from(1u64),
            tokenAddress: Address::ZERO,
            refundAddress: Address::ZERO,
            timelock: U256::from(1u64),
            v: 28,
            r: [0u8; 32].into(),
            s: [0u8; 32].into(),
        };
        let auth = ClaimCctpAuthorization {
            minAmount: U256::from(900u64),
            v: 28,
            r: [0u8; 32].into(),
            s: [0u8; 32].into(),
        };

        let encoded = encode_claim_erc20_execute_cctp(
            &claim,
            &[],
            Address::ZERO,
            Address::ZERO,
            &sample_cctp_data(),
            &auth,
        );

        assert_eq!(&encoded[..4], &claimERC20ExecuteCctpCall::SELECTOR);
        // Round-trips back to the same fields.
        let decoded = claimERC20ExecuteCctpCall::abi_decode(&encoded).unwrap();
        assert_eq!(decoded.cctpData.destinationDomain, 6);
        assert_eq!(decoded.cctpData.minFinalityThreshold, 1000);
        assert_eq!(decoded.auth.minAmount, U256::from(900u64));
    }

    #[macros::test_all]
    fn test_typehash_cctp_data_call_selector() {
        let encoded = encode_typehash_cctp_data_call();
        assert_eq!(&encoded[..4], &TYPEHASH_CCTP_DATACall::SELECTOR);
    }

    #[macros::test_all]
    fn test_hash_cctp_data_deterministic() {
        let typehash = [1u8; 32];
        let d = sample_cctp_data();
        assert_eq!(hash_cctp_data(typehash, &d), hash_cctp_data(typehash, &d));
        assert_ne!(hash_cctp_data(typehash, &d), [0u8; 32]);
    }

    #[macros::test_all]
    fn test_hash_cctp_data_field_sensitivity() {
        let typehash = [1u8; 32];
        let base = sample_cctp_data();

        let diff_domain = CctpData {
            destinationDomain: 7,
            ..base.clone()
        };
        assert_ne!(
            hash_cctp_data(typehash, &base),
            hash_cctp_data(typehash, &diff_domain)
        );

        // hookData is hashed into the struct hash, so it must affect the result.
        let diff_hook = CctpData {
            hookData: vec![0xde, 0xad].into(),
            ..base.clone()
        };
        assert_ne!(
            hash_cctp_data(typehash, &base),
            hash_cctp_data(typehash, &diff_hook)
        );
    }

    #[macros::test_all]
    fn test_hash_cctp_data_hashes_hookdata() {
        use alloy_primitives::keccak256;
        // Empty hookData must be folded in as keccak256("") — the canonical
        // empty-bytes hash. Guards against accidentally inlining raw hookData.
        let empty_keccak: [u8; 32] =
            hex::decode("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
                .unwrap()
                .try_into()
                .unwrap();
        assert_eq!(keccak256(b"").as_slice(), &empty_keccak);
    }

    /// Byte-for-byte golden vector against boltz-web-app's `cctp/evm.spec.ts`.
    /// Pins both the canonical `CctpData` EIP-712 typehash and the full struct
    /// hash for a fixed input — the reference's `cctpDataTypehash` and
    /// `hashCctpData(sample)`. A mismatch means the ABI field order/units or
    /// the typehash string diverged from the Router the signature is verified
    /// against, which would make every CCTP claim signature revert on-chain.
    #[macros::test_all]
    fn test_hash_cctp_data_matches_web_app_golden() {
        use alloy_primitives::keccak256;

        // 1. cctpDataTypehash — keccak256 of the canonical type string.
        let canonical = "CctpData(uint32 destinationDomain,bytes32 mintRecipient,bytes32 destinationCaller,uint256 maxFee,uint32 minFinalityThreshold,bytes32 hookData)";
        let typehash: [u8; 32] = keccak256(canonical.as_bytes()).into();
        assert_eq!(
            hex::encode(typehash),
            "9b5b1c929227bcc37f83e385e88fc739668266cfce6830b07fceef394627016f"
        );

        // 2. hashCctpData(sample) — mintRecipient = addressToBytes32(0x1111..1111),
        // destinationCaller = bytes32(0), maxFee = 100, minFinalityThreshold =
        // 1000, empty hookData.
        let sample = CctpData {
            destinationDomain: 0,
            mintRecipient: address_to_bytes32(
                parse_address("0x1111111111111111111111111111111111111111").unwrap(),
            ),
            destinationCaller: FixedBytes::<32>::ZERO,
            maxFee: U256::from(100u64),
            minFinalityThreshold: 1000,
            hookData: vec![].into(),
        };
        assert_eq!(
            hex::encode(hash_cctp_data(typehash, &sample)),
            "7680d81fad262508a7c1de25f63bb8fa2d7594056fa551b9978e34330c5941c5"
        );
    }

    /// CCTP v2 event topic hashes pinned against Circle's on-chain signature
    /// fixtures (boltz-web-app `cctp/events.spec.ts`). Independent of alloy's
    /// `SIGNATURE_HASH` derivation: a typo in the event declaration would make
    /// every `MessageSent`/`MintAndWithdraw` log query silently miss.
    #[macros::test_all]
    fn test_cctp_event_topics_match_circle_fixtures() {
        use alloy_primitives::keccak256;
        // Literal fixtures pinned in the web app's `cctp/events.spec.ts`.
        assert_eq!(
            hex::encode(MessageSent::SIGNATURE_HASH.as_slice()),
            "8c5261668696ce22758910d05bab8f186d6eb247ceac2af2e82c7dc17669b036"
        );
        assert_eq!(
            hex::encode(MintAndWithdraw::SIGNATURE_HASH.as_slice()),
            "50c55e915134d457debfa58eb6f4342956f8b0616d51a89a3659360178e1ab63"
        );
        // OFTSent: the web app pins it against keccak256 of the canonical
        // signature string rather than a literal — mirror that.
        assert_eq!(
            OFTSent::SIGNATURE_HASH.as_slice(),
            keccak256(b"OFTSent(bytes32,uint32,address,uint256,uint256)").as_slice()
        );
    }

    /// Independent validation of the CCTP burn-message byte offsets by building
    /// a message from its field layout (per Circle's `MessageV2` + `BurnMessageV2`)
    /// rather than by writing to the same offsets the decoder reads. Mirrors
    /// boltz-web-app `cctp/events.spec.ts` `parseCctpBurnMessage`. If an offset
    /// constant drifted, the amount/fee/nonce would decode wrong here even
    /// though the crate's self-referential test still passed.
    #[macros::test_all]
    fn test_cctp_burn_message_offsets_from_field_layout() {
        // Outer header: version(4) srcDomain(4) dstDomain(4) nonce(32)
        // sender(32) recipient(32) destinationCaller(32) minFinality(4)
        // finalityExecuted(4) = 148 bytes, then the body.
        let mut msg = Vec::new();
        msg.extend_from_slice(&2u32.to_be_bytes()); // version = 2
        msg.extend_from_slice(&3u32.to_be_bytes()); // sourceDomain = 3
        msg.extend_from_slice(&6u32.to_be_bytes()); // destinationDomain = 6
        let nonce = [0x44u8; 32];
        msg.extend_from_slice(&nonce); // nonce
        msg.extend_from_slice(&[0x22u8; 32]); // sender
        msg.extend_from_slice(&[0x11u8; 32]); // recipient
        msg.extend_from_slice(&[0u8; 32]); // destinationCaller
        msg.extend_from_slice(&1000u32.to_be_bytes()); // minFinalityThreshold
        msg.extend_from_slice(&1000u32.to_be_bytes()); // finalityThresholdExecuted
        assert_eq!(msg.len(), CCTP_BODY_OFFSET);
        // BurnMessage body: version(4) burnToken(32) mintRecipient(32)
        // amount(32) messageSender(32) maxFee(32) feeExecuted(32) expiry(32).
        msg.extend_from_slice(&1u32.to_be_bytes()); // body version
        msg.extend_from_slice(&[0u8; 32]); // burnToken
        msg.extend_from_slice(&[0x11u8; 32]); // mintRecipient
        msg.extend_from_slice(&U256::from(1_000_000u64).to_be_bytes::<32>()); // amount
        msg.extend_from_slice(&[0u8; 32]); // messageSender
        msg.extend_from_slice(&[0u8; 32]); // maxFee
        msg.extend_from_slice(&U256::from(130u64).to_be_bytes::<32>()); // feeExecuted
        msg.extend_from_slice(&[0u8; 32]); // expirationBlock

        let message_hex = format!("0x{}", hex::encode(&msg));
        // amountReceived = 1_000_000 - 130 = 999_870 (web-app golden).
        assert_eq!(
            decode_cctp_delivered_from_message(&message_hex),
            Some(999_870)
        );
        // Offsets land the right fields.
        assert_eq!(read_u32_be(&msg, CCTP_SOURCE_DOMAIN_OFFSET), Some(3));
        assert_eq!(decode_cctp_nonce_from_message(&message_hex), Some(nonce));
    }

    #[macros::test_all]
    fn test_build_oft_send_param_empty_extra_options() {
        let addr = parse_address("0x0000000000000000000000000000000000000042").unwrap();
        let to = address_to_bytes32(addr);
        let sp = build_oft_send_param(
            30111,
            to,
            U256::from(1000u64),
            U256::from(900u64),
            alloy_primitives::Bytes::new(),
        );
        assert_eq!(sp.dstEid, 30111);
        assert_eq!(sp.to, to);
        assert_eq!(&sp.to[12..], addr.as_slice());
        assert_eq!(sp.amountLD, U256::from(1000u64));
        assert_eq!(sp.minAmountLD, U256::from(900u64));
        assert!(sp.extraOptions.is_empty());
        assert!(sp.composeMsg.is_empty());
        assert!(sp.oftCmd.is_empty());
    }

    #[macros::test_all]
    fn test_build_oft_send_param_with_extra_options() {
        let to = FixedBytes::<32>::from([0xcc; 32]);
        let extra = alloy_primitives::Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);
        let sp = build_oft_send_param(30168, to, U256::from(5u64), U256::from(4u64), extra.clone());
        assert_eq!(sp.extraOptions, extra);
        assert!(sp.composeMsg.is_empty());
        assert!(sp.oftCmd.is_empty());
    }

    // ─── Delivered-amount decoding tests ─────────────────────────────

    use crate::evm::provider::LogEntry;

    const NATIVE_OFT: &str = "0x14E4A1B13bf7F943c8ff7C51fb60FA964A298D92";
    const LEGACY_OFT: &str = "0x77652D5aba086137b595875263FC200182919B92";
    const USDT_TOKEN: &str = "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9";

    fn mk_log(address: &str, topics: Vec<String>, data: &str) -> LogEntry {
        LogEntry {
            address: address.to_string(),
            topics,
            data: data.to_string(),
            block_number: "0x1".to_string(),
            transaction_hash: "0x0".to_string(),
        }
    }

    fn transfer_topic0() -> String {
        format!("0x{}", hex::encode(Transfer::SIGNATURE_HASH.as_slice()))
    }

    fn oft_sent_topic0() -> String {
        format!("0x{}", hex::encode(OFTSent::SIGNATURE_HASH.as_slice()))
    }

    /// Real legacy-mesh `OFTSent` payload from Arbitrum tx
    /// 0x99b6dbaf789231089316fb51838db6fa2af61093c0c239579a01cc82f6d7d2a1.
    /// dstEid=0x75ad, amountSentLD=0x1e86d9, amountReceivedLD=0x1e8481.
    const LEGACY_OFT_DATA: &str = "0x00000000000000000000000000000000000000000000000000000000000075ad00000000000000000000000000000000000000000000000000000000001e86d900000000000000000000000000000000000000000000000000000000001e8481";

    const LEGACY_GUID: &str = "0xfa94e9d0c5fb3816e30e5718deac0d3e1d526cff806e66a9092940f23d59e386";

    const LEGACY_FROM_TOPIC: &str =
        "0x0000000000000000000000008de29a04dee5894d7bd536a7b4c924560f2dff57";

    fn encode_uint256(value: u128) -> String {
        let bytes = U256::from(value).abi_encode();
        format!("0x{}", hex::encode(bytes))
    }

    #[macros::test_all]
    fn test_decode_legacy_mesh_oft_sent() {
        let log = mk_log(
            LEGACY_OFT,
            vec![
                oft_sent_topic0(),
                LEGACY_GUID.to_string(),
                LEGACY_FROM_TOPIC.to_string(),
            ],
            LEGACY_OFT_DATA,
        );

        let oft_contract = parse_address(LEGACY_OFT).unwrap();
        let result =
            decode_delivered_from_logs(&[log], &DeliveredAmountSource::OftSent { oft_contract })
                .unwrap();

        assert_eq!(result.amount, 0x001e_8481);
        assert_eq!(result.lz_guid.as_deref(), Some(LEGACY_GUID));
    }

    #[macros::test_all]
    fn test_decode_native_mesh_oft_sent() {
        // Synthetic: same event shape, different contract address and values.
        let guid = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let from_topic = "0x00000000000000000000000000000000000000000000000000000000000000aa";
        let data = {
            // dstEid=30101 (Ethereum), amountSent=1_000_000, amountRecv=1_000_000 (native, no fee)
            let eid = U256::from(30101u64).abi_encode();
            let sent = U256::from(1_000_000u64).abi_encode();
            let recv = U256::from(1_000_000u64).abi_encode();
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&eid);
            bytes.extend_from_slice(&sent);
            bytes.extend_from_slice(&recv);
            format!("0x{}", hex::encode(bytes))
        };

        let log = mk_log(
            NATIVE_OFT,
            vec![oft_sent_topic0(), guid.to_string(), from_topic.to_string()],
            &data,
        );

        let oft_contract = parse_address(NATIVE_OFT).unwrap();
        let result =
            decode_delivered_from_logs(&[log], &DeliveredAmountSource::OftSent { oft_contract })
                .unwrap();

        assert_eq!(result.amount, 1_000_000);
        assert_eq!(result.lz_guid.as_deref(), Some(guid));
    }

    fn cctp_message_sent_topic() -> String {
        format!("0x{}", hex::encode(MessageSent::SIGNATURE_HASH.as_slice()))
    }

    /// Build a synthetic CCTP message and its `MessageSent` log data:
    /// `sourceDomain` at byte 4, burn `amount` at byte 216, `feeExecuted` at
    /// byte 312.
    fn cctp_message_sent_data(source_domain: u32, amount: u128, fee: u128) -> String {
        let mut msg = vec![0u8; CCTP_BURN_FEE_OFFSET + 32];
        msg[CCTP_SOURCE_DOMAIN_OFFSET..CCTP_SOURCE_DOMAIN_OFFSET + 4]
            .copy_from_slice(&source_domain.to_be_bytes());
        msg[CCTP_BURN_AMOUNT_OFFSET..CCTP_BURN_AMOUNT_OFFSET + 32]
            .copy_from_slice(&U256::from(amount).to_be_bytes::<32>());
        msg[CCTP_BURN_FEE_OFFSET..CCTP_BURN_FEE_OFFSET + 32]
            .copy_from_slice(&U256::from(fee).to_be_bytes::<32>());
        let encoded = alloy_primitives::Bytes::from(msg).abi_encode();
        format!("0x{}", hex::encode(encoded))
    }

    const CCTP_MT: &str = "0x81D40F21F12A8F0E3252Bccb954D722d4c464B64";

    #[macros::test_all]
    fn test_decode_cctp_message_sent() {
        let log = mk_log(
            CCTP_MT,
            vec![cctp_message_sent_topic()],
            &cctp_message_sent_data(3, 1_000_000, 7),
        );
        let message_transmitter = parse_address(CCTP_MT).unwrap();
        let result = decode_delivered_from_logs(
            &[log],
            &DeliveredAmountSource::Cctp {
                message_transmitter,
            },
        )
        .unwrap();

        // Delivered estimate = burn amount - executed fee.
        assert_eq!(result.amount, 1_000_000 - 7);
        assert_eq!(result.cctp_source_domain, Some(3));
        assert_eq!(result.lz_guid, None);
    }

    #[macros::test_all]
    fn test_decode_cctp_delivered_from_attested_message() {
        // Build a raw attested message: amount 1_000_000 at byte 216, the
        // finalized feeExecuted 250 at byte 312. Unlike the source log, the
        // attested message carries the real fee, so delivered = 999_750.
        let mut msg = vec![0u8; CCTP_BURN_FEE_OFFSET + 32];
        msg[CCTP_BURN_AMOUNT_OFFSET..CCTP_BURN_AMOUNT_OFFSET + 32]
            .copy_from_slice(&U256::from(1_000_000u64).to_be_bytes::<32>());
        msg[CCTP_BURN_FEE_OFFSET..CCTP_BURN_FEE_OFFSET + 32]
            .copy_from_slice(&U256::from(250u64).to_be_bytes::<32>());
        let message_hex = format!("0x{}", hex::encode(&msg));

        assert_eq!(
            decode_cctp_delivered_from_message(&message_hex),
            Some(1_000_000 - 250)
        );
        // Malformed/short message yields None.
        assert_eq!(decode_cctp_delivered_from_message("0x1234"), None);
    }

    #[macros::test_all]
    fn test_decode_cctp_nonce_from_message() {
        // Real header prefix of the stuck swap's attested message:
        // version | srcDomain=3 | dstDomain=5 | nonce(32). The nonce must be
        // read verbatim at offset 12, matching what Iris reports as eventNonce.
        let message_hex = "0x000000010000000300000005\
d7c7c073ec476983e3f222924974a48a7f61a7045df31dcf3ed83172bf0bb478\
0000000000000000000000000000000000000000000000000000000000000000";
        let nonce = decode_cctp_nonce_from_message(message_hex).expect("nonce");
        assert_eq!(
            hex::encode(nonce),
            "d7c7c073ec476983e3f222924974a48a7f61a7045df31dcf3ed83172bf0bb478"
        );
        // Too short to hold the header nonce → None.
        assert_eq!(decode_cctp_nonce_from_message("0x1234"), None);
    }

    #[macros::test_all]
    fn test_decode_cctp_ignores_wrong_emitter() {
        let message_transmitter = parse_address(CCTP_MT).unwrap();
        // Same event, but emitted by a different contract — must not match.
        let log = mk_log(
            USDT_TOKEN,
            vec![cctp_message_sent_topic()],
            &cctp_message_sent_data(3, 1_000_000, 0),
        );
        assert!(
            decode_delivered_from_logs(
                &[log],
                &DeliveredAmountSource::Cctp {
                    message_transmitter
                }
            )
            .is_none()
        );
    }

    #[macros::test_all]
    fn test_decode_arbitrum_transfer_to_user() {
        let user = parse_address("0x1234567890abcdef1234567890abcdef12345678").unwrap();
        let user_topic = address_to_topic(&user.into_array());
        let from_topic = "0x000000000000000000000000000000000000000000000000000000000000beef";

        let log = mk_log(
            USDT_TOKEN,
            vec![transfer_topic0(), from_topic.to_string(), user_topic],
            &encode_uint256(42_000_000),
        );

        let token = parse_address(USDT_TOKEN).unwrap();
        let result = decode_delivered_from_logs(
            &[log],
            &DeliveredAmountSource::ArbitrumTransfer { token, user },
        )
        .unwrap();

        assert_eq!(result.amount, 42_000_000);
        assert_eq!(result.lz_guid, None);
    }

    #[macros::test_all]
    fn test_decode_arbitrum_transfer_picks_correct_to() {
        // Two Transfer logs on USDT, only one goes to the user.
        let user = parse_address("0x1234567890abcdef1234567890abcdef12345678").unwrap();
        let user_topic = address_to_topic(&user.into_array());
        let other = parse_address("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let other_topic = address_to_topic(&other.into_array());
        let zero_topic = "0x0000000000000000000000000000000000000000000000000000000000000000";

        let log_other = mk_log(
            USDT_TOKEN,
            vec![transfer_topic0(), zero_topic.to_string(), other_topic],
            &encode_uint256(999),
        );
        let log_user = mk_log(
            USDT_TOKEN,
            vec![transfer_topic0(), zero_topic.to_string(), user_topic],
            &encode_uint256(77_777),
        );

        let token = parse_address(USDT_TOKEN).unwrap();
        let result = decode_delivered_from_logs(
            &[log_other, log_user],
            &DeliveredAmountSource::ArbitrumTransfer { token, user },
        )
        .unwrap();

        assert_eq!(result.amount, 77_777);
    }

    #[macros::test_all]
    fn test_decode_no_match_returns_none() {
        // An OFTSent-looking log from the wrong contract is not a match.
        let unrelated = "0x0000000000000000000000000000000000000dead";
        let guid = "0x2222222222222222222222222222222222222222222222222222222222222222";
        let from_topic = "0x00000000000000000000000000000000000000000000000000000000000000aa";
        let log = mk_log(
            unrelated,
            vec![oft_sent_topic0(), guid.to_string(), from_topic.to_string()],
            LEGACY_OFT_DATA,
        );

        let oft_contract = parse_address(NATIVE_OFT).unwrap();
        let result =
            decode_delivered_from_logs(&[log], &DeliveredAmountSource::OftSent { oft_contract });
        assert!(result.is_none());
    }

    #[macros::test_all]
    fn test_decode_ignores_transfer_to_wrong_user() {
        let user = parse_address("0x1234567890abcdef1234567890abcdef12345678").unwrap();
        let other = parse_address("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let other_topic = address_to_topic(&other.into_array());
        let zero_topic = "0x0000000000000000000000000000000000000000000000000000000000000000";

        let log = mk_log(
            USDT_TOKEN,
            vec![transfer_topic0(), zero_topic.to_string(), other_topic],
            &encode_uint256(999),
        );

        let token = parse_address(USDT_TOKEN).unwrap();
        let result = decode_delivered_from_logs(
            &[log],
            &DeliveredAmountSource::ArbitrumTransfer { token, user },
        );
        assert!(result.is_none());
    }

    #[macros::test_all]
    fn test_decode_empty_logs_returns_none() {
        let oft_contract = parse_address(LEGACY_OFT).unwrap();
        let result =
            decode_delivered_from_logs(&[], &DeliveredAmountSource::OftSent { oft_contract });
        assert!(result.is_none());
    }
}
