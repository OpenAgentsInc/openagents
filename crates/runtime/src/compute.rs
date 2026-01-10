//! Compute filesystem service and providers.

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
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
use crate::identity::SigningService;
use crate::types::{AgentId, Timestamp};
#[cfg(not(target_arch = "wasm32"))]
use crate::wallet::{block_on_wallet, WalletFxProvider, WalletService};
#[cfg(all(feature = "browser", target_arch = "wasm32"))]
use crate::wasm_http;
#[cfg(not(target_arch = "wasm32"))]
use nostr::{
    DELETION_REQUEST_KIND, HandlerInfo, HandlerType, JobInput, JobRequest, JobResult, JobStatus,
    KIND_HANDLER_INFO, KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT,
    KIND_JOB_TEXT_GENERATION, UnsignedEvent, create_deletion_tags, get_event_hash, get_result_kind,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_arch = "wasm32"))]
use tokio::sync::{RwLock as TokioRwLock, mpsc};
#[cfg(all(
    target_arch = "wasm32",
    any(feature = "cloudflare", feature = "browser")
))]
use wasm_bindgen_futures::spawn_local;
#[cfg(feature = "cloudflare")]
use worker::Ai;

#[cfg(not(target_arch = "wasm32"))]
use ::compute as compute_provider;
#[cfg(not(target_arch = "wasm32"))]
use compute_provider::backends::{BackendError, InferenceBackend};
#[cfg(not(target_arch = "wasm32"))]
use compute_provider::backends::{
    BackendRegistry, CompletionRequest, CompletionResponse, UsageInfo as BackendUsageInfo,
};

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);

include!("compute/types.rs");
include!("compute/router.rs");
include!("compute/service.rs");
include!("compute/providers/local.rs");
include!("compute/providers/cloudflare.rs");
include!("compute/providers/browser.rs");
include!("compute/providers/dvm.rs");
include!("compute/support.rs");
