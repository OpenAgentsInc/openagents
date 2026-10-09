//! Serde helpers shared by the wire types.
//!
//! Two shapes recur in Open Responses:
//!
//! - **Open string enums.** Statuses, service tiers, and error types are
//!   strings the spec lets implementers extend. They decode to a known
//!   variant or to `Other(String)` and encode back to the same string.
//!   Closed enums (roles, `tool_choice` modes, truncation) use plain serde
//!   derives and reject unknown values.
//! - **Tagged unions with an unknown arm.** Items, content parts, tools, and
//!   stream events are discriminated by `type`. A known `type` must decode
//!   strictly; an unknown one (an extension such as `acme:search_result`)
//!   is kept whole as JSON, as the spec asks clients to do.

use serde::Serialize;
use serde_json::{Map, Value};

/// Fields an object carried that its Rust type does not name. Kept so a
/// decoded object re-encodes without losing anything.
pub type Extra = Map<String, Value>;

/// Defines an open string enum: known variants plus `Other(String)`.
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident = $wire:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this crate does not name, kept verbatim.
            Other(String),
        }

        impl $name {
            /// The wire string.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $wire, )*
                    Self::Other(value) => value,
                }
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match value {
                    $( $wire => Self::$variant, )*
                    other => Self::Other(other.to_owned()),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Ok(Self::from(value.as_str()))
            }
        }
    };
}

/// Implements `Serialize` and `Deserialize` for a union discriminated by
/// `type`, whose arms are newtype variants over structs that do not name
/// `type` themselves, plus an `Unknown(Value)` arm for any other `type`.
macro_rules! tagged_union {
    ($name:ident { $( $wire:literal => $variant:ident ),* $(,)? }) => {
        impl $name {
            /// The `type` string this value carries on the wire.
            #[must_use]
            pub fn type_name(&self) -> &str {
                match self {
                    $( Self::$variant(_) => $wire, )*
                    Self::Unknown(value) => value
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(""),
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    $( Self::$variant(body) => crate::wire::tagged($wire, body)
                        .map_err(serde::ser::Error::custom)?
                        .serialize(serializer), )*
                    Self::Unknown(value) => value.serialize(serializer),
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = serde_json::Value::deserialize(deserializer)?;
                let (tag, map) = crate::wire::split_tag(value, stringify!($name))
                    .map_err(serde::de::Error::custom)?;
                match tag.as_deref() {
                    $( Some($wire) => crate::wire::from_map(map)
                        .map(Self::$variant)
                        .map_err(|error| serde::de::Error::custom(
                            format!("{} `{}`: {error}", stringify!($name), $wire)
                        )), )*
                    _ => Ok(Self::Unknown(crate::wire::rejoin(tag, map))),
                }
            }
        }
    };
}

pub(crate) use open_enum;
pub(crate) use tagged_union;

/// Serializes `body` as an object and puts `"type": tag` first.
pub(crate) fn tagged<T: Serialize>(tag: &str, body: &T) -> Result<Value, serde_json::Error> {
    let Value::Object(fields) = serde_json::to_value(body)? else {
        return Err(serde::ser::Error::custom(format!(
            "`{tag}` body must serialize to an object"
        )));
    };
    let mut out = Map::with_capacity(fields.len() + 1);
    out.insert("type".to_owned(), Value::String(tag.to_owned()));
    for (key, value) in fields {
        if key != "type" {
            out.insert(key, value);
        }
    }
    Ok(Value::Object(out))
}

/// Splits an object into its `type` string (if any) and the other fields.
pub(crate) fn split_tag(
    value: Value,
    what: &str,
) -> Result<(Option<String>, Map<String, Value>), String> {
    let Value::Object(mut map) = value else {
        return Err(format!("{what} must be a JSON object"));
    };
    let tag = match map.shift_remove("type") {
        None | Some(Value::Null) => None,
        Some(Value::String(tag)) => Some(tag),
        Some(other) => return Err(format!("{what} `type` must be a string, got {other}")),
    };
    Ok((tag, map))
}

/// Puts a split-off `type` back at the front of an object.
pub(crate) fn rejoin(tag: Option<String>, map: Map<String, Value>) -> Value {
    let mut out = Map::with_capacity(map.len() + 1);
    if let Some(tag) = tag {
        out.insert("type".to_owned(), Value::String(tag));
    }
    out.extend(map);
    Value::Object(out)
}

/// Decodes a struct from an object's fields.
pub(crate) fn from_map<T: serde::de::DeserializeOwned>(
    map: Map<String, Value>,
) -> Result<T, serde_json::Error> {
    serde_json::from_value(Value::Object(map))
}
