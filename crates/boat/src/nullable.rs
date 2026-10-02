use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An optional nullable field: omitted, explicit JSON null, or a value.
///
/// Use `Null` to disable automatic archival and `Unset` to keep server defaults.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum Nullable<T> {
    #[default]
    Unset,
    Null,
    Value(T),
}

impl<T> Nullable<T> {
    pub fn is_unset(&self) -> bool {
        matches!(self, Self::Unset)
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Unset | Self::Null => None,
        }
    }
}

impl<T> From<T> for Nullable<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

impl<T> std::fmt::Debug for Nullable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unset => "Unset",
            Self::Null => "Null",
            Self::Value(_) => "Value(..)",
        })
    }
}

impl<T: Serialize> Serialize for Nullable<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Null => serializer.serialize_none(),
            Self::Unset => Err(serde::ser::Error::custom("An unset field must be omitted.")),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match Option::<T>::deserialize(deserializer)? {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}
