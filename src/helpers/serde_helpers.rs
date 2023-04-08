use chrono::{DateTime, Utc};
use serde::Serializer;

pub fn serialize_datetime<S>(datetime: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
{
    if datetime.is_none() {
        return serializer.serialize_none();
    }

    let datetime = datetime.unwrap();
    return serializer.serialize_i64(datetime.timestamp());
}