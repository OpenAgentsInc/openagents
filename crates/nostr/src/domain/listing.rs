//! NIP-99 classified listings.
//!
//! Kind `30402` is an addressable listing. Kind `30403` is the same
//! shape saved as a draft, so it does not replace the published listing.
//! The description is Markdown. `title`, `summary`, `published_at`,
//! `location`, and `price` are the structured tags. `price` is an amount,
//! a three-letter currency, and an optional frequency. `status` is
//! `active` or `sold`.
//!
//! Images use the NIP-58 `image` tag. `nostr:` references in the
//! description stay text. The relay does not fetch images or rewrite the
//! description. Neither kind is added to the NIP-11 list.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress};

const LISTING_KIND: u16 = 30_402;
const DRAFT_KIND: u16 = 30_403;

/// Whether a listing is still offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListingStatus {
    Active,
    Sold,
}

/// A price: amount, currency, and an optional recurring frequency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Price {
    pub amount: String,
    pub currency: String,
    pub frequency: Option<String>,
}

/// One image and its optional pixel size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListingImage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A classified listing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub identifier: String,
    pub draft: bool,
    pub content: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub published_at: Option<u64>,
    pub location: Option<String>,
    pub price: Option<Price>,
    pub status: Option<ListingStatus>,
    pub topics: Vec<String>,
    pub images: Vec<ListingImage>,
    pub geohash: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn optional_relay(value: &str) -> Result<Option<String>, DomainError> {
    if value.is_empty() {
        return Ok(None);
    }
    if is_relay(value) {
        return Ok(Some(value.to_owned()));
    }
    Err(invalid("a listing relay hint must be ws:// or wss://"))
}

fn dimensions(value: &str) -> Result<(u32, u32), DomainError> {
    let Some((width, height)) = value.split_once('x') else {
        return Err(invalid("listing image dimensions are widthxheight"));
    };
    let parsed_width = width
        .parse::<u32>()
        .map_err(|_| invalid("listing image dimensions are widthxheight"))?;
    let parsed_height = height
        .parse::<u32>()
        .map_err(|_| invalid("listing image dimensions are widthxheight"))?;
    if parsed_width == 0
        || parsed_height == 0
        || width != parsed_width.to_string()
        || height != parsed_height.to_string()
    {
        return Err(invalid("listing image dimensions are widthxheight"));
    }
    Ok((parsed_width, parsed_height))
}

fn amount_ok(value: &str) -> bool {
    let Some((whole, fraction)) = value.split_once('.') else {
        return integer_ok(value);
    };
    integer_ok(whole) && !fraction.is_empty() && fraction.bytes().all(|byte| byte.is_ascii_digit())
}

fn integer_ok(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 24
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn one<'a>(event: &'a Event, name: &str, reason: &str) -> Result<Option<&'a str>, DomainError> {
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect::<Vec<_>>();
    if tags.len() > 1 {
        return Err(invalid(reason));
    }
    match tags.first() {
        None => Ok(None),
        Some(tag) => match tag.value() {
            Some(value) if !value.is_empty() => Ok(Some(value)),
            _ => Err(invalid(reason)),
        },
    }
}

/// Read a kind `30402` listing or a kind `30403` draft.
///
/// # Errors
///
/// Returns a sentence when the identifier or a structured tag is refused.
pub fn open_listing(event: &Event) -> Result<Listing, DomainError> {
    let draft = match event.kind {
        LISTING_KIND => false,
        DRAFT_KIND => true,
        _ => return Err(invalid("a classified listing has kind 30402 or 30403")),
    };
    let identifiers = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect::<Vec<_>>();
    if identifiers.len() != 1 {
        return Err(invalid("a classified listing has one d tag"));
    }
    let Some(identifier) = identifiers[0].value() else {
        return Err(invalid("a classified listing has one d tag"));
    };
    if identifier.is_empty()
        || identifier.len() > 256
        || identifier.chars().any(char::is_whitespace)
    {
        return Err(invalid(
            "a listing identifier contains 1 to 256 characters and no whitespace",
        ));
    }
    let title = one(event, "title", "a listing title is one non-empty value")?.map(str::to_owned);
    let summary =
        one(event, "summary", "a listing summary is one non-empty value")?.map(str::to_owned);
    let published_at = match one(
        event,
        "published_at",
        "a listing published_at is one unix timestamp",
    )? {
        None => None,
        Some(value) => {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| invalid("a listing published_at is one unix timestamp"))?;
            if parsed.to_string() != value {
                return Err(invalid("a listing published_at is one unix timestamp"));
            }
            Some(parsed)
        }
    };
    let location = one(
        event,
        "location",
        "a listing location is one non-empty value",
    )?
    .map(str::to_owned);
    let price = match event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("price"))
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] => None,
        [tag] => Some(price_of(tag.as_slice())?),
        _ => return Err(invalid("a listing has one price")),
    };
    let status = match one(event, "status", "a listing status is active or sold")? {
        None => None,
        Some("active") => Some(ListingStatus::Active),
        Some("sold") => Some(ListingStatus::Sold),
        Some(_) => return Err(invalid("a listing status is active or sold")),
    };
    let mut topics = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("t")) {
        let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
            return Err(invalid("a listing topic is non-empty"));
        };
        topics.push(value.to_owned());
    }
    let mut images = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("image")) {
        let values = tag.as_slice();
        if values.len() < 2 || values.len() > 3 || !valid_http_url(&values[1]) {
            return Err(invalid("a listing image is an http:// or https:// URL"));
        }
        let (width, height) = match values.get(2) {
            None => (None, None),
            Some(value) => {
                let (width, height) = dimensions(value)?;
                (Some(width), Some(height))
            }
        };
        images.push(ListingImage {
            url: values[1].clone(),
            width,
            height,
        });
    }
    let geohash = match one(event, "g", "a listing geohash is one base32 value")? {
        None => None,
        Some(value) => {
            if !(1..=12).contains(&value.len())
                || !value
                    .bytes()
                    .all(|byte| b"0123456789bcdefghjkmnpqrstuvwxyz".contains(&byte))
            {
                return Err(invalid("a listing geohash is one base32 value"));
            }
            Some(value.to_owned())
        }
    };
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("e")) {
        let Some(id) = tag.value() else {
            return Err(invalid("a listing event id must be 32 lowercase hex bytes"));
        };
        decode_lower_hex::<32>(id, "listing event id")
            .map_err(|_| invalid("a listing event id must be 32 lowercase hex bytes"))?;
        if let Some(relay) = tag.as_slice().get(2) {
            optional_relay(relay)?;
        }
    }
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("a")) {
        let Some(value) = tag.value() else {
            return Err(invalid("a listing address is kind:pubkey:identifier"));
        };
        ReplacementAddress::from_str(value)
            .map_err(|_| invalid("a listing address is kind:pubkey:identifier"))?;
        if let Some(relay) = tag.as_slice().get(2) {
            optional_relay(relay)?;
        }
    }
    Ok(Listing {
        identifier: identifier.to_owned(),
        draft,
        content: event.content.clone(),
        title,
        summary,
        published_at,
        location,
        price,
        status,
        topics,
        images,
        geohash,
    })
}

fn price_of(values: &[String]) -> Result<Price, DomainError> {
    if values.len() < 3 || values.len() > 4 || !amount_ok(&values[1]) {
        return Err(invalid(
            "a listing price is an amount, a three-letter currency, and an optional frequency",
        ));
    }
    if values[2].len() != 3 || !values[2].bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(invalid(
            "a listing price is an amount, a three-letter currency, and an optional frequency",
        ));
    }
    let frequency = match values.get(3) {
        None => None,
        Some(value)
            if (1..=16).contains(&value.len())
                && value.bytes().all(|byte| byte.is_ascii_lowercase()) =>
        {
            Some(value.clone())
        }
        Some(_) => {
            return Err(invalid(
                "a listing price frequency is a lowercase word such as month or year",
            ));
        }
    };
    Ok(Price {
        amount: values[1].clone(),
        currency: values[2].clone(),
        frequency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"99".repeat(32)).unwrap()
    }

    #[test]
    fn a_listing_keeps_its_price_and_a_draft_does_not_replace_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/99.md"
        ))
        .unwrap();
        assert!(text.contains("kind:30402"));
        assert!(text.contains("kind:30403"));
        assert!(text.contains("Lorem Ipsum"));
        assert!(text.contains("\"price\", \"100\", \"USD\""));
        assert!(text.contains("nostr:naddr1"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "99.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "99.md")
        );

        let content = "Lorem [ipsum][nostr:naddr1example] dolor sit amet.";
        let event = signer().sign(
            1_675_642_635,
            LISTING_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["title".into(), "Lorem Ipsum".into()]),
                Tag::new(vec!["published_at".into(), "1296962229".into()]),
                Tag::new(vec!["t".into(), "electronics".into()]),
                Tag::new(vec![
                    "image".into(),
                    "https://url.to.img".into(),
                    "256x256".into(),
                ]),
                Tag::new(vec![
                    "summary".into(),
                    "More lorem ipsum that is a little more than the title".into(),
                ]),
                Tag::new(vec!["location".into(), "NYC".into()]),
                Tag::new(vec!["price".into(), "100".into(), "USD".into()]),
                Tag::new(vec![
                    "e".into(),
                    "b3e392b11f5d4f28321cedd09303a748acfd0487aea5a7450b3481c60b6e4f87".into(),
                    "wss://relay.example.com".into(),
                ]),
                Tag::new(vec![
                    "a".into(),
                    "30023:a695f6b60119d9521934a691347d9f78e8770b56da16bb255ee286ddf9fda919:ipsum"
                        .into(),
                    "wss://relay.nostr.org".into(),
                ]),
            ],
            content.into(),
        );
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Addressable);
        let listing = open_listing(&event).unwrap();
        assert!(!listing.draft);
        assert_eq!(listing.identifier, "lorem-ipsum");
        assert_eq!(listing.content, content);
        assert_eq!(listing.title.as_deref(), Some("Lorem Ipsum"));
        assert_eq!(listing.published_at, Some(1_296_962_229));
        assert_eq!(listing.topics, vec!["electronics".to_owned()]);
        assert_eq!(listing.location.as_deref(), Some("NYC"));
        assert_eq!(listing.images[0].width, Some(256));
        assert_eq!(
            listing.price,
            Some(Price {
                amount: "100".into(),
                currency: "USD".into(),
                frequency: None,
            })
        );

        let newer = signer().sign(
            1_675_642_700,
            LISTING_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["title".into(), "Lorem Ipsum".into()]),
                Tag::new(vec!["status".into(), "sold".into()]),
                Tag::new(vec![
                    "price".into(),
                    "15".into(),
                    "EUR".into(),
                    "month".into(),
                ]),
                Tag::new(vec!["g".into(), "dr5reg".into()]),
            ],
            content.into(),
        );
        newer.validate_structure().unwrap();
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
        let sold = open_listing(&newer).unwrap();
        assert_eq!(sold.status, Some(ListingStatus::Sold));
        assert_eq!(sold.price.unwrap().frequency.as_deref(), Some("month"));
        assert_eq!(sold.geohash.as_deref(), Some("dr5reg"));

        let draft = signer().sign(
            1_675_642_800,
            DRAFT_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["title".into(), "Lorem Ipsum".into()]),
            ],
            String::new(),
        );
        draft.validate_structure().unwrap();
        assert!(open_listing(&draft).unwrap().draft);
        assert!(matches!(
            compare_replacement(&event, &draft),
            Err(DomainError::ReplacementAddressMismatch)
        ));

        let bad_currency = signer().sign(
            1_675_642_900,
            LISTING_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["price".into(), "100".into(), "US".into()]),
            ],
            String::new(),
        );
        assert!(bad_currency.validate_structure().is_err());
        let bad_status = signer().sign(
            1_675_643_000,
            LISTING_KIND,
            vec![
                Tag::new(vec!["d".into(), "lorem-ipsum".into()]),
                Tag::new(vec!["status".into(), "expired".into()]),
            ],
            String::new(),
        );
        assert!(bad_status.validate_structure().is_err());
    }
}
