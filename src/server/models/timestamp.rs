use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{
    postgres::{PgArgumentBuffer, PgTypeInfo},
    Decode, Encode, Postgres, Type,
};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timestamp(OffsetDateTime);

pub trait TimestampExt {
    fn to_timestamp(self) -> Option<Timestamp>;
    fn from_timestamp(ts: Option<Timestamp>) -> Self;
}

impl Timestamp {
    pub fn now() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn into_inner(self) -> OffsetDateTime {
        self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(dt: DateTime<Utc>) -> Self {
        let nanos = dt.timestamp_nanos_opt().unwrap_or_default();
        let secs = nanos / 1_000_000_000;
        let subsec_nanos = (nanos % 1_000_000_000) as u32;
        Self(
            OffsetDateTime::from_unix_timestamp(secs.try_into().unwrap())
                .unwrap()
                .replace_nanosecond(subsec_nanos)
                .unwrap(),
        )
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        let secs = ts.0.unix_timestamp();
        let nsecs = ts.0.nanosecond();
        Utc.timestamp_opt(secs, nsecs).unwrap()
    }
}

impl From<Option<DateTime<Utc>>> for Option<Timestamp> {
    fn from(opt: Option<DateTime<Utc>>) -> Self {
        opt.map(Into::into)
    }
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
        <OffsetDateTime as Type<Postgres>>::type_info()
    }
}

impl Encode<'_, Postgres> for Timestamp {
    fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> sqlx::encode::IsNull {
        self.0.encode_by_ref(buf)
    }
}

impl<'r> Decode<'r, Postgres> for Timestamp {
    fn decode(
        value: <Postgres as sqlx::database::HasValueRef<'r>>::ValueRef,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self(OffsetDateTime::decode(value)?))
    }
}
