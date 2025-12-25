//! Atoms - The smallest UI building blocks.
//!
//! Atoms are the fundamental UI elements that cannot be broken down further.
//! They include status indicators, badges, icons, and simple interactive elements.

mod bech32_entity;
mod bitcoin_amount;
mod checkpoint_badge;
mod content_type_icon;
mod entry_marker;
mod event_kind_badge;
mod feedback_button;
mod keybinding_hint;
mod mode_badge;
mod model_badge;
mod network_badge;
mod payment_method_icon;
mod payment_status_badge;
mod permission_button;
mod relay_status;
mod status_dot;
mod streaming_indicator;
mod thinking_toggle;
mod tool_icon;
mod tool_status_badge;

pub use bech32_entity::{Bech32Entity, Bech32Type};
pub use bitcoin_amount::{AmountDirection, BitcoinAmount, BitcoinUnit};
pub use checkpoint_badge::CheckpointBadge;
pub use content_type_icon::{ContentType, ContentTypeIcon};
pub use entry_marker::{EntryMarker, EntryType};
pub use event_kind_badge::{EventKind, EventKindBadge};
pub use feedback_button::{FeedbackButton, FeedbackType};
pub use keybinding_hint::KeybindingHint;
pub use mode_badge::{Mode, ModeBadge};
pub use model_badge::{Model, ModelBadge};
pub use network_badge::{BitcoinNetwork, NetworkBadge};
pub use payment_method_icon::{PaymentMethod, PaymentMethodIcon};
pub use payment_status_badge::{PaymentStatus, PaymentStatusBadge};
pub use permission_button::{PermissionAction, PermissionButton};
pub use relay_status::{RelayStatus, RelayStatusBadge, RelayStatusDot};
pub use status_dot::{Status, StatusDot};
pub use streaming_indicator::StreamingIndicator;
pub use thinking_toggle::ThinkingToggle;
pub use tool_icon::{ToolIcon, ToolType};
pub use tool_status_badge::{ToolStatus, ToolStatusBadge};
