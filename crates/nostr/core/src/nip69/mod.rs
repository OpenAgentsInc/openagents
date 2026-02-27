//! NIP-69: Peer-to-peer Order Events
//!
//! Defines peer-to-peer order events for decentralized marketplace trading.
//! Creates a unified liquidity pool across P2P platforms.
//!
//! Internal module boundaries:
//! - `model`: order model + parsing/validation
//! - `tags`: tag construction helpers
//! - `builder`: fluent order builder
//! - `tests`: protocol coverage
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/69.md>

mod builder;
mod model;
mod tags;

pub use builder::P2POrderBuilder;
pub use model::{
    BitcoinLayer, DOCUMENT_TYPE, Nip69Error, OrderStatus, OrderType, P2P_ORDER_KIND, P2POrder,
    Rating, is_p2p_order_kind,
};
pub use tags::{
    create_amount_tag, create_currency_tag, create_document_tag, create_fiat_amount_tag,
    create_layer_tag, create_order_id_tag, create_order_type_tag, create_payment_method_tag,
    create_platform_tag, create_status_tag,
};

#[cfg(test)]
mod tests;
