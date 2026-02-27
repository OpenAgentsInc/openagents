use std::str::FromStr;

pub(crate) fn tag_name(tag: &[String]) -> Option<&str> {
    tag.first().map(String::as_str)
}

pub(crate) fn tag_field(tag: &[String], index: usize) -> Option<&str> {
    tag.get(index).map(String::as_str)
}

pub(crate) fn is_tag(tag: &[String], name: &str) -> bool {
    matches!(tag_name(tag), Some(tag_name) if tag_name == name)
}

pub(crate) fn find_tag<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a [String]> {
    tags.iter()
        .find(|tag| is_tag(tag, name) && tag_field(tag, 1).is_some())
        .map(Vec::as_slice)
}

pub(crate) fn find_tag_value<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
    find_tag(tags, name).and_then(|tag| tag_field(tag, 1))
}

pub(crate) fn collect_tag_values(tags: &[Vec<String>], name: &str) -> Vec<String> {
    tags.iter()
        .filter(|tag| is_tag(tag, name))
        .filter_map(|tag| tag_field(tag, 1).map(str::to_owned))
        .collect()
}

pub(crate) fn parse_tag_field<T>(tag: &[String], index: usize) -> Option<T>
where
    T: FromStr,
{
    tag_field(tag, index).and_then(|value| value.parse::<T>().ok())
}

pub(crate) fn parse_tag_value<T>(tags: &[Vec<String>], name: &str) -> Option<T>
where
    T: FromStr,
{
    find_tag(tags, name).and_then(|tag| parse_tag_field(tag, 1))
}
