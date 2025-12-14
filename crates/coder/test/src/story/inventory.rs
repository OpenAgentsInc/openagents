//! Story inventory for collecting and executing stories.

use super::builder::StepFn;

/// A complete story ready for execution.
pub struct Story {
    /// Story name (used in reports).
    pub name: String,
    /// Given (setup) steps.
    pub given: Vec<StepFn>,
    /// When (action) steps.
    pub when: Vec<StepFn>,
    /// Then (assertion) steps.
    pub then: Vec<StepFn>,
    /// Tags for filtering.
    pub tags: Vec<String>,
}

impl Story {
    /// Check if this story has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Check if this story matches any of the given tags.
    pub fn matches_any_tag(&self, tags: &[String]) -> bool {
        if tags.is_empty() {
            return true;
        }
        tags.iter().any(|t| self.has_tag(t))
    }

    /// Check if story name matches a filter pattern.
    pub fn matches_filter(&self, filter: &str) -> bool {
        self.name.to_lowercase().contains(&filter.to_lowercase())
    }
}

/// Outcome of executing a story.
#[derive(Debug, Clone)]
pub enum StoryOutcome {
    /// Story passed all assertions.
    Passed,
    /// Story failed with an error.
    Failed {
        /// Error message.
        message: String,
        /// Phase where failure occurred (given/when/then).
        phase: String,
    },
    /// Story was skipped.
    Skipped {
        /// Reason for skipping.
        reason: String,
    },
}

impl StoryOutcome {
    /// Check if this outcome represents a pass.
    pub fn is_passed(&self) -> bool {
        matches!(self, StoryOutcome::Passed)
    }

    /// Check if this outcome represents a failure.
    pub fn is_failed(&self) -> bool {
        matches!(self, StoryOutcome::Failed { .. })
    }

    /// Check if this outcome represents a skip.
    pub fn is_skipped(&self) -> bool {
        matches!(self, StoryOutcome::Skipped { .. })
    }
}

/// Global inventory of registered stories.
pub struct StoryInventory {
    stories: Vec<Story>,
}

impl StoryInventory {
    /// Create a new empty inventory.
    pub fn new() -> Self {
        Self {
            stories: Vec::new(),
        }
    }

    /// Register a story with the inventory.
    pub fn register(&mut self, story: Story) {
        self.stories.push(story);
    }

    /// Get all registered stories.
    pub fn stories(&self) -> &[Story] {
        &self.stories
    }

    /// Get stories filtered by tags.
    pub fn stories_with_tags(&self, tags: &[String]) -> Vec<&Story> {
        self.stories
            .iter()
            .filter(|s| s.matches_any_tag(tags))
            .collect()
    }

    /// Get stories filtered by name pattern.
    pub fn stories_matching(&self, filter: &str) -> Vec<&Story> {
        self.stories
            .iter()
            .filter(|s| s.matches_filter(filter))
            .collect()
    }

    /// Total number of registered stories.
    pub fn len(&self) -> usize {
        self.stories.len()
    }

    /// Check if inventory is empty.
    pub fn is_empty(&self) -> bool {
        self.stories.is_empty()
    }
}

impl Default for StoryInventory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_story(name: &str, tags: &[&str]) -> Story {
        Story {
            name: name.to_string(),
            given: Vec::new(),
            when: Vec::new(),
            then: Vec::new(),
            tags: tags.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn test_story_has_tag() {
        let story = make_story("test", &["chat", "e2e"]);

        assert!(story.has_tag("chat"));
        assert!(story.has_tag("e2e"));
        assert!(!story.has_tag("smoke"));
    }

    #[test]
    fn test_story_matches_any_tag() {
        let story = make_story("test", &["chat", "e2e"]);

        // Empty tags should match all
        assert!(story.matches_any_tag(&[]));

        // Should match if any tag matches
        assert!(story.matches_any_tag(&["chat".to_string()]));
        assert!(story.matches_any_tag(&["smoke".to_string(), "chat".to_string()]));

        // Should not match if no tags match
        assert!(!story.matches_any_tag(&["smoke".to_string()]));
    }

    #[test]
    fn test_story_matches_filter() {
        let story = make_story("User can send a message", &[]);

        assert!(story.matches_filter("send"));
        assert!(story.matches_filter("message"));
        assert!(story.matches_filter("SEND")); // Case insensitive
        assert!(!story.matches_filter("receive"));
    }

    #[test]
    fn test_story_outcome() {
        let passed = StoryOutcome::Passed;
        assert!(passed.is_passed());
        assert!(!passed.is_failed());

        let failed = StoryOutcome::Failed {
            message: "error".to_string(),
            phase: "then".to_string(),
        };
        assert!(failed.is_failed());
        assert!(!failed.is_passed());

        let skipped = StoryOutcome::Skipped {
            reason: "not implemented".to_string(),
        };
        assert!(skipped.is_skipped());
    }

    #[test]
    fn test_inventory() {
        let mut inv = StoryInventory::new();

        inv.register(make_story("Story A", &["chat"]));
        inv.register(make_story("Story B", &["e2e"]));
        inv.register(make_story("Story C", &["chat", "e2e"]));

        assert_eq!(inv.len(), 3);

        let chat_stories = inv.stories_with_tags(&["chat".to_string()]);
        assert_eq!(chat_stories.len(), 2);

        let matching = inv.stories_matching("Story A");
        assert_eq!(matching.len(), 1);
    }
}
