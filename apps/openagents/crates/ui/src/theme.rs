//! The app theme. The tokens and palettes are `openagents_chat_view::theme`,
//! shared with the web; this module adds what installing one needs on the
//! desktop: the user's fonts and sizes, the wallpaper tint from saved
//! settings, and `gpui_base`'s scrollbar colors.

use gpui::App;
use openagents_theme::{AccentSelection, SurfacePreference};

pub use openagents_chat_view::theme::*;

/// Install a theme as the gpui global with the desktop's typography and
/// settings. The **only** way the appearance should change; setting the
/// global directly leaves [`current_appearance`] stale.
pub trait ThemeInstall {
    fn install(appearance: Appearance, cx: &mut App);
    fn install_preferences(appearance: Appearance, accent: AccentColor, cx: &mut App);
    fn install_selection(
        appearance: Appearance,
        variant_id: &str,
        accent_selection: AccentSelection,
        surface_preference: SurfacePreference,
        cx: &mut App,
    );
    /// Re-resolve a selection after its registry entry changed in place.
    fn reinstall_selection(
        appearance: Appearance,
        variant_id: &str,
        accent_selection: AccentSelection,
        surface_preference: SurfacePreference,
        cx: &mut App,
    );
}

impl ThemeInstall for Theme {
    /// Install the theme for `appearance` as the gpui global and point the
    /// context-free paint helpers at it. The **only** way the appearance should
    /// change — setting the global directly leaves [`current_appearance`] stale.
    fn install(appearance: Appearance, cx: &mut App) {
        Theme::install_preferences(appearance, AccentColor::default(), cx);
    }

    fn install_preferences(appearance: Appearance, accent: AccentColor, cx: &mut App) {
        let accent_changed = cx
            .try_global::<Theme>()
            .is_some_and(|theme| theme.accent_color != accent);
        set_current_appearance(appearance);
        let next = Theme::for_preferences(appearance, accent)
            .with_font_sans(crate::typography::effective_family_name(cx))
            .with_font_mono(crate::typography::code_effective_family_name(cx))
            .with_font_terminal(crate::typography::terminal_effective_family_name(cx))
            .with_code_font_size(crate::typography::code_font_size(cx))
            .with_terminal_font_size(crate::typography::terminal_font_size(cx));
        sync_gpui_base_scrollbar(&next, cx);
        cx.set_global(next);
        // An accent-only swap leaves CURRENT_APPEARANCE unchanged, but cached
        // resolved colors still need to be discarded for the next frame.
        if accent_changed {
            bump_style_generation();
        }
    }

    fn install_selection(
        appearance: Appearance,
        variant_id: &str,
        accent_selection: AccentSelection,
        surface_preference: SurfacePreference,
        cx: &mut App,
    ) {
        install_selection_inner(
            appearance,
            variant_id,
            accent_selection,
            surface_preference,
            false,
            cx,
        );
    }

    /// Re-resolve a selection after its registry entry changed in place.
    /// Linked themes keep stable ids across reloads, so identity comparisons
    /// cannot detect that their resolved colors moved. Forcing the generation
    /// invalidates paint caches that contain already-resolved colors.
    fn reinstall_selection(
        appearance: Appearance,
        variant_id: &str,
        accent_selection: AccentSelection,
        surface_preference: SurfacePreference,
        cx: &mut App,
    ) {
        install_selection_inner(
            appearance,
            variant_id,
            accent_selection,
            surface_preference,
            true,
            cx,
        );
    }
}

fn install_selection_inner(
    appearance: Appearance,
    variant_id: &str,
    accent_selection: AccentSelection,
    surface_preference: SurfacePreference,
    force_generation: bool,
    cx: &mut App,
) {
    let next = Theme::for_selection_with_wallpaper(
        appearance,
        variant_id,
        accent_selection,
        surface_preference,
        crate::settings::wallpaper_colors::active(cx),
    )
    .with_font_sans(crate::typography::effective_family_name(cx))
    .with_font_mono(crate::typography::code_effective_family_name(cx))
    .with_font_terminal(crate::typography::terminal_effective_family_name(cx))
    .with_code_font_size(crate::typography::code_font_size(cx))
    .with_terminal_font_size(crate::typography::terminal_font_size(cx));
    let changed = cx.try_global::<Theme>().is_some_and(|theme| {
        theme.variant_id != next.variant_id
            || theme.accent_selection != next.accent_selection
            || theme.surface_preference != next.surface_preference
            || theme.appearance != next.appearance
            || theme.wallpaper_color != next.wallpaper_color
    });
    set_current_appearance(appearance);
    sync_gpui_base_scrollbar(&next, cx);
    cx.set_global(next);
    if changed || force_generation {
        bump_style_generation();
    }
}

fn sync_gpui_base_scrollbar(theme: &Theme, cx: &mut App) {
    let (normal, hover, active) = scrollbar_thumb_colors(theme);
    gpui_base::Theme::global_mut(cx).scrollbar.styles = gpui_base::ScrollbarStyles::default()
        .thumb(|style| style.bg(normal))
        .thumb_hover(|style| style.bg(hover))
        .thumb_active(|style| style.bg(active));
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_theme::{SurfaceTreatment, ThemeRegistry};

    #[test]
    fn both_appearances_stay_frosted_and_light_runs_heavier() {
        if Theme::GLASS_ALPHA < 1.0 {
            let (dark, light) = (Theme::dark(), Theme::light());
            assert!(dark.glass().a < 1.0, "dark keeps its translucent frost");
            assert!(light.glass().a < 1.0, "light is glass-forward like dark");
            assert!(
                light.glass().a > dark.glass().a - f32::EPSILON,
                "a light tint dominates the blur less, so it must not run looser than dark"
            );
            assert!(dark.is_frost());
            assert!(light.is_frost());
            assert!(dark.glass_overlay().a < 1.0);
            assert!(light.glass_overlay().a < 1.0);
            assert_eq!(light.glass_overlay().a, 0.45);
            assert_eq!(light.input_glass_bg().a, 0.35);
            assert_eq!(crate::popover::surface_bg(&dark).a, 0.15);
            assert_eq!(dark.for_popup().text_muted, dark.text.opacity(0.64));
            assert_eq!(dark.for_popup().text_faint, dark.text.opacity(0.48));
        } else {
            assert_eq!(Theme::light().glass().a, 1.0);
            assert_eq!(Theme::dark().glass().a, 1.0);
        }
    }

    /// An input plate has to read as *lifted* in both appearances. Dark does that
    /// with a faint white wash; the literal light translation is a faint black
    /// wash, which reads as a dent instead — so light lifts with white plus its
    /// border. Assert the plate is never darker than the panel it sits on.
    #[test]
    fn light_frost_keeps_backdrop_visible_across_builtin_themes() {
        let registry = ThemeRegistry::builtin();
        for variant in registry.families.iter().flat_map(|family| &family.variants) {
            let theme = Theme::from_variant(
                variant,
                AccentSelection::ThemeDefault,
                SurfacePreference::Frosted,
            );
            if !theme.is_frost() {
                continue;
            }
            let popup = theme.for_popup();
            if matches!(theme.appearance, Appearance::Light) {
                assert_eq!(crate::popover::surface_bg(&theme).a, 0.45);
                assert_eq!(crate::popover::surface_bg(&popup).a, 0.45);
                assert_eq!(theme.input_glass_bg().a, 0.35);
            } else {
                assert_eq!(crate::popover::surface_bg(&theme).a, 0.15);
                assert_eq!(crate::popover::surface_bg(&popup).a, 0.15);
            }
            // Text on transparent glass depends on the scene underneath. Test
            // the application's painted canvas rather than a solid black image.
            let canvas = flatten(theme.glass(), theme.bg);
            let floating = flatten(crate::popover::surface_bg(&popup), canvas);
            assert!(
                painted_contrast(popup.text, floating) >= 4.5,
                "{} primary popup text on app canvas",
                variant.id
            );
            let input = flatten(theme.input_glass_bg(), canvas);
            assert!(
                painted_contrast(theme.text, input) >= 4.5,
                "{} input text on app canvas",
                variant.id
            );
        }
    }

    #[test]
    fn opaque_floating_surfaces_keep_their_existing_fill_and_text() {
        for mut theme in [Theme::dark(), Theme::light()] {
            theme.surface_treatment = SurfaceTreatment::Opaque;
            assert_eq!(crate::popover::surface_bg(&theme), theme.input_glass_bg());
            assert_eq!(crate::popover::surface_bg(&theme).a, 1.0);
            assert_eq!(theme.for_popup().text_muted, theme.text_muted);
            assert_eq!(theme.for_popup().text_faint, theme.text_faint);
        }
    }

    #[test]
    fn custom_light_popup_text_is_readable_and_preparation_is_idempotent() {
        let mut variant = ThemeRegistry::builtin()
            .variant("openagents-light")
            .unwrap()
            .clone();
        variant.id = "custom-light".to_owned();
        variant.colors.text_muted = variant.colors.background;
        variant.colors.text_faint = variant.colors.background;
        let theme = Theme::from_variant(
            &variant,
            AccentSelection::ThemeDefault,
            SurfacePreference::Frosted,
        );
        let popup = theme.for_popup();
        let nested = popup.for_popup();
        assert_eq!(popup.text_muted, nested.text_muted);
        assert_eq!(popup.text_faint, nested.text_faint);
        assert_eq!(
            crate::popover::surface_bg(&popup),
            crate::popover::surface_bg(&nested)
        );
        for level in [0, 32, 64, 128, 192, 255] {
            let canvas = flatten(theme.glass(), grey(level));
            let surface = flatten(crate::popover::surface_bg(&popup), canvas);
            for color in [popup.text, popup.text_muted, popup.text_faint] {
                assert!(
                    painted_contrast(color, surface) >= 4.5,
                    "custom light popup text"
                );
            }
        }
    }
}
