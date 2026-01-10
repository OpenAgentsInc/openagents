//! Claude filesystem service and providers.

#![allow(missing_docs)]

use crate::budget::{BudgetError, BudgetPolicy, BudgetReservation, BudgetTracker};
use crate::fs::{
    BytesHandle, DirEntry, FileHandle, FileService, FsError, FsResult, OpenFlags, Permissions,
    SeekFrom, Stat, WatchEvent, WatchHandle,
};
use crate::idempotency::{IdempotencyJournal, JournalError};
use crate::identity::{PublicKey, Signature, SigningService};
use crate::types::{AgentId, Timestamp};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

const IDEMPOTENCY_TTL: Duration = Duration::from_secs(3600);
const AUTH_CHALLENGE_TTL: Duration = Duration::from_secs(300);

include!("claude/types.rs");
include!("claude/router.rs");
include!("claude/fs.rs");
include!("claude/support.rs");
