use chrono::{DateTime, LocalResult, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize_datetime_option<S>(datetime: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    if datetime.is_none() {
        return serializer.serialize_none();
    }

    let datetime = datetime.unwrap();
    return serializer.serialize_i64(datetime.timestamp_millis());
}

pub fn serialize_datetime<S>(datetime: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    return serializer.serialize_i64(datetime.timestamp_millis());
}

pub fn deserialize_datetime<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where D: Deserializer<'de>
{
    let timestamp = i64::deserialize(deserializer)?;
    let date_time = Utc.timestamp_millis_opt(timestamp);

    let date_time = match date_time {
        LocalResult::Single(t) => t,
        LocalResult::None => return Ok(None),
        LocalResult::Ambiguous(_, _) => return Ok(None)
    };

    return Ok(Some(date_time));
}