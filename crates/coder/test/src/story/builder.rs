//! StoryBuilder - fluent API for constructing story tests.

use super::context::TestContext;
use super::inventory::{Story, StoryOutcome};
use std::panic::{self, AssertUnwindSafe};

/// A step function in a story.
pub type StepFn = Box<dyn Fn(&mut TestContext) + Send + Sync>;

/// Builder for constructing story tests with a fluent API.
///
/// # Example
///
/// ```rust,ignore
/// StoryBuilder::new("User can send message")
///     .given(|cx| { /* setup */ })
///     .when(|cx| { /* action */ })
///     .then(|cx| { /* assertion */ })
///     .run();
/// ```
pub struct StoryBuilder {
    name: String,
    given: Vec<StepFn>,
    when: Vec<StepFn>,
    then: Vec<StepFn>,
    tags: Vec<String>,
}

impl StoryBuilder {
    /// Create a new story builder with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            given: Vec::new(),
            when: Vec::new(),
            then: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Add a setup step (given).
    ///
    /// Given steps are executed first and set up the initial state.
    pub fn given<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut TestContext) + Send + Sync + 'static,
    {
        self.given.push(Box::new(f));
        self
    }

    /// Add an action step (when).
    ///
    /// When steps are executed after given steps and perform actions.
    pub fn when<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut TestContext) + Send + Sync + 'static,
    {
        self.when.push(Box::new(f));
        self
    }

    /// Add an assertion step (then).
    ///
    /// Then steps are executed last and verify the expected outcomes.
    pub fn then<F>(mut self, f: F) -> Self
    where
        F: Fn(&mut TestContext) + Send + Sync + 'static,
    {
        self.then.push(Box::new(f));
        self
    }

    /// Add a tag to this story for filtering.
    pub fn tagged(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add multiple tags at once.
    pub fn tagged_all<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Execute this story immediately and panic on failure.
    ///
    /// This is the typical way to run a story in a `#[test]` function.
    pub fn run(self) {
        let story = self.build();
        let outcome = story.execute();

        match outcome {
            StoryOutcome::Passed => {}
            StoryOutcome::Failed { message, phase } => {
                panic!(
                    "Story '{}' failed in {} phase:\n{}",
                    story.name, phase, message
                );
            }
            StoryOutcome::Skipped { reason } => {
                eprintln!("Story '{}' skipped: {}", story.name, reason);
            }
        }
    }

    /// Build the story without executing it.
    ///
    /// Use this when you want to register the story with a test runner
    /// for parallel or batch execution.
    pub fn build(self) -> Story {
        Story {
            name: self.name,
            given: self.given,
            when: self.when,
            then: self.then,
            tags: self.tags,
        }
    }

    /// Register this story with the global test runner.
    ///
    /// Use this for batch execution of multiple stories.
    pub fn register(self) {
        let story = self.build();
        crate::runner::TestRunner::register(story);
    }
}

impl Story {
    /// Execute this story and return the outcome.
    pub fn execute(&self) -> StoryOutcome {
        let mut cx = TestContext::new();

        // Execute given steps
        for step in &self.given {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                step(&mut cx);
            }));

            if let Err(e) = result {
                return StoryOutcome::Failed {
                    message: panic_message(&e),
                    phase: "given".to_string(),
                };
            }
        }

        // Execute when steps
        for step in &self.when {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                step(&mut cx);
            }));

            if let Err(e) = result {
                return StoryOutcome::Failed {
                    message: panic_message(&e),
                    phase: "when".to_string(),
                };
            }
        }

        // Execute then steps
        for step in &self.then {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                step(&mut cx);
            }));

            if let Err(e) = result {
                return StoryOutcome::Failed {
                    message: panic_message(&e),
                    phase: "then".to_string(),
                };
            }
        }

        StoryOutcome::Passed
    }
}

/// Extract a message from a panic payload.
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "Unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_story_builder_basic() {
        let story = StoryBuilder::new("test story")
            .given(|_cx| {})
            .when(|_cx| {})
            .then(|_cx| {})
            .build();

        assert_eq!(story.name, "test story");
        assert_eq!(story.given.len(), 1);
        assert_eq!(story.when.len(), 1);
        assert_eq!(story.then.len(), 1);
    }

    #[test]
    fn test_story_builder_multiple_steps() {
        let story = StoryBuilder::new("multi-step")
            .given(|_cx| {})
            .given(|_cx| {})
            .when(|_cx| {})
            .when(|_cx| {})
            .then(|_cx| {})
            .then(|_cx| {})
            .then(|_cx| {})
            .build();

        assert_eq!(story.given.len(), 2);
        assert_eq!(story.when.len(), 2);
        assert_eq!(story.then.len(), 3);
    }

    #[test]
    fn test_story_builder_tags() {
        let story = StoryBuilder::new("tagged")
            .tagged("chat")
            .tagged("e2e")
            .tagged_all(["smoke", "critical"])
            .build();

        assert_eq!(story.tags, vec!["chat", "e2e", "smoke", "critical"]);
    }

    #[test]
    fn test_story_execute_passes() {
        let story = StoryBuilder::new("passing")
            .given(|_cx| {})
            .when(|_cx| {})
            .then(|_cx| {
                assert_eq!(1 + 1, 2);
            })
            .build();

        assert!(matches!(story.execute(), StoryOutcome::Passed));
    }

    #[test]
    fn test_story_execute_fails() {
        let story = StoryBuilder::new("failing")
            .then(|_cx| {
                panic!("intentional failure");
            })
            .build();

        let outcome = story.execute();
        assert!(matches!(
            outcome,
            StoryOutcome::Failed { phase, .. } if phase == "then"
        ));
    }
}
