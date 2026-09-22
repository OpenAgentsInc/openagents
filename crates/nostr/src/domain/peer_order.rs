//! NIP-69 peer-to-peer orders.
//!
//! Kind `38383` is an addressable order. `d` is the order id. `k` is
//! `buy` or `sell`. `f` is a three-letter currency code. `amt` is the
//! bitcoin amount in satoshis, and `0` means the taker learns the amount
//! later. `fa` is one fiat amount or a minimum and a maximum. `z` is
//! `order`.
//!
//! The relay does not fetch a bitcoin price, visit the source URL, or
//! settle the trade. A three-letter currency code is not looked up in
//! ISO 4217. NIP-69 is a draft, so this kind stays off the NIP-11 list.

use super::{DomainError, Event};

const ORDER_KIND: u16 = 38_383;
const GEOHASH: &[u8] = b"0123456789bcdefghjkmnpqrstuvwxyz";
const SCALE: i128 = 100_000_000;

/// Whether the maker is buying or selling bitcoin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// The status of a peer-to-peer order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerOrderStatus {
    Pending,
    Canceled,
    InProgress,
    Success,
    Expired,
}

/// The maker rating carried on an order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MakerRating {
    pub total_reviews: u64,
    pub total_rating: String,
    pub last_rating: String,
    pub max_rate: String,
    pub min_rate: String,
}

/// A kind `38383` peer-to-peer order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerOrder {
    pub id: String,
    pub side: OrderSide,
    pub currency: String,
    pub status: PeerOrderStatus,
    pub amount_sats: u64,
    pub fiat_min: String,
    pub fiat_max: String,
    pub methods: Vec<String>,
    pub premium: String,
    pub source: Option<String>,
    pub rating: Option<MakerRating>,
    pub network: String,
    pub layer: String,
    pub name: Option<String>,
    pub geohash: Option<String>,
    pub bond_sats: Option<u64>,
    pub expires_at: u64,
    pub expiration: u64,
    pub platform: String,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn whole(value: &str, reason: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    value.parse().map_err(|_| invalid(reason))
}

fn scaled(value: &str, allow_negative: bool) -> Result<(String, i128), DomainError> {
    let reason = "an order amount is a decimal number";
    let (negative, rest) = match value.strip_prefix('-') {
        Some(rest) if allow_negative => (true, rest),
        Some(_) => return Err(invalid(reason)),
        None => (false, value),
    };
    let (whole_part, fraction) = match rest.split_once('.') {
        Some((whole_part, fraction)) => (whole_part, Some(fraction)),
        None => (rest, None),
    };
    if whole_part.is_empty()
        || (whole_part.len() > 1 && whole_part.starts_with('0'))
        || !whole_part.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    let fraction = fraction.unwrap_or("");
    if fraction.len() > 8 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid(reason));
    }
    let whole_part: i128 = whole_part.parse().map_err(|_| invalid(reason))?;
    let mut frac: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| invalid(reason))?
    };
    for _ in fraction.len()..8 {
        frac *= 10;
    }
    let mut scale = whole_part * SCALE + frac;
    if negative {
        if scale == 0 {
            return Err(invalid(reason));
        }
        scale = -scale;
    }
    Ok((value.to_owned(), scale))
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn token(value: &str) -> bool {
    (1..=16).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn side(value: &str) -> Result<OrderSide, DomainError> {
    match value {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(invalid("an order type is buy or sell")),
    }
}

fn status(value: &str) -> Result<PeerOrderStatus, DomainError> {
    match value {
        "pending" => Ok(PeerOrderStatus::Pending),
        "canceled" => Ok(PeerOrderStatus::Canceled),
        "in-progress" => Ok(PeerOrderStatus::InProgress),
        "success" => Ok(PeerOrderStatus::Success),
        "expired" => Ok(PeerOrderStatus::Expired),
        _ => Err(invalid(
            "an order status is pending, canceled, in-progress, success, or expired",
        )),
    }
}

fn payment_methods(tag: &super::Tag) -> Result<Vec<String>, DomainError> {
    let parts = tag.as_slice();
    if parts.len() < 2 {
        return Err(invalid("an order lists a payment method"));
    }
    let mut methods = Vec::new();
    for part in parts.iter().skip(1) {
        for method in part.split(',') {
            let method = method.trim();
            if method.is_empty()
                || method.len() > 64
                || method.chars().any(char::is_control)
                || methods.iter().any(|seen| seen == method)
            {
                return Err(invalid("an order lists a payment method"));
            }
            methods.push(method.to_owned());
        }
    }
    if methods.is_empty() {
        return Err(invalid("an order lists a payment method"));
    }
    Ok(methods)
}

fn rating(value: &str) -> Result<MakerRating, DomainError> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|_| invalid("an order rating is a JSON object"))?;
    let Some(fields) = parsed.as_object() else {
        return Err(invalid("an order rating is a JSON object"));
    };
    let reason = "an order rating is a JSON object";
    let reviews = fields
        .get("total_reviews")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invalid(reason))?;
    let (total_rating, _) = rate(fields.get("total_rating"))?;
    let (last_rating, last) = rate(fields.get("last_rating"))?;
    let (max_rate, max) = rate(fields.get("max_rate"))?;
    let (min_rate, min) = rate(fields.get("min_rate"))?;
    if min > max || last < min || last > max {
        return Err(invalid("an order rating stays inside its scale"));
    }
    Ok(MakerRating {
        total_reviews: reviews,
        total_rating,
        last_rating,
        max_rate,
        min_rate,
    })
}

fn rate(value: Option<&serde_json::Value>) -> Result<(String, i128), DomainError> {
    let Some(number) = value.and_then(serde_json::Value::as_number) else {
        return Err(invalid("an order rating is a JSON object"));
    };
    scaled(&number.to_string(), false)
}

fn plain(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn one(tag: &super::Tag) -> Result<&str, DomainError> {
    if tag.as_slice().len() != 2 {
        return Err(invalid("an order tag has one value"));
    }
    tag.value()
        .ok_or_else(|| invalid("an order tag has one value"))
}

/// Read a kind `38383` peer-to-peer order.
pub fn open_peer_order(event: &Event) -> Result<PeerOrder, DomainError> {
    if event.kind != ORDER_KIND {
        return Err(invalid("a peer order has kind 38383"));
    }
    if event.content.len() > 2_048 || event.content.chars().any(char::is_control) {
        return Err(invalid("an order comment is plain text"));
    }
    let mut id = None;
    let mut side_name = None;
    let mut currency = None;
    let mut status_name = None;
    let mut amount_sats = None;
    let mut fiat = None;
    let mut methods = None;
    let mut premium = None;
    let mut source = None;
    let mut rating_value = None;
    let mut network = None;
    let mut layer = None;
    let mut name = None;
    let mut geohash = None;
    let mut bond_sats = None;
    let mut expires_at = None;
    let mut expiration = None;
    let mut platform = None;
    let mut document = false;
    for tag in &event.tags {
        match tag.name() {
            Some("d") => {
                if id.is_some() {
                    return Err(invalid("an order has one identifier"));
                }
                let value = one(tag)?;
                if !plain(value, 128) || value.chars().any(char::is_whitespace) {
                    return Err(invalid("an order identifier is 1 to 128 characters"));
                }
                id = Some(value.to_owned());
            }
            Some("k") => {
                if side_name.is_some() {
                    return Err(invalid("an order type is buy or sell"));
                }
                side_name = Some(side(one(tag)?)?);
            }
            Some("f") => {
                if currency.is_some() {
                    return Err(invalid("an order currency is three letters"));
                }
                let value = one(tag)?;
                if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
                    return Err(invalid("an order currency is three letters"));
                }
                currency = Some(value.to_owned());
            }
            Some("s") => {
                if status_name.is_some() {
                    return Err(invalid("an order has one status"));
                }
                status_name = Some(status(one(tag)?)?);
            }
            Some("amt") => {
                if amount_sats.is_some() {
                    return Err(invalid("an order amount is satoshis"));
                }
                amount_sats = Some(whole(one(tag)?, "an order amount is satoshis")?);
            }
            Some("fa") => {
                if fiat.is_some() {
                    return Err(invalid("an order has one fiat amount"));
                }
                let parts = tag.as_slice();
                if !(2..=3).contains(&parts.len()) {
                    return Err(invalid("an order has one fiat amount"));
                }
                let (min_text, min) = scaled(&parts[1], false)?;
                if min == 0 {
                    return Err(invalid("an order fiat amount is positive"));
                }
                let (max_text, max) = if parts.len() == 3 {
                    scaled(&parts[2], false)?
                } else {
                    (min_text.clone(), min)
                };
                if max < min {
                    return Err(invalid("an order fiat maximum is at least the minimum"));
                }
                fiat = Some((min_text, max_text));
            }
            Some("pm") => {
                if methods.is_some() {
                    return Err(invalid("an order lists a payment method"));
                }
                methods = Some(payment_methods(tag)?);
            }
            Some("premium") => {
                if premium.is_some() {
                    return Err(invalid("an order has one premium"));
                }
                premium = Some(scaled(one(tag)?, true)?.0);
            }
            Some("source") => {
                if source.is_some() {
                    return Err(invalid("an order source is an http URL"));
                }
                let value = one(tag)?;
                if !http_url(value) {
                    return Err(invalid("an order source is an http URL"));
                }
                source = Some(value.to_owned());
            }
            Some("rating") => {
                if rating_value.is_some() {
                    return Err(invalid("an order has one rating"));
                }
                rating_value = Some(rating(one(tag)?)?);
            }
            Some("network") => {
                if network.is_some() {
                    return Err(invalid("an order network is a short lowercase name"));
                }
                let value = one(tag)?;
                if !token(value) {
                    return Err(invalid("an order network is a short lowercase name"));
                }
                network = Some(value.to_owned());
            }
            Some("layer") => {
                if layer.is_some() {
                    return Err(invalid("an order layer is a short lowercase name"));
                }
                let value = one(tag)?;
                if !token(value) {
                    return Err(invalid("an order layer is a short lowercase name"));
                }
                layer = Some(value.to_owned());
            }
            Some("name") => {
                if name.is_some() {
                    return Err(invalid("an order has one maker name"));
                }
                let value = one(tag)?;
                if !plain(value, 64) {
                    return Err(invalid("an order has one maker name"));
                }
                name = Some(value.to_owned());
            }
            Some("g") => {
                if geohash.is_some() {
                    return Err(invalid("an order geohash is 1 to 12 characters"));
                }
                let value = one(tag)?;
                if !(1..=12).contains(&value.len())
                    || !value.bytes().all(|byte| GEOHASH.contains(&byte))
                {
                    return Err(invalid("an order geohash is 1 to 12 characters"));
                }
                geohash = Some(value.to_owned());
            }
            Some("bond") => {
                if bond_sats.is_some() {
                    return Err(invalid("an order bond is satoshis"));
                }
                bond_sats = Some(whole(one(tag)?, "an order bond is satoshis")?);
            }
            Some("expires_at") => {
                if expires_at.is_some() {
                    return Err(invalid("an order has one pending deadline"));
                }
                expires_at = Some(whole(one(tag)?, "an order deadline is unix seconds")?);
            }
            Some("expiration") => {
                if expiration.is_some() {
                    return Err(invalid("an order has one expiration"));
                }
                expiration = Some(whole(one(tag)?, "an order deadline is unix seconds")?);
            }
            Some("y") => {
                if platform.is_some() {
                    return Err(invalid("an order names one platform"));
                }
                let value = one(tag)?;
                if !plain(value, 64) || value.chars().any(char::is_whitespace) {
                    return Err(invalid("an order names one platform"));
                }
                platform = Some(value.to_owned());
            }
            Some("z") => {
                if document || one(tag)? != "order" {
                    return Err(invalid("an order document is order"));
                }
                document = true;
            }
            _ => {}
        }
    }
    let Some(id) = id else {
        return Err(invalid("an order has one identifier"));
    };
    let Some(side) = side_name else {
        return Err(invalid("an order type is buy or sell"));
    };
    let Some(currency) = currency else {
        return Err(invalid("an order currency is three letters"));
    };
    let Some(status) = status_name else {
        return Err(invalid("an order has one status"));
    };
    let Some(amount_sats) = amount_sats else {
        return Err(invalid("an order amount is satoshis"));
    };
    let Some((fiat_min, fiat_max)) = fiat else {
        return Err(invalid("an order has one fiat amount"));
    };
    let Some(methods) = methods else {
        return Err(invalid("an order lists a payment method"));
    };
    let Some(premium) = premium else {
        return Err(invalid("an order has one premium"));
    };
    let Some(network) = network else {
        return Err(invalid("an order network is a short lowercase name"));
    };
    let Some(layer) = layer else {
        return Err(invalid("an order layer is a short lowercase name"));
    };
    let Some(expires_at) = expires_at else {
        return Err(invalid("an order has one pending deadline"));
    };
    let Some(expiration) = expiration else {
        return Err(invalid("an order has one expiration"));
    };
    let Some(platform) = platform else {
        return Err(invalid("an order names one platform"));
    };
    if !document {
        return Err(invalid("an order document is order"));
    }
    if expires_at <= event.created_at || expiration <= expires_at {
        return Err(invalid("an order expires after its pending deadline"));
    }
    Ok(PeerOrder {
        id,
        side,
        currency,
        status,
        amount_sats,
        fiat_min,
        fiat_max,
        methods,
        premium,
        source,
        rating: rating_value,
        network,
        layer,
        name,
        geohash,
        bond_sats,
        expires_at,
        expiration,
        platform,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"69".repeat(32)).unwrap()
    }

    fn order_tags(id: &str, status: &str, fiat: &[&str]) -> Vec<Tag> {
        let mut tags = vec![
            Tag::new(vec!["d".into(), id.into()]),
            Tag::new(vec!["k".into(), "sell".into()]),
            Tag::new(vec!["f".into(), "VES".into()]),
            Tag::new(vec!["s".into(), status.into()]),
            Tag::new(vec!["amt".into(), "0".into()]),
        ];
        let mut fiat_tag = vec!["fa".into()];
        fiat_tag.extend(fiat.iter().map(|value| (*value).to_owned()));
        tags.push(Tag::new(fiat_tag));
        tags.extend([
            Tag::new(vec![
                "pm".into(),
                "face to face".into(),
                "bank transfer".into(),
            ]),
            Tag::new(vec!["premium".into(), "1".into()]),
            Tag::new(vec![
                "rating".into(),
                r#"{"total_reviews":1,"total_rating":3.0,"last_rating":3,"max_rate":5,"min_rate":1}"#
                    .into(),
            ]),
            Tag::new(vec![
                "source".into(),
                "https://t.me/p2plightning/order".into(),
            ]),
            Tag::new(vec!["network".into(), "mainnet".into()]),
            Tag::new(vec!["layer".into(), "lightning".into()]),
            Tag::new(vec!["name".into(), "Nakamoto".into()]),
            Tag::new(vec!["g".into(), "ww8p1r4t8".into()]),
            Tag::new(vec!["bond".into(), "0".into()]),
            Tag::new(vec!["expires_at".into(), "1700086400".into()]),
            Tag::new(vec!["expiration".into(), "1700172800".into()]),
            Tag::new(vec!["y".into(), "lnp2pbot".into()]),
            Tag::new(vec!["z".into(), "order".into()]),
        ]);
        tags
    }

    #[test]
    fn a_pending_sell_replaces_when_it_succeeds() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/69.md"
        ))
        .unwrap();
        assert!(text.contains("38383"));
        assert!(text.contains("sell"));
        assert!(text.contains("\"z\", \"order\""));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "69.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "69.md")
        );

        let maker = signer();
        let id = "ede61c96-4c13-4519-bf3a-dcf7f1e9d842";
        let pending = maker.sign(
            1_700_000_000,
            ORDER_KIND,
            order_tags(id, "pending", &["100"]),
            String::new(),
        );
        pending.validate_structure().unwrap();
        assert_eq!(pending.class(), EventClass::Addressable);
        let opened = open_peer_order(&pending).unwrap();
        assert_eq!(opened.side, OrderSide::Sell);
        assert_eq!(opened.currency, "VES");
        assert_eq!(opened.status, PeerOrderStatus::Pending);
        assert_eq!(opened.amount_sats, 0);
        assert_eq!(opened.fiat_min, "100");
        assert_eq!(opened.fiat_max, "100");
        assert_eq!(
            opened.methods,
            ["face to face".to_owned(), "bank transfer".to_owned()]
        );
        assert_eq!(opened.premium, "1");
        assert_eq!(opened.rating.unwrap().total_reviews, 1);
        assert_eq!(opened.network, "mainnet");
        assert_eq!(opened.layer, "lightning");
        assert_eq!(opened.platform, "lnp2pbot");
        assert_eq!(opened.bond_sats, Some(0));

        let done = maker.sign(
            1_700_000_100,
            ORDER_KIND,
            order_tags(id, "success", &["100", "500"]),
            String::new(),
        );
        done.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&pending, &done),
            Ok(ReplacementDecision::ReplaceCurrent)
        );
        let ranged = open_peer_order(&done).unwrap();
        assert_eq!(ranged.status, PeerOrderStatus::Success);
        assert_eq!(ranged.fiat_min, "100");
        assert_eq!(ranged.fiat_max, "500");

        let mut broken = order_tags(id, "pending", &["100"]);
        broken.retain(|tag| tag.name() != Some("k"));
        let missing = maker.sign(1_700_000_200, ORDER_KIND, broken, String::new());
        assert!(missing.validate_structure().is_err());
    }
}
