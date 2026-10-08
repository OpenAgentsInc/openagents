//! A rendered acknowledgment may clear its own draft, but not a later edit.

pub(crate) fn next_value<'a>(
    acknowledged: &str,
    live: &'a str,
    rendered: &'a str,
    secret: bool,
) -> &'a str {
    if secret || live != acknowledged {
        live
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acknowledged_submission_clears_and_later_typing_survives() {
        assert_eq!(next_value("Send this", "Send this", "", false), "");
        assert_eq!(
            next_value("Send this", "A newer draft", "", false),
            "A newer draft"
        );
        assert_eq!(next_value("", "日本語🙂", "", false), "日本語🙂");
        assert_eq!(next_value("old", "accepted", "accepted", false), "accepted");
    }
    #[test]
    fn secret_drafts_never_come_from_rendered_state() {
        assert_eq!(next_value("", "private draft", "", true), "private draft");
    }
}
