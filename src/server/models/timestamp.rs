use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgTypeInfo;
use sqlx::{Decode, Encode, Postgres, Type};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamp(OffsetDateTime);

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
        let unix_timestamp = dt.timestamp();
        let nanos = dt.timestamp_subsec_nanos() as i32;
        Self(OffsetDateTime::from_unix_timestamp(unix_timestamp).unwrap() + time::Duration::nanoseconds(nanos as i64))
    }
}

impl From<Timestamp> for DateTime<Utc> {
    fn from(ts: Timestamp) -> Self {
        let unix_timestamp = ts.0.unix_timestamp();
        let nanos = ts.0.nanosecond();
        Utc.timestamp_opt(unix_timestamp, nanos).unwrap()
    }
}

impl From<Option<DateTime<Utc>>> for Option<Timestamp> {
    fn from(opt: Option<DateTime<Utc>>) -> Self {
        opt.map(Timestamp::from)
    }
}

impl From<Option<Timestamp>> for Option<DateTime<Utc>> {
    fn from(opt: Option<Timestamp>) -> Self {
        opt.map(DateTime::from)
    }
}

impl Type<Postgres> for Timestamp {
    fn type_info() -> PgTypeInfo {
        <OffsetDateTime as Type<Postgres>>::type_info()
    }
}

impl Encode<'_, Postgres> for Timestamp {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> sqlx::encode::IsNull {
        self.0.encode_by_ref(buf)
    }
}

impl Decode<'_, Postgres> for Timestamp {
    fn decode(value: sqlx::postgres::PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        Ok(Self(OffsetDateTime::decode(value)?))
    }
}
