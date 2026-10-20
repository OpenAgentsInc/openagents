//! Command palette values, independent of the browser and GPUI (#12279).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    All,
    Actions,
    Chats,
    Files,
    Content,
}

impl Tab {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Actions,
        Self::Chats,
        Self::Files,
        Self::Content,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Actions => "Actions",
            Self::Chats => "Chats",
            Self::Files => "Files",
            Self::Content => "Content",
        }
    }

    pub fn includes(self, tab: Self) -> bool {
        self == Self::All || self == tab
    }
}

/// Every word must match a field. Title hits outrank project/computer hits,
/// word starts beat substrings, and archived chats score half, as on desktop.
pub fn score(query: &str, title: &str, detail: &str, archived: bool) -> Option<u32> {
    let fields = [(title.to_lowercase(), 100), (detail.to_lowercase(), 40)];
    let mut total = 0;
    for term in query.split_whitespace().map(str::to_lowercase) {
        total += fields
            .iter()
            .filter_map(|(field, weight)| {
                field
                    .match_indices(&term)
                    .map(|(at, _)| {
                        let start = field[..at]
                            .chars()
                            .next_back()
                            .is_none_or(|c| !c.is_alphanumeric());
                        weight + if start { weight / 2 } else { 0 }
                    })
                    .max()
            })
            .max()?;
    }
    Some(if archived { total / 2 } else { total })
}

/// Wrap arrow navigation even when a search removes all results.
pub fn step(active: usize, by: isize, count: usize) -> usize {
    if count == 0 {
        0
    } else {
        (active as isize + by).rem_euclid(count as isize) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_search_matches_words_across_fields_and_ranks_like_desktop() {
        assert_eq!(
            score("deploy mac", "Deploy staging", "MacBook", false),
            Some(210)
        );
        assert_eq!(
            score("deploy windows", "Deploy staging", "MacBook", false),
            None
        );
        assert!(score("deploy", "Deploy", "", false) > score("deploy", "Redeploy", "", false));
        assert!(score("deploy", "Deploy", "", false) > score("deploy", "Deploy", "", true));
        assert_eq!(score("CAFÉ", "Café notes", "", false), Some(150));
    }

    #[test]
    fn tabs_and_navigation_handle_empty_and_changed_results() {
        assert!(Tab::All.includes(Tab::Content));
        assert!(!Tab::Chats.includes(Tab::Files));
        assert_eq!(step(0, -1, 3), 2);
        assert_eq!(step(2, 1, 3), 0);
        assert_eq!(step(8, -1, 0), 0);
    }
}
