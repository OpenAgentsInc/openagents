use crate::data::example::Example;
use csv::StringRecord;

use regex::Regex;
use std::sync::LazyLock;

#[allow(dead_code)]
static IS_URL_PAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("((http|https)://)(www.)?[a-zA-Z0-9@:%._\\+~#?&//=]{2,256}\\.[a-z]{2,6}\\b([-a-zA-Z0-9@:%._\\+~#?&//=]*)"
).unwrap()
});

pub fn string_record_to_example(
    record: StringRecord,
    input_keys: Vec<String>,
    output_keys: Vec<String>,
) -> Example {
    Example::new(
        record
            .iter()
            .map(|cell| (cell.to_string(), cell.to_string().into()))
            .collect(),
        input_keys.clone(),
        output_keys.clone(),
    )
}

pub fn is_url(path: &str) -> bool {
    IS_URL_PAT.is_match(path)
}
