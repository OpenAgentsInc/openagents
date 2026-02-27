use super::*;

impl Storybook {
    pub(crate) fn paint_gitafter_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let repo_height = panel_height(240.0);
        let issue_height = panel_height(320.0);
        let pr_height = panel_height(280.0);
        let labels_height = panel_height(200.0);

        let panels = panel_stack(
            bounds,
            &[repo_height, issue_height, pr_height, labels_height],
        );

        // ========== Panel 1: Repository Cards ==========
        let repo_bounds = panels[0];
        draw_panel("Repository Cards", repo_bounds, cx, |inner, cx| {
            let repos = [
                RepoInfo::new("repo-1", "openagents")
                    .description("An open source AI agent framework for autonomous workflows")
                    .visibility(RepoVisibility::Public)
                    .stars(1250)
                    .forks(180)
                    .issues(42)
                    .language("Rust")
                    .updated_at("2 hours ago"),
                RepoInfo::new("repo-2", "wgpui")
                    .description("GPU-accelerated native UI framework")
                    .visibility(RepoVisibility::Public)
                    .stars(340)
                    .forks(28)
                    .issues(15)
                    .language("Rust")
                    .updated_at("Yesterday"),
            ];

            for (i, repo) in repos.iter().enumerate() {
                let mut card = RepoCard::new(repo.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 110.0,
                        inner.size.width.min(500.0),
                        100.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 2: Issue List ==========
        let issue_bounds = panels[1];
        draw_panel("Issue List with Bounties", issue_bounds, cx, |inner, cx| {
            let issues = [
                IssueInfo::new("issue-1", 42, "Memory leak in event processing loop")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::bug())
                    .label(IssueLabel::help_wanted())
                    .author("alice")
                    .bounty(50000)
                    .comments(12)
                    .created_at("3 days ago"),
                IssueInfo::new("issue-2", 43, "Add dark mode toggle to settings")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::enhancement())
                    .label(IssueLabel::good_first_issue())
                    .author("bob")
                    .bounty(25000)
                    .comments(5)
                    .created_at("1 week ago"),
                IssueInfo::new("issue-3", 44, "Update documentation for v2.0 release")
                    .status(IssueStatus::Closed)
                    .label(IssueLabel::new("docs", Hsla::new(190.0, 0.6, 0.5, 1.0)))
                    .author("charlie")
                    .comments(8)
                    .created_at("2 weeks ago"),
            ];

            for (i, issue) in issues.iter().enumerate() {
                let mut row = IssueRow::new(issue.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 90.0,
                        inner.size.width,
                        80.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 3: PR Timeline ==========
        let pr_bounds = panels[2];
        draw_panel("PR Timeline", pr_bounds, cx, |inner, cx| {
            let events = [
                PrEvent::new("ev-1", PrEventType::Commit, "alice")
                    .message("Initial implementation of feature X")
                    .commit_sha("abc1234def")
                    .timestamp("3 hours ago"),
                PrEvent::new("ev-2", PrEventType::Review, "bob")
                    .review_state(ReviewState::Approved)
                    .timestamp("2 hours ago"),
                PrEvent::new("ev-3", PrEventType::Comment, "charlie")
                    .message("Looks good! Just one minor suggestion...")
                    .timestamp("1 hour ago"),
                PrEvent::new("ev-4", PrEventType::Merge, "alice")
                    .message("Merged into main")
                    .timestamp("30 minutes ago"),
            ];

            for (i, event) in events.iter().enumerate() {
                let is_last = i == events.len() - 1;
                let mut item = PrTimelineItem::new(event.clone()).is_last(is_last);
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width.min(500.0),
                        60.0,
                    ),
                    cx,
                );
            }
        });

        // ========== Panel 4: Issue Labels & Status Variants ==========
        let labels_bounds = panels[3];
        draw_panel(
            "Issue Labels & PR Events",
            labels_bounds,
            cx,
            |inner, cx| {
                // Draw predefined labels
                let labels = [
                    IssueLabel::bug(),
                    IssueLabel::enhancement(),
                    IssueLabel::good_first_issue(),
                    IssueLabel::help_wanted(),
                    IssueLabel::new("security", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                    IssueLabel::new("performance", Hsla::new(280.0, 0.6, 0.5, 1.0)),
                ];

                let mut label_x = inner.origin.x;
                for label in &labels {
                    let label_w = (label.name.len() as f32 * 7.0) + 16.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(label_x, inner.origin.y, label_w, 20.0))
                            .with_background(label.color.with_alpha(0.2))
                            .with_border(label.color, 1.0),
                    );
                    let text = cx.text.layout(
                        &label.name,
                        Point::new(label_x + 6.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        label.color,
                    );
                    cx.scene.draw_text(text);
                    label_x += label_w + 8.0;
                }

                // Draw PR event types
                let mut event_y = inner.origin.y + 40.0;
                let events = [
                    PrEventType::Commit,
                    PrEventType::Review,
                    PrEventType::Comment,
                    PrEventType::StatusChange,
                    PrEventType::Merge,
                    PrEventType::Close,
                    PrEventType::Reopen,
                ];

                let mut event_x = inner.origin.x;
                for event in &events {
                    let icon_text = format!("{} {}", event.icon(), event.label());
                    let text = cx.text.layout(
                        &icon_text,
                        Point::new(event_x, event_y),
                        theme::font_size::SM,
                        event.color(),
                    );
                    cx.scene.draw_text(text);
                    event_x += 120.0;
                    if event_x > inner.origin.x + inner.size.width - 120.0 {
                        event_x = inner.origin.x;
                        event_y += 24.0;
                    }
                }

                // Draw review states
                let review_y = event_y + 40.0;
                let states = [
                    ReviewState::Approved,
                    ReviewState::RequestChanges,
                    ReviewState::Commented,
                    ReviewState::Pending,
                ];

                let mut state_x = inner.origin.x;
                for state in &states {
                    let state_w = 120.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(state_x, review_y, state_w, 24.0))
                            .with_background(state.color().with_alpha(0.2))
                            .with_border(state.color(), 1.0),
                    );
                    let text = cx.text.layout(
                        state.label(),
                        Point::new(state_x + 8.0, review_y + 5.0),
                        theme::font_size::XS,
                        state.color(),
                    );
                    cx.scene.draw_text(text);
                    state_x += state_w + 12.0;
                }
            },
        );
    }
}
