//! NIP-15 marketplace events.
//!
//! Stalls, products, auctions, and marketplace pages are addressable.
//! A bid is a regular kind `1021` event. Its content is a non-negative
//! integer, and its one `e` tag is one auction event id. Checkout
//! messages are plaintext JSON of types 0, 1, and 2. They travel inside
//! NIP-04, so a relay does not read them.
//!
//! The pinned text marks this NIP unrecommended. It stays off the NIP-11
//! list. Costs are JSON numbers. This module does not settle a payment.

use serde_json::{Map, Value};

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const STALL_KIND: u16 = 30_017;
const PRODUCT_KIND: u16 = 30_018;
const MARKET_KIND: u16 = 30_019;
const AUCTION_KIND: u16 = 30_020;
const BID_KIND: u16 = 1_021;
const BID_CONFIRMATION_KIND: u16 = 1_022;

/// One shipping zone on a stall.
#[derive(Clone, Debug, PartialEq)]
pub struct ShippingZone {
    pub id: String,
    pub name: Option<String>,
    pub cost: f64,
    pub regions: Vec<String>,
}

/// A kind `30017` stall.
#[derive(Clone, Debug, PartialEq)]
pub struct Stall {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub shipping: Vec<ShippingZone>,
}

/// Extra shipping for one product and one zone.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductShipping {
    pub id: String,
    pub cost: f64,
}

/// A kind `30018` product.
#[derive(Clone, Debug, PartialEq)]
pub struct Product {
    pub id: String,
    pub stall_id: String,
    pub name: String,
    pub description: Option<String>,
    pub images: Vec<String>,
    pub currency: String,
    pub price: f64,
    /// `None` means the merchant published unlimited availability.
    pub quantity: Option<u64>,
    pub specs: Vec<(String, String)>,
    pub shipping: Vec<ProductShipping>,
    pub categories: Vec<String>,
}

/// A kind `30019` marketplace page. Every field is optional.
#[derive(Clone, Debug, PartialEq)]
pub struct MarketplacePage {
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture: Option<String>,
    pub banner: Option<String>,
    pub theme: Option<String>,
    pub dark_mode: Option<bool>,
    pub merchants: Vec<String>,
}

/// A kind `30020` auction.
#[derive(Clone, Debug, PartialEq)]
pub struct Auction {
    pub id: String,
    pub stall_id: String,
    pub name: String,
    pub description: Option<String>,
    pub images: Vec<String>,
    pub starting_bid: u64,
    pub start_date: Option<u64>,
    pub duration: u64,
    pub specs: Vec<(String, String)>,
    pub shipping: Vec<ProductShipping>,
}

/// A kind `1021` bid. `auction_event_id` is one version of an auction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bid {
    pub amount: u64,
    pub auction_event_id: String,
}

/// How a merchant answered a bid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidStatus {
    Accepted,
    Rejected,
    Pending,
    Winner,
}

/// A kind `1022` bid confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidConfirmation {
    pub status: BidStatus,
    pub message: Option<String>,
    pub duration_extended: Option<u64>,
    pub bid_event_id: String,
    pub auction_event_id: String,
}

/// One line of a customer order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderItem {
    pub product_id: String,
    pub quantity: u64,
}

/// Checkout message type 0, from the customer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub id: String,
    pub name: Option<String>,
    pub address: Option<String>,
    pub message: Option<String>,
    pub contact_nostr: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
    pub items: Vec<OrderItem>,
    pub shipping_id: String,
}

/// One way to pay. The JSON field is `type`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentOption {
    pub method: String,
    pub link: String,
}

/// Checkout message type 1, from the merchant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaymentRequest {
    pub id: String,
    pub message: Option<String>,
    pub payment_options: Vec<PaymentOption>,
}

/// Checkout message type 2, from the merchant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderStatus {
    pub id: String,
    pub message: String,
    pub paid: bool,
    pub shipped: bool,
}

/// A decrypted checkout body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checkout {
    Order(Order),
    Payment(PaymentRequest),
    Status(OrderStatus),
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn json_object(text: &str) -> Result<Map<String, Value>, DomainError> {
    let value: Value =
        serde_json::from_str(text).map_err(|_| invalid("marketplace content must be JSON"))?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("marketplace content must be a JSON object"))
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String, DomainError> {
    let Some(value) = object.get(key) else {
        return Err(invalid("a required marketplace field is missing"));
    };
    let Some(text) = value.as_str() else {
        return Err(invalid("a required marketplace field must be a string"));
    };
    if text.is_empty() {
        return Err(invalid("a required marketplace field is empty"));
    }
    Ok(text.to_owned())
}

fn optional_string(object: &Map<String, Value>, key: &str) -> Result<Option<String>, DomainError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(text) = value.as_str() else {
        return Err(invalid("an optional marketplace field must be a string"));
    };
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text.to_owned()))
    }
}

fn required_finite(object: &Map<String, Value>, key: &str) -> Result<f64, DomainError> {
    let Some(value) = object.get(key).and_then(Value::as_f64) else {
        return Err(invalid("a marketplace cost must be a number"));
    };
    if !value.is_finite() || value < 0.0 {
        return Err(invalid(
            "a marketplace cost must be a non-negative finite number",
        ));
    }
    Ok(value)
}

fn required_u64(object: &Map<String, Value>, key: &str) -> Result<u64, DomainError> {
    let Some(value) = object.get(key).and_then(Value::as_u64) else {
        return Err(invalid(
            "a marketplace count must be a non-negative integer",
        ));
    };
    Ok(value)
}

fn optional_u64(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, DomainError> {
    let Some(value) = object.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .map(Some)
        .ok_or_else(|| invalid("a marketplace count must be a non-negative integer"))
}

fn one_d_tag(event: &Event) -> Result<&str, DomainError> {
    let mut values = event.tag_values("d");
    let Some(id) = values.next() else {
        return Err(invalid("a marketplace address requires one d tag"));
    };
    if id.is_empty() || values.next().is_some() {
        return Err(invalid("a marketplace address requires one d tag"));
    }
    Ok(id)
}

fn same_id(event: &Event, id: &str) -> Result<(), DomainError> {
    if one_d_tag(event)? == id {
        Ok(())
    } else {
        Err(invalid("the d tag must equal the marketplace id"))
    }
}

fn specs(object: &Map<String, Value>) -> Result<Vec<(String, String)>, DomainError> {
    let Some(value) = object.get("specs") else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err(invalid("specs must be an array of pairs"));
    };
    let mut parsed = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(pair) = row.as_array() else {
            return Err(invalid("specs must be an array of pairs"));
        };
        if pair.len() != 2 {
            return Err(invalid("specs must be an array of pairs"));
        }
        let Some(key) = pair[0].as_str().filter(|text| !text.is_empty()) else {
            return Err(invalid("specs must be an array of pairs"));
        };
        let Some(spec) = pair[1].as_str() else {
            return Err(invalid("specs must be an array of pairs"));
        };
        parsed.push((key.to_owned(), spec.to_owned()));
    }
    Ok(parsed)
}

fn images(object: &Map<String, Value>) -> Result<Vec<String>, DomainError> {
    let Some(value) = object.get("images") else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err(invalid("images must be an array of strings"));
    };
    rows.iter()
        .map(|value| {
            value
                .as_str()
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("images must be an array of strings"))
        })
        .collect()
}

fn stall_shipping(value: &Value) -> Result<Vec<ShippingZone>, DomainError> {
    let Some(rows) = value.as_array() else {
        return Err(invalid("stall shipping must be an array"));
    };
    let mut zones = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(zone) = row.as_object() else {
            return Err(invalid("a shipping zone must be an object"));
        };
        let id = required_string(zone, "id")?;
        if zones
            .iter()
            .any(|existing: &ShippingZone| existing.id == id)
        {
            return Err(invalid("shipping zone ids must be unique"));
        }
        let regions = match zone.get("regions") {
            None => Vec::new(),
            Some(value) => {
                let Some(regions) = value.as_array() else {
                    return Err(invalid("shipping regions must be an array of strings"));
                };
                regions
                    .iter()
                    .map(|region| {
                        region
                            .as_str()
                            .filter(|text| !text.is_empty())
                            .map(str::to_owned)
                            .ok_or_else(|| invalid("shipping regions must be an array of strings"))
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        zones.push(ShippingZone {
            id,
            name: optional_string(zone, "name")?,
            cost: required_finite(zone, "cost")?,
            regions,
        });
    }
    Ok(zones)
}

fn product_shipping(object: &Map<String, Value>) -> Result<Vec<ProductShipping>, DomainError> {
    let Some(value) = object.get("shipping") else {
        return Ok(Vec::new());
    };
    let Some(rows) = value.as_array() else {
        return Err(invalid("product shipping must be an array"));
    };
    let mut extras = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(extra) = row.as_object() else {
            return Err(invalid("product shipping must be an array"));
        };
        let id = required_string(extra, "id")?;
        if extras
            .iter()
            .any(|existing: &ProductShipping| existing.id == id)
        {
            return Err(invalid("shipping zone ids must be unique"));
        }
        extras.push(ProductShipping {
            id,
            cost: required_finite(extra, "cost")?,
        });
    }
    Ok(extras)
}

/// Read a kind `30017` stall. The `d` tag must equal the content `id`.
pub fn open_stall(event: &Event) -> Result<Stall, DomainError> {
    if event.kind != STALL_KIND {
        return Err(invalid("a stall has kind 30017"));
    }
    let object = json_object(&event.content)?;
    let id = required_string(&object, "id")?;
    same_id(event, &id)?;
    let Some(shipping) = object.get("shipping") else {
        return Err(invalid("a stall requires a shipping array"));
    };
    Ok(Stall {
        id,
        name: required_string(&object, "name")?,
        description: optional_string(&object, "description")?,
        currency: required_string(&object, "currency")?,
        shipping: stall_shipping(shipping)?,
    })
}

/// Read a kind `30018` product. The `d` tag must equal the content `id`.
pub fn open_product(event: &Event) -> Result<Product, DomainError> {
    if event.kind != PRODUCT_KIND {
        return Err(invalid("a product has kind 30018"));
    }
    let object = json_object(&event.content)?;
    let id = required_string(&object, "id")?;
    same_id(event, &id)?;
    let quantity = match object.get("quantity") {
        None => return Err(invalid("a product requires quantity")),
        Some(value) if value.is_null() => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| invalid("product quantity must be an integer or null"))?,
        ),
    };
    let categories = event
        .tag_values("t")
        .map(|category| {
            if category.is_empty() {
                Err(invalid("a product category tag is empty"))
            } else {
                Ok(category.to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Product {
        id,
        stall_id: required_string(&object, "stall_id")?,
        name: required_string(&object, "name")?,
        description: optional_string(&object, "description")?,
        images: images(&object)?,
        currency: required_string(&object, "currency")?,
        price: required_finite(&object, "price")?,
        quantity,
        specs: specs(&object)?,
        shipping: product_shipping(&object)?,
        categories,
    })
}

/// Read a kind `30019` marketplace page.
pub fn open_marketplace(event: &Event) -> Result<MarketplacePage, DomainError> {
    if event.kind != MARKET_KIND {
        return Err(invalid("a marketplace page has kind 30019"));
    }
    let object = json_object(&event.content)?;
    let ui = match object.get("ui") {
        None => None,
        Some(value) => Some(
            value
                .as_object()
                .ok_or_else(|| invalid("marketplace ui must be an object"))?,
        ),
    };
    let merchants = match object.get("merchants") {
        None => Vec::new(),
        Some(value) => {
            let Some(rows) = value.as_array() else {
                return Err(invalid("merchants must be an array of pubkeys"));
            };
            rows.iter()
                .map(|merchant| {
                    let Some(pubkey) = merchant.as_str() else {
                        return Err(invalid("merchants must be an array of pubkeys"));
                    };
                    decode_lower_hex::<32>(pubkey, "merchant pubkey")
                        .map_err(|_| invalid("merchants must be an array of pubkeys"))?;
                    Ok(pubkey.to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    Ok(MarketplacePage {
        name: optional_string(&object, "name")?,
        about: optional_string(&object, "about")?,
        picture: ui
            .map(|ui| optional_string(ui, "picture"))
            .transpose()?
            .flatten(),
        banner: ui
            .map(|ui| optional_string(ui, "banner"))
            .transpose()?
            .flatten(),
        theme: ui
            .map(|ui| optional_string(ui, "theme"))
            .transpose()?
            .flatten(),
        dark_mode: match ui.and_then(|ui| ui.get("darkMode")) {
            None => None,
            Some(value) => Some(
                value
                    .as_bool()
                    .ok_or_else(|| invalid("darkMode must be a boolean"))?,
            ),
        },
        merchants,
    })
}

/// Read a kind `30020` auction. The `d` tag must equal the content `id`.
pub fn open_auction(event: &Event) -> Result<Auction, DomainError> {
    if event.kind != AUCTION_KIND {
        return Err(invalid("an auction has kind 30020"));
    }
    let object = json_object(&event.content)?;
    let id = required_string(&object, "id")?;
    same_id(event, &id)?;
    let duration = required_u64(&object, "duration")?;
    if duration == 0 {
        return Err(invalid("an auction duration must be greater than zero"));
    }
    Ok(Auction {
        id,
        stall_id: required_string(&object, "stall_id")?,
        name: required_string(&object, "name")?,
        description: optional_string(&object, "description")?,
        images: images(&object)?,
        starting_bid: required_u64(&object, "starting_bid")?,
        start_date: optional_u64(&object, "start_date")?,
        duration,
        specs: specs(&object)?,
        shipping: product_shipping(&object)?,
    })
}

/// Read a kind `1021` bid.
pub fn open_bid(event: &Event) -> Result<Bid, DomainError> {
    if event.kind != BID_KIND {
        return Err(invalid("a bid has kind 1021"));
    }
    if event.content.is_empty()
        || !event.content.bytes().all(|byte| byte.is_ascii_digit())
        || event.content.starts_with('0') && event.content.len() > 1
    {
        return Err(invalid("a bid content must be a non-negative integer"));
    }
    let amount: u64 = event
        .content
        .parse()
        .map_err(|_| invalid("a bid content must be a non-negative integer"))?;
    let mut targets = event.tag_values("e");
    let Some(auction_event_id) = targets.next() else {
        return Err(invalid("a bid requires one e tag"));
    };
    if targets.next().is_some() || decode_lower_hex::<32>(auction_event_id, "auction id").is_err() {
        return Err(invalid("a bid requires one e tag"));
    }
    Ok(Bid {
        amount,
        auction_event_id: auction_event_id.to_owned(),
    })
}

/// Read a kind `1022` bid confirmation.
///
/// The first `e` tag is the bid. The second `e` tag is the auction version.
pub fn open_bid_confirmation(event: &Event) -> Result<BidConfirmation, DomainError> {
    if event.kind != BID_CONFIRMATION_KIND {
        return Err(invalid("a bid confirmation has kind 1022"));
    }
    let object = json_object(&event.content)?;
    let status = match object.get("status").and_then(Value::as_str) {
        Some("accepted") => BidStatus::Accepted,
        Some("rejected") => BidStatus::Rejected,
        Some("pending") => BidStatus::Pending,
        Some("winner") => BidStatus::Winner,
        _ => {
            return Err(invalid(
                "a bid confirmation status is not one of the four named values",
            ));
        }
    };
    let mut targets = event.tag_values("e");
    let Some(bid_event_id) = targets.next() else {
        return Err(invalid(
            "a bid confirmation requires the bid and the auction",
        ));
    };
    let Some(auction_event_id) = targets.next() else {
        return Err(invalid(
            "a bid confirmation requires the bid and the auction",
        ));
    };
    if targets.next().is_some()
        || decode_lower_hex::<32>(bid_event_id, "bid id").is_err()
        || decode_lower_hex::<32>(auction_event_id, "auction id").is_err()
    {
        return Err(invalid(
            "a bid confirmation requires the bid and the auction",
        ));
    }
    Ok(BidConfirmation {
        status,
        message: optional_string(&object, "message")?,
        duration_extended: optional_u64(&object, "duration_extended")?,
        bid_event_id: bid_event_id.to_owned(),
        auction_event_id: auction_event_id.to_owned(),
    })
}

/// Check the marketplace kinds a relay can read without decrypting.
pub fn validate_marketplace(event: &Event) -> Result<(), DomainError> {
    match event.kind {
        BID_KIND => {
            open_bid(event)?;
        }
        BID_CONFIRMATION_KIND => {
            open_bid_confirmation(event)?;
        }
        STALL_KIND => {
            open_stall(event)?;
        }
        PRODUCT_KIND => {
            open_product(event)?;
        }
        MARKET_KIND => {
            open_marketplace(event)?;
        }
        AUCTION_KIND => {
            open_auction(event)?;
        }
        _ => return Err(invalid("not a marketplace event")),
    }
    Ok(())
}

/// Read a decrypted checkout body.
pub fn open_checkout(plaintext: &str) -> Result<Checkout, DomainError> {
    let object = json_object(plaintext)?;
    let message_type = object
        .get("type")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("a checkout message requires a type"))?;
    match message_type {
        0 => Ok(Checkout::Order(open_order(&object)?)),
        1 => Ok(Checkout::Payment(open_payment(&object)?)),
        2 => Ok(Checkout::Status(open_status(&object)?)),
        _ => Err(invalid("a checkout type must be 0, 1, or 2")),
    }
}

fn open_order(object: &Map<String, Value>) -> Result<Order, DomainError> {
    let Some(rows) = object.get("items").and_then(Value::as_array) else {
        return Err(invalid("an order requires items"));
    };
    if rows.is_empty() {
        return Err(invalid("an order requires items"));
    }
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(item) = row.as_object() else {
            return Err(invalid("an order item must be an object"));
        };
        let quantity = required_u64(item, "quantity")?;
        if quantity == 0 {
            return Err(invalid("an order item quantity must be greater than zero"));
        }
        items.push(OrderItem {
            product_id: required_string(item, "product_id")?,
            quantity,
        });
    }
    let (contact_nostr, contact_phone, contact_email) = match object.get("contact") {
        None | Some(Value::Null) => (None, None, None),
        Some(value) => {
            let Some(contact) = value.as_object() else {
                return Err(invalid("order contact must be an object"));
            };
            if let Some(pubkey) = optional_string(contact, "nostr")? {
                decode_lower_hex::<32>(&pubkey, "contact pubkey")
                    .map_err(|_| invalid("order contact nostr must be a pubkey"))?;
            }
            (
                optional_string(contact, "nostr")?,
                optional_string(contact, "phone")?,
                optional_string(contact, "email")?,
            )
        }
    };
    Ok(Order {
        id: required_string(object, "id")?,
        name: optional_string(object, "name")?,
        address: optional_string(object, "address")?,
        message: optional_string(object, "message")?,
        contact_nostr,
        contact_phone,
        contact_email,
        items,
        shipping_id: required_string(object, "shipping_id")?,
    })
}

fn open_payment(object: &Map<String, Value>) -> Result<PaymentRequest, DomainError> {
    let Some(rows) = object.get("payment_options").and_then(Value::as_array) else {
        return Err(invalid("a payment request requires payment options"));
    };
    if rows.is_empty() {
        return Err(invalid("a payment request requires payment options"));
    }
    let mut payment_options = Vec::with_capacity(rows.len());
    for row in rows {
        let Some(option) = row.as_object() else {
            return Err(invalid("a payment option must be an object"));
        };
        payment_options.push(PaymentOption {
            method: required_string(option, "type")?,
            link: required_string(option, "link")?,
        });
    }
    Ok(PaymentRequest {
        id: required_string(object, "id")?,
        message: optional_string(object, "message")?,
        payment_options,
    })
}

fn open_status(object: &Map<String, Value>) -> Result<OrderStatus, DomainError> {
    let paid = object
        .get("paid")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("order status requires paid"))?;
    let shipped = object
        .get("shipped")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("order status requires shipped"))?;
    Ok(OrderStatus {
        id: required_string(object, "id")?,
        message: required_string(object, "message")?,
        paid,
        shipped,
    })
}

/// Whether `zone_id` is one of the stall's shipping zones.
pub fn shipping_zone<'a>(stall: &'a Stall, zone_id: &str) -> Option<&'a ShippingZone> {
    stall.shipping.iter().find(|zone| zone.id == zone_id)
}

/// Extra cost a product adds for `zone_id`, or zero when it names none.
pub fn product_shipping_extra(product: &Product, zone_id: &str) -> Option<f64> {
    product
        .shipping
        .iter()
        .find(|extra| extra.id == zone_id)
        .map(|extra| extra.cost)
}

/// Base zone cost plus the product extra multiplied by the number of units.
///
/// A quantity above `u32::MAX` is refused. `f64` cannot represent every
/// larger integer, and a shipping total has to be exact for the units the
/// customer asked for.
pub fn shipping_cost(base: f64, extra_per_unit: f64, units: u64) -> Result<f64, DomainError> {
    if !base.is_finite() || !extra_per_unit.is_finite() || base < 0.0 || extra_per_unit < 0.0 {
        return Err(invalid(
            "shipping cost must be a non-negative finite number",
        ));
    }
    let units =
        u32::try_from(units).map_err(|_| invalid("product quantity is too large to price"))?;
    let total = base + extra_per_unit * f64::from(units);
    if !total.is_finite() {
        return Err(invalid("shipping cost overflow"));
    }
    Ok(total)
}

/// Auction end, in unix seconds, after every confirmation extension.
///
/// The pinned formula is `start_date + duration + sum(duration_extended)`.
/// An auction with no start date has no end yet.
pub fn auction_end(auction: &Auction, extensions: &[u64]) -> Option<u64> {
    let mut end = auction.start_date?.checked_add(auction.duration)?;
    for extra in extensions {
        end = end.checked_add(*extra)?;
    }
    Some(end)
}

/// Whether this confirmation accepts `bid` for this auction version.
///
/// The merchant who signed the auction must sign the confirmation. A bid
/// names one auction event id, so a later edit of the auction does not
/// keep that bid.
pub fn bid_confirmation_matches(
    confirmation: &Event,
    bid: &Event,
    auction: &Event,
) -> Result<BidStatus, DomainError> {
    let parsed_confirmation = open_bid_confirmation(confirmation)?;
    let parsed_bid = open_bid(bid)?;
    let _auction = open_auction(auction)?;
    if confirmation.pubkey != auction.pubkey
        || parsed_confirmation.bid_event_id != bid.id
        || parsed_confirmation.auction_event_id != auction.id
        || parsed_bid.auction_event_id != auction.id
    {
        return Err(invalid(
            "the bid confirmation does not match this auction version",
        ));
    }
    Ok(parsed_confirmation.status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn merchant() -> RelaySigner {
        RelaySigner::from_secret_hex(&"31".repeat(32)).unwrap()
    }

    fn customer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"32".repeat(32)).unwrap()
    }

    #[test]
    fn a_stall_product_and_bid_follow_the_pinned_marketplace_events() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/15.md"
        ))
        .unwrap();
        assert!(text.contains("unrecommended"));
        assert!(text.contains("30017"));
        assert!(text.contains("1021"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "15.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "15.md")
        );

        let merchant = merchant();
        let stall_body = r#"{
            "id": "stall-1",
            "name": "North",
            "description": "wool",
            "currency": "SAT",
            "shipping": [{
                "id": "eu",
                "name": "Europe",
                "cost": 2.5,
                "regions": ["EU"]
            }]
        }"#;
        let stall_event = merchant.sign(
            1_700_000_000,
            STALL_KIND,
            vec![Tag::new(vec!["d".into(), "stall-1".into()])],
            stall_body.into(),
        );
        stall_event.validate_structure().unwrap();
        assert_eq!(stall_event.class(), EventClass::Addressable);
        let stall = open_stall(&stall_event).unwrap();
        assert_eq!(stall.shipping[0].cost, 2.5);
        assert!(shipping_zone(&stall, "eu").is_some());

        let replaced = merchant.sign(
            1_700_000_100,
            STALL_KIND,
            vec![Tag::new(vec!["d".into(), "stall-1".into()])],
            stall_body.replace("North", "South"),
        );
        assert_eq!(
            compare_replacement(&stall_event, &replaced).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let mismatched = merchant.sign(
            1_700_000_000,
            STALL_KIND,
            vec![Tag::new(vec!["d".into(), "other".into()])],
            stall_body.into(),
        );
        assert!(mismatched.validate_structure().is_err());

        let product_body = r#"{
            "id": "prod-1",
            "stall_id": "stall-1",
            "name": "Hat",
            "currency": "SAT",
            "price": 10.0,
            "quantity": null,
            "specs": [["color", "amber"]],
            "shipping": [{"id": "eu", "cost": 1.0}]
        }"#;
        let product_event = merchant.sign(
            1_700_000_000,
            PRODUCT_KIND,
            vec![
                Tag::new(vec!["d".into(), "prod-1".into()]),
                Tag::new(vec!["t".into(), "wool".into()]),
            ],
            product_body.into(),
        );
        let product = open_product(&product_event).unwrap();
        assert_eq!(product.quantity, None);
        assert_eq!(product.categories, vec!["wool".to_owned()]);
        assert_eq!(
            shipping_cost(2.5, product_shipping_extra(&product, "eu").unwrap(), 3).unwrap(),
            5.5
        );

        let auction_body = r#"{
            "id": "auc-1",
            "stall_id": "stall-1",
            "name": "Vase",
            "starting_bid": 100,
            "start_date": 1700000000,
            "duration": 3600
        }"#;
        let auction_event = merchant.sign(
            1_700_000_000,
            AUCTION_KIND,
            vec![Tag::new(vec!["d".into(), "auc-1".into()])],
            auction_body.into(),
        );
        auction_event.validate_structure().unwrap();
        assert_eq!(auction_event.class(), EventClass::Addressable);
        let auction = open_auction(&auction_event).unwrap();
        assert_eq!(
            auction_end(&auction, &[120]).unwrap(),
            1_700_000_000 + 3600 + 120
        );

        let bid_event = customer().sign(
            1_700_000_010,
            BID_KIND,
            vec![Tag::new(vec!["e".into(), auction_event.id.clone()])],
            "250".into(),
        );
        bid_event.validate_structure().unwrap();
        assert_eq!(bid_event.class(), EventClass::Regular);
        assert_eq!(open_bid(&bid_event).unwrap().amount, 250);
        assert!(
            customer()
                .sign(1_700_000_010, BID_KIND, Vec::new(), "250".into())
                .validate_structure()
                .is_err()
        );

        let confirmation = merchant.sign(
            1_700_000_020,
            BID_CONFIRMATION_KIND,
            vec![
                Tag::new(vec!["e".into(), bid_event.id.clone()]),
                Tag::new(vec!["e".into(), auction_event.id.clone()]),
            ],
            r#"{"status":"winner","duration_extended":120}"#.into(),
        );
        assert_eq!(
            bid_confirmation_matches(&confirmation, &bid_event, &auction_event).unwrap(),
            BidStatus::Winner
        );
        let stranger = customer().sign(
            1_700_000_020,
            BID_CONFIRMATION_KIND,
            vec![
                Tag::new(vec!["e".into(), bid_event.id.clone()]),
                Tag::new(vec!["e".into(), auction_event.id.clone()]),
            ],
            r#"{"status":"winner"}"#.into(),
        );
        assert!(bid_confirmation_matches(&stranger, &bid_event, &auction_event).is_err());

        let edited = merchant.sign(
            1_700_000_200,
            AUCTION_KIND,
            vec![Tag::new(vec!["d".into(), "auc-1".into()])],
            auction_body.replace("Vase", "Bowl"),
        );
        assert_ne!(edited.id, auction_event.id);
        assert!(bid_confirmation_matches(&confirmation, &bid_event, &edited).is_err());

        let page = merchant.sign(
            1_700_000_000,
            MARKET_KIND,
            vec![Tag::new(vec!["d".into(), "market".into()])],
            format!(
                r#"{{"name":"Amber","ui":{{"darkMode":true}},"merchants":["{}"]}}"#,
                merchant.pubkey()
            ),
        );
        let page = open_marketplace(&page).unwrap();
        assert_eq!(page.dark_mode, Some(true));
        assert_eq!(page.merchants, vec![merchant.pubkey().to_owned()]);

        let order = open_checkout(
            r#"{
                "id": "order-1",
                "type": 0,
                "items": [{"product_id": "prod-1", "quantity": 2}],
                "shipping_id": "eu",
                "contact": {"nostr": "3333333333333333333333333333333333333333333333333333333333333333"}
            }"#,
        )
        .unwrap();
        let Checkout::Order(order) = order else {
            panic!("type 0 is an order");
        };
        assert_eq!(order.items[0].quantity, 2);
        assert_eq!(order.shipping_id, "eu");

        let payment = open_checkout(
            r#"{
                "id": "order-1",
                "type": 1,
                "payment_options": [{"type": "ln", "link": "lnbc1"}]
            }"#,
        )
        .unwrap();
        let Checkout::Payment(payment) = payment else {
            panic!("type 1 is a payment request");
        };
        assert_eq!(payment.payment_options[0].method, "ln");

        let status = open_checkout(
            r#"{"id":"order-1","type":2,"message":"on the way","paid":true,"shipped":true}"#,
        )
        .unwrap();
        assert!(matches!(
            status,
            Checkout::Status(OrderStatus {
                paid: true,
                shipped: true,
                ..
            })
        ));
        assert!(open_checkout(r#"{"id":"order-1","type":9}"#).is_err());
    }
}
