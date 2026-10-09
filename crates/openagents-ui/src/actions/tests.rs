use maud::{PreEscaped, Render, html};

use super::*;

const EVIL: &str = r#"<script>alert("x")</script> & 'q'"#;
const EVIL_ESCAPED: &str = "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;q&#39;";

fn out(component: impl Render) -> String {
    component.render().into_string()
}

fn icon() -> PreEscaped<&'static str> {
    PreEscaped(r#"<svg data-test-icon></svg>"#)
}

/// Maud escapes `'` differently from our writer; accept either.
fn assert_escaped(html: &str) {
    assert!(!html.contains("<script>"), "unescaped script in {html}");
    assert!(
        html.contains("&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; "),
        "missing escaped text in {html}"
    );
}

#[test]
fn button_default_snapshot() {
    assert_eq!(
        out(Button::new("Save")),
        r#"<button class="oa-button" type="button" data-color="primary" data-variant="solid" data-pill data-size="md"><span class="oa-button-inner">Save</span></button>"#
    );
}

#[test]
fn button_every_option_snapshot() {
    let html = out(Button::new("Go")
        .color(Color::Danger)
        .variant(ButtonVariant::Outline)
        .pill(false)
        .size(ControlSize::Xl3)
        .icon_size(IconSize::Xl2)
        .gutter_size(GutterSize::Xs2)
        .block(true)
        .selected(true)
        .optically_align(OpticalAlign::End)
        .kind(ButtonType::Submit)
        .name("intent")
        .value("go")
        .id("go")
        .class("tw:mt-2")
        .attr("hx-post", "/go")
        .icon_start(icon())
        .icon_end(icon()));
    assert_eq!(
        html,
        r#"<button id="go" class="oa-button tw:mt-2" type="submit" data-color="danger" data-variant="outline" data-size="3xl" data-gutter-size="2xs" data-icon-size="2xl" data-selected data-block data-optically-align="end" name="intent" value="go" hx-post="/go"><span class="oa-button-inner"><svg data-test-icon></svg><span>Go</span><svg data-test-icon></svg></span></button>"#
    );
}

#[test]
fn button_variants_and_colors() {
    for (variant, name) in [
        (ButtonVariant::Solid, "solid"),
        (ButtonVariant::Soft, "soft"),
        (ButtonVariant::Outline, "outline"),
        (ButtonVariant::Ghost, "ghost"),
        (ButtonVariant::Transparent, "transparent"),
    ] {
        let html = out(Button::new("x").variant(variant));
        assert!(html.contains(&format!(r#"data-variant="{name}""#)));
    }
    for color in [
        Color::Primary,
        Color::Secondary,
        Color::Danger,
        Color::Success,
        Color::Warning,
        Color::Caution,
        Color::Discovery,
        Color::Info,
    ] {
        let html = out(Button::new("x").color(color));
        assert!(html.contains(&format!(r#"data-color="{}""#, color.as_str())));
    }
    for (size, name) in [
        (ControlSize::Xs3, "3xs"),
        (ControlSize::Xs2, "2xs"),
        (ControlSize::Xs, "xs"),
        (ControlSize::Sm, "sm"),
        (ControlSize::Md, "md"),
        (ControlSize::Lg, "lg"),
        (ControlSize::Xl, "xl"),
        (ControlSize::Xl2, "2xl"),
        (ControlSize::Xl3, "3xl"),
    ] {
        assert!(out(Button::new("x").size(size)).contains(&format!(r#"data-size="{name}""#)));
    }
}

#[test]
fn icon_only_button_is_uniform_and_named() {
    assert_eq!(
        out(Button::icon(icon(), "Close").variant(ButtonVariant::Ghost)),
        r#"<button class="oa-button" type="button" data-color="primary" data-variant="ghost" data-pill data-uniform data-size="md" aria-label="Close"><span class="oa-button-inner"><svg data-test-icon></svg></span></button>"#
    );
}

#[test]
fn disabled_button_is_announced() {
    let html = out(Button::new("Save")
        .disabled(true)
        .disabled_tone(DisabledTone::Relaxed));
    assert!(html.contains(
        r#" disabled aria-disabled="true" tabindex="-1" data-disabled data-disabled-tone="relaxed""#
    ));
}

#[test]
fn loading_button_is_busy_and_inert() {
    assert_eq!(
        out(Button::new("Save").loading(true)),
        r#"<button class="oa-button" type="button" data-color="primary" data-variant="solid" data-pill data-size="md" data-loading disabled aria-disabled="true" tabindex="-1" aria-busy="true"><span class="oa-button-loader"><span class="oa-loading-indicator" aria-hidden="true"></span></span><span class="oa-button-inner">Save</span></button>"#
    );
    // Loading is inert but not visually disabled.
    assert!(!out(Button::new("Save").loading(true)).contains("data-disabled"));
}

#[test]
fn button_link_internal_external_and_disabled() {
    assert_eq!(
        out(ButtonLink::new("Docs", "/docs").variant(ButtonVariant::Soft)),
        r#"<a class="oa-button" href="/docs" data-color="primary" data-variant="soft" data-pill data-size="md"><span class="oa-button-inner">Docs</span></a>"#
    );
    let external = out(ButtonLink::new("GitHub", "https://github.com"));
    assert!(
        external.contains(r#"href="https://github.com" target="_blank" rel="noopener noreferrer""#)
    );
    let forced = out(ButtonLink::new("App", "/app").external(true));
    assert!(forced.contains(r#"target="_blank""#));
    assert_eq!(
        out(ButtonLink::new("Docs", "/docs").disabled(true)),
        r#"<span class="oa-button" role="link" aria-disabled="true" tabindex="-1" data-disabled data-color="primary" data-variant="solid" data-pill data-size="md"><span class="oa-button-inner">Docs</span></span>"#
    );
}

#[test]
fn script_urls_are_neutralized() {
    let html = out(ButtonLink::new("x", " JavaScript:alert(1)"));
    assert!(html.contains(r#"href="about:blank""#));
    assert!(out(TextLink::new("x", "java\tscript:alert(1)")).contains("about:blank"));
    assert!(out(Image::new("data:text/html,<b>", "")).contains("about:blank"));
    assert!(out(Image::new("data:image/png;base64,AA", "")).contains("data:image/png"));
}

#[test]
fn event_handler_attributes_are_dropped_in_release() {
    if cfg!(debug_assertions) {
        let result = std::panic::catch_unwind(|| out(Button::new("x").attr("onclick", "evil()")));
        assert!(result.is_err());
    } else {
        assert!(!out(Button::new("x").attr("onclick", "evil()")).contains("onclick"));
    }
}

#[test]
fn copy_button_snapshot() {
    let html = out(CopyButton::new("npm i").variant(ButtonVariant::Ghost));
    assert!(html.starts_with(
        r#"<button class="oa-button" type="button" data-color="primary" data-variant="ghost" data-pill data-uniform data-size="md" aria-label="Copy" data-oa-copy="npm i" data-oa-copied-label="Copied">"#
    ));
    assert!(html.contains(r#"<span class="oa-copy-button-icon" data-copy-icon="copy"><svg"#));
    assert!(html.contains(r#"<span class="oa-copy-button-icon" data-copy-icon="copied"><svg"#));
    assert!(html.contains(
        r#"<span class="oa-copy-button-status" role="status" aria-live="polite"></span>"#
    ));

    let labelled = out(CopyButton::new("x").label("Copy link").copy_from("code-1"));
    assert!(!labelled.contains("aria-label"));
    assert!(!labelled.contains("data-uniform"));
    assert!(labelled.contains(r#"data-oa-copy="x" data-oa-copy-from="code-1""#));
    assert!(labelled.contains("<span>Copy link</span>"));
}

#[test]
fn copy_button_script_uses_the_data_hook() {
    assert!(COPY_BUTTON_JS.contains("[data-oa-copy]"));
    assert!(COPY_BUTTON_JS.contains("data-copied"));
    assert!(!COPY_BUTTON_JS.contains("eval("));
}

#[test]
fn text_link_variants() {
    assert_eq!(
        out(TextLink::new("Terms", "/terms")),
        r#"<a class="oa-text-link" href="/terms" data-underline>Terms</a>"#
    );
    assert_eq!(
        out(TextLink::new("Site", "https://example.com").primary(true)),
        r#"<a class="oa-text-link" target="_blank" rel="noopener noreferrer" href="https://example.com" data-primary>Site</a>"#
    );
    assert_eq!(
        out(TextLink::without_href("More")),
        r#"<span class="oa-text-link" role="button" tabindex="0" data-underline>More</span>"#
    );
}

#[test]
fn badge_snapshot() {
    assert_eq!(
        out(Badge::new("Beta")),
        r#"<span class="oa-badge" data-color="secondary" data-size="sm" data-variant="soft">Beta</span>"#
    );
    assert_eq!(
        out(Badge::new("Live")
            .color(Color::Success)
            .variant(Variant::Solid)
            .size(BadgeSize::Lg)
            .pill(true)
            .icon_start(icon())),
        r#"<span class="oa-badge" data-color="success" data-size="lg" data-pill data-variant="solid"><svg data-test-icon></svg><span>Live</span></span>"#
    );
}

#[test]
fn indicators_snapshot() {
    assert_eq!(
        out(LoadingIndicator::new()),
        r#"<span class="oa-loading-indicator" role="status" aria-label="Loading"></span>"#
    );
    assert_eq!(
        out(LoadingIndicator::new().decorative()),
        r#"<span class="oa-loading-indicator" aria-hidden="true"></span>"#
    );
    assert_eq!(
        out(LoadingDots::new().label("Thinking")),
        r#"<span class="oa-loading-dots" role="status" aria-label="Thinking"><span class="oa-loading-dots-dot"></span><span class="oa-loading-dots-dot"></span><span class="oa-loading-dots-dot"></span></span>"#
    );
    let ring = out(CircularProgress::new(25.0));
    assert!(ring.starts_with(
        r#"<span class="oa-circular-progress" role="progressbar" aria-label="Progress" aria-valuemin="0" aria-valuemax="100" aria-valuenow="25">"#
    ));
    assert!(ring.contains(r#"stroke-dashoffset="37.5""#));
    assert!(out(CircularProgress::new(250.0)).contains(r#"stroke-dashoffset="0""#));
    assert!(out(CircularProgress::new(f32::NAN)).contains(r#"stroke-dashoffset="50""#));
}

#[test]
fn avatar_variants() {
    assert_eq!(
        out(Avatar::new().name("ada")),
        r#"<span class="oa-avatar" role="presentation" data-color="secondary" data-variant="soft"><span class="oa-avatar-initial">A</span></span>"#
    );
    assert_eq!(
        out(Avatar::new()
            .name("Ada")
            .image_url("/a.png")
            .size(AvatarSize::Px32)
            .color(Color::Info)
            .variant(Variant::Solid)),
        r#"<span class="oa-avatar" role="presentation" data-color="info" data-variant="solid" data-avatar-size="32"><span class="oa-avatar-initial">A</span><span class="oa-avatar-image-container"><img src="/a.png" class="oa-avatar-image" data-loaded alt="" role="presentation"></span></span>"#
    );
    assert_eq!(
        out(Avatar::new().overflow_count(1500)),
        r#"<span class="oa-avatar" role="presentation" data-color="secondary" data-variant="soft"><span class="oa-avatar-overflow-count" data-letter-count="2"><span class="oa-avatar-overflow-count-symbol">+</span>2k</span></span>"#
    );
    let button = out(Avatar::new()
        .name("B")
        .interactive(true)
        .attr("aria-label", "Account"));
    assert!(button.starts_with(
        r#"<button class="oa-avatar" data-color="secondary" data-variant="soft" type="button" aria-label="Account">"#
    ));
    assert!(out(Avatar::new().icon(icon())).contains(r#"<span class="oa-avatar-icon"><svg"#));
    // Auth0 Gravatar fallbacks are skipped.
    assert!(
        !out(Avatar::new().image_url("https://gravatar.com/x?d=https://cdn.auth0.com/y"))
            .contains("img")
    );
}

#[test]
fn compact_counts() {
    use super::avatar::compact_count;
    assert_eq!(compact_count(7), "7");
    assert_eq!(compact_count(999), "999");
    assert_eq!(compact_count(1_000), "1k");
    assert_eq!(compact_count(12_400), "12k");
    assert_eq!(compact_count(999_600), "1m");
    assert_eq!(compact_count(2_500_000), "3m");
    assert_eq!(compact_count(4_000_000_000), "4b");
}

#[test]
fn avatar_group_reverses_for_start_stack() {
    let group = out(AvatarGroup::new()
        .size(AvatarSize::Px24)
        .avatar(Avatar::new().name("A"))
        .avatar(Avatar::new().name("B")));
    assert!(
        group.starts_with(
            r#"<div class="oa-avatar-group" data-stack="start" data-avatar-size="24">"#
        )
    );
    assert!(group.find(">B<").unwrap() < group.find(">A<").unwrap());
    let end = out(AvatarGroup::new()
        .stack(AvatarStack::End)
        .avatars([Avatar::new().name("A"), Avatar::new().name("B")]));
    assert!(end.find(">A<").unwrap() < end.find(">B<").unwrap());
}

#[test]
fn alert_snapshot() {
    let html = out(Alert::new()
        .color(Color::Danger)
        .variant(Variant::Soft)
        .title("Payment failed")
        .description("Try another card.")
        .actions(Button::new("Retry")));
    assert!(html.starts_with(
        r#"<div class="oa-alert" data-variant="soft" data-color="danger" role="alert" data-actions-placement="end"><div class="oa-alert-indicator"><svg"#
    ));
    assert!(html.ends_with(
        r#"<div class="oa-alert-content"><div class="oa-alert-message"><div class="oa-alert-title">Payment failed</div><div class="oa-alert-description">Try another card.</div></div><div class="oa-alert-actions"><button class="oa-button" type="button" data-color="primary" data-variant="solid" data-pill data-size="md"><span class="oa-button-inner">Retry</span></button></div></div></div>"#
    ));
    assert_eq!(
        out(Alert::new()
            .title("Heads up")
            .no_indicator()
            .actions_placement(AlertActionsPlacement::Bottom)),
        r#"<div class="oa-alert" data-variant="outline" data-color="primary" data-actions-placement="bottom"><div class="oa-alert-content"><div class="oa-alert-message"><div class="oa-alert-title">Heads up</div></div></div></div>"#
    );
    assert!(
        out(Alert::new().indicator(icon()))
            .contains(r#"<div class="oa-alert-indicator"><svg data-test-icon>"#)
    );
}

#[test]
fn empty_message_snapshot() {
    assert_eq!(
        out(EmptyMessage::new()
            .icon(icon())
            .icon_size(EmptyMessageIconSize::Sm)
            .icon_color(Color::Warning)
            .title("No runs yet")
            .description("Start one from the composer.")
            .actions(Button::new("New run"))),
        r#"<div class="oa-empty-message" data-fill="static"><div class="oa-empty-message-icon" data-size="sm" data-color="warning" aria-hidden="true"><svg data-test-icon></svg></div><div class="oa-empty-message-title" data-color="secondary">No runs yet</div><div class="oa-empty-message-description">Start one from the composer.</div><div class="oa-empty-message-action-row"><button class="oa-button" type="button" data-color="primary" data-variant="solid" data-pill data-size="md"><span class="oa-button-inner">New run</span></button></div></div>"#
    );
    assert!(
        out(EmptyMessage::new().fill(EmptyMessageFill::Absolute))
            .contains(r#"data-fill="absolute""#)
    );
}

#[test]
fn image_snapshot() {
    assert_eq!(
        out(Image::new("/x.png", "A chart")
            .width(320)
            .height(200)
            .lazy()),
        r#"<img class="oa-image" src="/x.png" alt="A chart" width="320" height="200" loading="lazy" decoding="async" data-loaded draggable="false">"#
    );
    assert_eq!(out(Image::new("", "nothing")), "");
}

#[test]
fn shimmer_text_snapshot() {
    assert_eq!(
        out(ShimmerText::new("Thinking").tag(ShimmerTag::Span)),
        r#"<span class="oa-shimmer-text">Thinking</span>"#
    );
    assert_eq!(
        out(ShimmerText::new("Done").idle(true)),
        r#"<div class="oa-shimmer-text" data-idle>Done</div>"#
    );
}

#[test]
fn every_text_input_is_escaped() {
    let cases: Vec<String> = vec![
        out(Button::new(EVIL)),
        out(Button::new("x").name(EVIL)),
        out(Button::new("x").value(EVIL)),
        out(Button::new("x").aria_label(EVIL)),
        out(Button::new("x").id(EVIL)),
        out(Button::new("x").class(EVIL)),
        out(Button::new("x").attr("hx-vals", EVIL)),
        out(Button::icon(icon(), EVIL)),
        out(ButtonLink::new(EVIL, "/")),
        out(ButtonLink::new("x", EVIL)),
        out(ButtonLink::new("x", "/").disabled(true).aria_label(EVIL)),
        out(CopyButton::new(EVIL)),
        out(CopyButton::new("x").label(EVIL)),
        out(CopyButton::new("x").copied_label(EVIL)),
        out(CopyButton::new("x").copy_from(EVIL)),
        out(TextLink::new(EVIL, "/")),
        out(TextLink::new("x", EVIL)),
        out(TextLink::without_href(EVIL)),
        out(Badge::new(EVIL)),
        out(Badge::new(EVIL).icon_start(icon())),
        out(LoadingIndicator::new().label(EVIL)),
        out(LoadingDots::new().label(EVIL)),
        out(CircularProgress::new(1.0).label(EVIL)),
        out(Avatar::new().image_url(EVIL)),
        out(Avatar::new().attr("aria-label", EVIL)),
        out(Alert::new().title(EVIL)),
        out(Alert::new().description(EVIL)),
        out(EmptyMessage::new().title(EVIL)),
        out(EmptyMessage::new().description(EVIL)),
        out(Image::new(EVIL, "")),
        out(Image::new("/x.png", EVIL)),
        out(ShimmerText::new(EVIL)),
    ];
    for html in &cases {
        assert_escaped(html);
    }
    // Our attribute writer also escapes single quotes.
    assert!(out(Button::new("x").aria_label(EVIL)).contains(EVIL_ESCAPED));
    // Avatar initials come from the escaped first character.
    assert!(out(Avatar::new().name("<b>")).contains(">&lt;<"));
    // Markup slots are trusted and passed through.
    assert!(out(Alert::new().description_markup(html! { b { "ok" } })).contains("<b>ok</b>"));
}

#[test]
fn stylesheets_use_oa_classes_and_no_build_syntax() {
    let sheets = [
        include_str!("../../static/components/button.css"),
        include_str!("../../static/components/text-link.css"),
        include_str!("../../static/components/badge.css"),
        include_str!("../../static/components/indicator.css"),
        include_str!("../../static/components/avatar.css"),
        include_str!("../../static/components/alert.css"),
        include_str!("../../static/components/empty-message.css"),
        include_str!("../../static/components/image.css"),
        include_str!("../../static/components/shimmer-text.css"),
    ];
    for sheet in sheets {
        assert!(sheet.contains("Apps SDK UI"), "missing attribution");
        assert!(!sheet.contains("@mixin"), "postcss mixin left in sheet");
        assert!(
            !sheet.contains("spacing("),
            "postcss function left in sheet"
        );
        assert!(
            !sheet.contains(":global("),
            "CSS-modules syntax left in sheet"
        );
        let opens = sheet.matches('{').count();
        assert_eq!(opens, sheet.matches('}').count(), "unbalanced braces");
    }
    // Focus-visible rings are carried over.
    assert!(sheets[0].contains("&:focus-visible"));
    assert!(sheets[1].contains("&:focus-visible"));
}
