//! Container filesystem service and providers.

#![allow(missing_docs)]

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::compute::{ApiTokenProvider, Prefer};
#[cfg(not(target_arch = "wasm32"))]
use crate::dvm::{
    DvmFeedbackStatus, DvmTransport, RelayPoolTransport, msats_to_sats, parse_feedback_event,
};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::fx::{FxRateCache, FxRateProvider, FxSource};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::identity::{PublicKey, Signature, SigningService};
use crate::storage::AgentStorage;
use crate::types::{AgentId, Timestamp};
#[cfg(not(target_arch = "wasm32"))]
use crate::wallet::{block_on_wallet, WalletFxProvider, WalletService};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use crate::wasm_http;
use bech32::{Bech32, Hrp};
#[cfg(not(target_arch = "wasm32"))]
use compute::domain::sandbox_run::{
    ResourceLimits as SandboxResourceLimits, SandboxRunRequest, SandboxRunResult,
};
#[cfg(not(target_arch = "wasm32"))]
use daytona::{
    CreateSandbox, DaytonaClient, DaytonaConfig, ExecuteRequest, GitCloneRequest,
    SandboxState as DaytonaSandboxState,
};
#[cfg(not(target_arch = "wasm32"))]
use nostr::nip90::KIND_JOB_SANDBOX_RUN;
#[cfg(not(target_arch = "wasm32"))]
use nostr::{
    DELETION_REQUEST_KIND, JobRequest, JobResult, JobStatus, KIND_JOB_FEEDBACK, UnsignedEvent,
    create_deletion_tags, get_event_hash,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
#[cfg(not(target_arch = "wasm32"))]
use std::io::{Read, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::mpsc;
use urlencoding::{decode, encode};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use wasm_bindgen_futures::spawn_local;

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);
const CHUNK_SIZE: u64 = 1_048_576;
const MAX_PATH_LEN: usize = 4096;
const AUTH_STATE_KEY: &str = "containers:auth:state";
const AUTH_TOKEN_KEY: &str = "containers:auth:token";
const AUTH_CHALLENGE_KEY: &str = "containers:auth:challenge";
const AUTH_CHALLENGE_TTL: Duration = Duration::from_secs(300);
const OPENAGENTS_API_URL_ENV: &str = "OPENAGENTS_API_URL";
const DAYTONA_API_URL_ENV: &str = "DAYTONA_API_URL";
const DAYTONA_BASE_URL_ENV: &str = "DAYTONA_BASE_URL";
const DAYTONA_API_KEY_ENV: &str = "DAYTONA_API_KEY";
const DAYTONA_ORG_ID_ENV: &str = "DAYTONA_ORG_ID";
const DAYTONA_TARGET_ENV: &str = "DAYTONA_TARGET";
const DAYTONA_SNAPSHOT_ENV: &str = "DAYTONA_SNAPSHOT";
const DAYTONA_DEFAULT_SNAPSHOT_ENV: &str = "DAYTONA_DEFAULT_SNAPSHOT";
const DAYTONA_AUTO_STOP_ENV: &str = "DAYTONA_AUTO_STOP_MINUTES";
const DAYTONA_AUTO_DELETE_ENV: &str = "DAYTONA_AUTO_DELETE_MINUTES";

include!("containers/types.rs");
include!("containers/router.rs");
include!("containers/openagents.rs");
include!("containers/service.rs");
include!("containers/providers/dvm.rs");
include!("containers/providers/openagents.rs");
include!("containers/providers/daytona.rs");
include!("containers/providers/browser.rs");
include!("containers/providers/apple.rs");
include!("containers/providers/docker.rs");
