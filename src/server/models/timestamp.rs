use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgTypeInfo, Decode, Encode, Postgres, Type};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }

    pub fn as_inner(&self) -> &OffsetDateTime {
        &self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        let nanos = dt.timestamp_nanos();
        let secs = nanos / 1_000_000_000;
        let subsec_nanos = (nanos % 1_000_000_000) as u32;
        Self(
            OffsetDateTime::from_unix_timestamp(secs)
                .unwrap()
                .replace_nanosecond(subsec_nanos)
                .unwrap(),
        )
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        let nanos = ts.0.unix_timestamp_nanos();
        let secs = nanos / 1_000_000_000;
        let nsecs = (nanos % 1_000_000_000) as u32;
        Utc.timestamp_opt(secs, nsecs).unwrap()
    }
}

pub trait TimestampExt {
    fn to_timestamp(self) -> Option<Timestamp>;
    fn from_timestamp(ts: Option<Timestamp>) -> Self;
}

impl TimestampExt for Option<DateTime<Utc>> {
    fn to_timestamp(self) -> Option<Timestamp> {
        self.map(Into::into)
    }

    fn from_timestamp(ts: Option<Timestamp>) -> Self {
        ts.map(Into::into)
    }
}

impl Type<Postgres> for Timestamp {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("timestamptz")
    }
}

impl Encode<'_, Postgres> for Timestamp {
    fn encode_by_ref(&self, buf: &mut Vec<u8>) -> sqlx::encode::IsNull {
        self.0.encode_by_ref(buf)
    }
}

impl<'r> Decode<'r, Postgres> for Timestamp {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self(OffsetDateTime::decode(value)?))
    }
}
