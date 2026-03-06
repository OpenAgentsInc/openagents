use crate::deck::model::{Deck, Slide};

#[derive(Clone, Debug)]
pub struct DeckState {
    deck: Deck,
    current_slide_index: usize,
    redraw_requested: bool,
}

impl DeckState {
    pub fn new(deck: Deck) -> Result<Self, String> {
        if deck.is_empty() {
            return Err("deck must contain at least one slide".to_string());
        }

        Ok(Self {
            deck,
            current_slide_index: 0,
            redraw_requested: true,
        })
    }

    pub fn deck(&self) -> &Deck {
        &self.deck
    }

    pub fn current_slide(&self) -> Option<&Slide> {
        self.deck
            .slides
            .get(self.current_slide_index)
            .or_else(|| self.deck.slides.first())
    }

    pub fn current_slide_index(&self) -> usize {
        self.current_slide_index
    }

    pub fn slide_count(&self) -> usize {
        self.deck.slide_count()
    }

    pub fn advance(&mut self) -> bool {
        if self.current_slide_index + 1 < self.slide_count() {
            self.current_slide_index += 1;
            self.redraw_requested = true;
            true
        } else {
            false
        }
    }

    pub fn retreat(&mut self) -> bool {
        if self.current_slide_index > 0 {
            self.current_slide_index -= 1;
            self.redraw_requested = true;
            true
        } else {
            false
        }
    }

    pub fn jump_to(&mut self, index: usize) -> bool {
        if index < self.slide_count() && index != self.current_slide_index {
            self.current_slide_index = index;
            self.redraw_requested = true;
            true
        } else {
            false
        }
    }

    pub fn request_redraw(&mut self) {
        self.redraw_requested = true;
    }

    pub fn take_redraw_request(&mut self) -> bool {
        let redraw_requested = self.redraw_requested;
        self.redraw_requested = false;
        redraw_requested
    }
}

#[cfg(test)]
mod tests {
    use crate::deck::model::{
        Deck, DeckMetadata, DeckTheme, MarkdownSlide, Slide, SlideKind, SlideLayout,
        SlideTransition,
    };
    use crate::state::DeckState;
    use wgpui::markdown::MarkdownDocument;

    fn sample_deck() -> Deck {
        Deck {
            metadata: DeckMetadata {
                title: "Example".to_string(),
                slug: None,
                theme: DeckTheme::Hud,
            },
            slides: vec![
                Slide {
                    id: "one".to_string(),
                    title: "One".to_string(),
                    eyebrow: None,
                    summary: None,
                    footer: None,
                    sources: Vec::new(),
                    theme: DeckTheme::Hud,
                    layout: SlideLayout::Body,
                    diagram: None,
                    notes: None,
                    transition: SlideTransition::None,
                    kind: SlideKind::Markdown(MarkdownSlide {
                        markdown: "# One".to_string(),
                        document: MarkdownDocument::new(),
                    }),
                },
                Slide {
                    id: "two".to_string(),
                    title: "Two".to_string(),
                    eyebrow: None,
                    summary: None,
                    footer: None,
                    sources: Vec::new(),
                    theme: DeckTheme::Hud,
                    layout: SlideLayout::Body,
                    diagram: None,
                    notes: None,
                    transition: SlideTransition::None,
                    kind: SlideKind::Markdown(MarkdownSlide {
                        markdown: "# Two".to_string(),
                        document: MarkdownDocument::new(),
                    }),
                },
            ],
        }
    }

    #[test]
    fn navigates_without_running_past_bounds() {
        let created = DeckState::new(sample_deck());
        assert!(created.is_ok(), "state should initialize: {created:?}");
        let Ok(mut state) = created else {
            return;
        };

        assert_eq!(state.deck().metadata.title, "Example");
        assert!(state.current_slide().is_some());
        assert_eq!(state.current_slide_index(), 0);
        assert!(state.take_redraw_request());
        assert!(!state.take_redraw_request());
        state.request_redraw();
        assert!(state.take_redraw_request());
        assert!(state.advance());
        assert_eq!(state.current_slide_index(), 1);
        assert!(!state.advance());
        assert!(state.jump_to(0));
        assert_eq!(state.current_slide_index(), 0);
        assert!(!state.retreat());
    }
}
