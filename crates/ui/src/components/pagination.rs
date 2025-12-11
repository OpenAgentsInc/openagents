//! Pagination component for page navigation

use gpui::prelude::*;
use gpui::*;
use theme::{bg, text, border};

/// Pagination component
///
/// # Example
/// ```
/// Pagination::new()
///     .total_pages(10)
///     .current_page(3)
///     .on_page_change(|page, _, _| set_page(page))
/// ```
#[derive(IntoElement)]
pub struct Pagination {
    current_page: usize,
    total_pages: usize,
    siblings: usize, // Number of pages to show on each side of current
    on_page_change: Option<Box<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
}

impl Pagination {
    pub fn new() -> Self {
        Self {
            current_page: 1,
            total_pages: 1,
            siblings: 1,
            on_page_change: None,
        }
    }

    pub fn current_page(mut self, page: usize) -> Self {
        self.current_page = page.max(1);
        self
    }

    pub fn total_pages(mut self, total: usize) -> Self {
        self.total_pages = total.max(1);
        self
    }

    pub fn siblings(mut self, count: usize) -> Self {
        self.siblings = count;
        self
    }

    pub fn on_page_change(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_page_change = Some(Box::new(handler));
        self
    }

    fn get_page_range(&self) -> Vec<usize> {
        let current = self.current_page;
        let total = self.total_pages;
        let siblings = self.siblings;

        let mut pages = Vec::new();

        // Always show first page
        pages.push(1);

        // Calculate range around current
        let start = (current.saturating_sub(siblings)).max(2);
        let end = (current + siblings).min(total.saturating_sub(1));

        for p in start..=end {
            if p > 1 && p < total {
                pages.push(p);
            }
        }

        // Always show last page (if > 1)
        if total > 1 {
            pages.push(total);
        }

        pages.sort();
        pages.dedup();
        pages
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Pagination {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let mut container = div()
            .flex()
            .items_center()
            .gap(px(4.0));

        let current = self.current_page;
        let total = self.total_pages;
        let pages = self.get_page_range();
        let on_change = self.on_page_change;

        // Previous button
        let prev_disabled = current <= 1;
        let prev_target = current.saturating_sub(1);

        let mut prev_btn = div()
            .id(ElementId::Name("page-nav-prev".into()))
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .border_1()
            .border_color(border::DEFAULT)
            .text_sm()
            .child("←");

        if prev_disabled {
            prev_btn = prev_btn
                .opacity(0.5)
                .cursor_not_allowed()
                .text_color(text::DISABLED);
        } else {
            prev_btn = prev_btn
                .cursor_pointer()
                .text_color(text::PRIMARY)
                .hover(|s| s.bg(bg::HOVER));

            if let Some(ref handler) = on_change {
                let handler_ptr = handler.as_ref() as *const _;
                prev_btn = prev_btn.on_click(move |_, window, cx| {
                    unsafe {
                        let handler: &dyn Fn(usize, &mut Window, &mut App) = &*handler_ptr;
                        handler(prev_target, window, cx);
                    }
                });
            }
        }

        container = container.child(prev_btn);

        // Page numbers
        let mut prev_page = 0usize;

        for page in pages {
            // Add ellipsis if there's a gap
            if page > prev_page + 1 && prev_page > 0 {
                container = container.child(
                    div()
                        .px(px(8.0))
                        .text_sm()
                        .text_color(text::MUTED)
                        .child("...")
                );
            }

            let is_current = page == current;

            let mut page_btn = div()
                .id(ElementId::Name(format!("page-{}", page).into()))
                .w(px(32.0))
                .h(px(32.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .text_sm()
                .child(format!("{}", page));

            if is_current {
                page_btn = page_btn
                    .bg(bg::ELEVATED)
                    .border_1()
                    .border_color(border::DEFAULT)
                    .text_color(text::PRIMARY)
                    .font_weight(FontWeight::MEDIUM);
            } else {
                page_btn = page_btn
                    .cursor_pointer()
                    .text_color(text::MUTED)
                    .hover(|s| s.bg(bg::HOVER).text_color(text::PRIMARY));

                if let Some(ref handler) = on_change {
                    let handler_ptr = handler.as_ref() as *const _;
                    let target_page = page;
                    page_btn = page_btn.on_click(move |_, window, cx| {
                        unsafe {
                            let handler: &dyn Fn(usize, &mut Window, &mut App) = &*handler_ptr;
                            handler(target_page, window, cx);
                        }
                    });
                }
            }

            container = container.child(page_btn);
            prev_page = page;
        }

        // Next button
        let next_disabled = current >= total;
        let next_target = current + 1;

        let mut next_btn = div()
            .id(ElementId::Name("page-nav-next".into()))
            .w(px(32.0))
            .h(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(4.0))
            .border_1()
            .border_color(border::DEFAULT)
            .text_sm()
            .child("→");

        if next_disabled {
            next_btn = next_btn
                .opacity(0.5)
                .cursor_not_allowed()
                .text_color(text::DISABLED);
        } else {
            next_btn = next_btn
                .cursor_pointer()
                .text_color(text::PRIMARY)
                .hover(|s| s.bg(bg::HOVER));

            if let Some(ref handler) = on_change {
                let handler_ptr = handler.as_ref() as *const _;
                next_btn = next_btn.on_click(move |_, window, cx| {
                    unsafe {
                        let handler: &dyn Fn(usize, &mut Window, &mut App) = &*handler_ptr;
                        handler(next_target, window, cx);
                    }
                });
            }
        }

        container = container.child(next_btn);

        container
    }
}
