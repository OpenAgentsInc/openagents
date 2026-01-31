use wgpui::components::organisms::{
    GuidanceAction, GuidanceAudit, GuidanceDecision, GuidanceField, GuidanceGoal,
    GuidanceGuardrail, GuidanceGuardrailStatus, GuidanceModuleCard, GuidanceNetwork,
    GuidancePermissions, GuidanceState,
};
use wgpui::{Bounds, Component, PaintContext};

use crate::helpers::{draw_panel, panel_height};
use crate::state::Storybook;

impl Storybook {
    pub(crate) fn paint_guidance_modules(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let decision_height = panel_height(560.0);
        let decision_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, width, decision_height);
        draw_panel(
            "Guidance Module Decision",
            decision_bounds,
            cx,
            |inner, cx| {
                let goal = GuidanceGoal::new("Ship Guidance Module UI").success_criteria(vec![
                    "Card renders core fields".to_string(),
                    "Story shows decision inputs + outputs".to_string(),
                    "Sidebar entry is visible".to_string(),
                ]);

                let summary = vec![
                    GuidanceField::new("Last turn", "Added Guidance Module card component"),
                    GuidanceField::new("Diff", "3 files changed, 1 added"),
                    GuidanceField::new("Tests", "Not run yet"),
                    GuidanceField::new("Budget", "15k tokens remaining"),
                ];

                let state = GuidanceState::new(7, 1)
                    .tokens_remaining(Some(15_240))
                    .time_remaining_ms(Some(18 * 60 * 1000));

                let permissions = GuidancePermissions::new(true, true, GuidanceNetwork::Scoped);

                let decision = GuidanceDecision::new(
                    GuidanceAction::Pause,
                    "Confidence dipped below threshold after repeated no-progress turns.",
                    0.62,
                )
                .next_input("Ask for confirmation before running full test suite.")
                .tags(vec![
                    "verification".to_string(),
                    "budget".to_string(),
                    "handoff".to_string(),
                ]);

                let guardrails = vec![
                    GuidanceGuardrail::new("max_turns", GuidanceGuardrailStatus::Clear)
                        .detail("7/20 used"),
                    GuidanceGuardrail::new("no_progress_limit", GuidanceGuardrailStatus::Triggered)
                        .detail("2 consecutive stalls"),
                    GuidanceGuardrail::new("low_confidence", GuidanceGuardrailStatus::Triggered)
                        .detail("confidence < 0.7"),
                    GuidanceGuardrail::new("max_tokens", GuidanceGuardrailStatus::Clear)
                        .detail("15k remaining"),
                ];

                let audit =
                    GuidanceAudit::new("in_b7c1", "out_4e2a", "gpt-5.2-codex", "guidance.v1");

                let mut card = GuidanceModuleCard::new("Guidance Module")
                    .goal(goal)
                    .summary(summary)
                    .state(state)
                    .permissions(permissions)
                    .decision(decision)
                    .guardrails(guardrails)
                    .audit(audit);

                card.paint(inner, cx);
            },
        );
    }
}
