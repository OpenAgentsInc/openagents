use wgpui::PaintContext;

use crate::app_state::{SkillRegistryPaneState, SkillTrustRevocationPaneState};
use crate::pane_renderer::{
    paint_action_button, paint_label_line, paint_multiline_phrase, paint_source_badge,
    paint_state_summary,
};
use crate::pane_system::{
    skill_registry_discover_button_bounds, skill_registry_inspect_button_bounds,
    skill_registry_install_button_bounds, skill_registry_row_bounds,
    skill_registry_visible_row_count, skill_trust_attestations_button_bounds,
    skill_trust_kill_switch_button_bounds, skill_trust_refresh_button_bounds,
    skill_trust_revoke_button_bounds,
};

pub fn paint_skill_registry_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &SkillRegistryPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, &pane_state.source, paint);

    let discover = skill_registry_discover_button_bounds(content_bounds);
    let inspect = skill_registry_inspect_button_bounds(content_bounds);
    let install = skill_registry_install_button_bounds(content_bounds);
    let install_label = pane_state
        .selected_skill_index
        .and_then(|index| pane_state.discovered_skills.get(index))
        .map_or("Enable/Disable", |skill| {
            if skill.enabled {
                "Disable Skill"
            } else {
                "Enable Skill"
            }
        });

    paint_action_button(discover, "Discover", paint);
    paint_action_button(inspect, "Inspect Manifest", paint);
    paint_action_button(install, install_label, paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        discover.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Search",
        &pane_state.search_query,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Slug",
        &pane_state.manifest_slug,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Version",
        &pane_state.manifest_version,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Manifest a",
        pane_state.manifest_a.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "33400 manifest",
        pane_state.manifest_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "33401 version log",
        pane_state.version_event_id.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "6390 search result",
        pane_state
            .search_result_event_id
            .as_deref()
            .unwrap_or("n/a"),
    );

    let mut y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Repo skills root",
        pane_state.repo_skills_root.as_deref().unwrap_or("n/a"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Discovered skills",
        &pane_state.discovered_skills.len().to_string(),
    );

    let visible_rows = skill_registry_visible_row_count(pane_state.discovered_skills.len());
    for row_index in 0..visible_rows {
        let Some(skill) = pane_state.discovered_skills.get(row_index) else {
            continue;
        };
        let row_bounds = skill_registry_row_bounds(content_bounds, row_index);
        let selected = pane_state.selected_skill_index == Some(row_index);

        paint.scene.draw_quad(
            wgpui::Quad::new(row_bounds)
                .with_background(if selected {
                    wgpui::theme::bg::ELEVATED.with_alpha(0.92)
                } else {
                    wgpui::theme::bg::APP.with_alpha(0.64)
                })
                .with_border(wgpui::theme::border::DEFAULT, 1.0)
                .with_corner_radius(4.0),
        );
        let row_label = format!(
            "{} [{}] {} deps:{}",
            skill.name,
            if skill.enabled { "enabled" } else { "disabled" },
            skill.scope,
            skill.dependency_count
        );
        paint.scene.draw_text(paint.text.layout_mono(
            &row_label,
            wgpui::Point::new(row_bounds.origin.x + 8.0, row_bounds.origin.y + 16.0),
            10.0,
            if selected {
                wgpui::theme::accent::PRIMARY
            } else {
                wgpui::theme::text::MUTED
            },
        ));
    }

    y = skill_registry_row_bounds(content_bounds, visible_rows).max_y() + 10.0;
    if let Some(index) = pane_state.selected_skill_index
        && let Some(skill) = pane_state.discovered_skills.get(index)
    {
        let selected_details = format!(
            "{} ({}) path={} deps={} interface={}",
            skill.name,
            skill.scope,
            skill.path,
            skill.dependency_count,
            skill
                .interface_display_name
                .as_deref()
                .unwrap_or("n/a")
        );
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "Selected",
            &selected_details,
        );
    }

    for error in pane_state.discovery_errors.iter().take(4) {
        y = paint_multiline_phrase(
            paint,
            content_bounds.origin.x + 12.0,
            y,
            "codex error",
            error,
        );
    }
}

pub fn paint_skill_trust_revocation_pane(
    content_bounds: wgpui::Bounds,
    pane_state: &SkillTrustRevocationPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "runtime", paint);

    let refresh = skill_trust_refresh_button_bounds(content_bounds);
    let attestations = skill_trust_attestations_button_bounds(content_bounds);
    let kill_switch = skill_trust_kill_switch_button_bounds(content_bounds);
    let revoke = skill_trust_revoke_button_bounds(content_bounds);

    paint_action_button(refresh, "Refresh Trust", paint);
    paint_action_button(attestations, "Attestations", paint);
    paint_action_button(kill_switch, "Kill Switch", paint);
    paint_action_button(revoke, "Revoke", paint);

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        refresh.max_y() + 12.0,
        pane_state.load_state,
        &format!("State: {}", pane_state.load_state.label()),
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Trust tier",
        &pane_state.trust_tier,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Manifest a",
        pane_state.manifest_a.as_deref().unwrap_or("n/a"),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Attestation count",
        &pane_state.attestation_count.to_string(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Kill switch",
        if pane_state.kill_switch_active {
            "active"
        } else {
            "inactive"
        },
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Revocation",
        pane_state.revocation_event_id.as_deref().unwrap_or("n/a"),
    );
}
