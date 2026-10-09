use super::*;

/// Coder Light's native palette is exactly the table's light side, so a
/// native light surface and a light web page paint the same colors.
#[test]
fn the_light_palette_is_the_tables_light_side() {
    let resolver = Resolver::new();
    for (token, color) in Palette::SOURCES.iter().zip(Palette::LIGHT.roles()) {
        let resolved = resolver
            .color(token, Scheme::Light)
            .unwrap_or_else(|| panic!("{token} resolves in light"))
            .to_8bit();
        assert_eq!(resolved, color, "{token}");
    }
}

/// Coder Noir's native palette is the table's dark side wherever Noir
/// overrides the role, and every role it overrides is in the palette.
#[test]
fn the_noir_palette_is_the_tables_dark_side_where_noir_owns_the_role() {
    let resolver = Resolver::new();
    for (token, noir) in noir::OVERRIDES {
        let at = Palette::SOURCES
            .iter()
            .position(|source| source == token)
            .unwrap_or_else(|| panic!("{token} has no palette role"));
        assert_eq!(Palette::NOIR.roles()[at], Rgba8::rgb(*noir), "{token}");
        assert_eq!(
            resolver.color(token, Scheme::Dark).map(Rgba::to_8bit),
            Some(Rgba8::rgb(*noir)),
            "{token}"
        );
    }
}

#[test]
fn every_palette_source_is_a_real_token() {
    let resolver = Resolver::new();
    for token in Palette::SOURCES {
        assert!(resolver.defines(token), "{token}");
    }
}

/// Text on the canvas and raised surfaces is legible in both palettes
/// (WCAG AA 4.5:1 for content and secondary content, 3:1 for tertiary).
#[test]
fn palette_text_is_legible_in_both_schemes() {
    let to = |c: Rgba8| Rgba {
        r: f64::from(c.r) / 255.0,
        g: f64::from(c.g) / 255.0,
        b: f64::from(c.b) / 255.0,
        a: f64::from(c.a) / 255.0,
    };
    for scheme in [Scheme::Light, Scheme::Dark] {
        let palette = Palette::of(scheme);
        for field in [
            palette.canvas,
            palette.surface_subtle,
            palette.surface_raised,
        ] {
            let field = to(field);
            for (text, min) in [
                (palette.content, 4.5),
                (palette.content_secondary, 4.5),
                (palette.content_tertiary, 3.0),
            ] {
                let ratio = to(text).over(field).contrast(field);
                assert!(ratio >= min, "{scheme:?}: {ratio:.2} < {min}");
            }
        }
        let solid = to(palette.accent_solid);
        assert!(to(palette.accent_on_solid).contrast(solid) >= 4.5);
    }
}

#[test]
fn the_choice_follows_the_system_unless_the_person_overrides_it() {
    for system in [Some(Scheme::Light), Some(Scheme::Dark)] {
        assert_eq!(ThemeChoice::System.resolve(system), system.unwrap());
        assert_eq!(ThemeChoice::Light.resolve(system), Scheme::Light);
        assert_eq!(ThemeChoice::Dark.resolve(system), Scheme::Dark);
    }
    assert_eq!(ThemeChoice::System.resolve(None), Scheme::Dark);
    assert_eq!(ThemeChoice::default(), ThemeChoice::System);
    for choice in ThemeChoice::ALL {
        assert_eq!(ThemeChoice::parse(choice.as_str()), Some(choice));
    }
    assert_eq!(ThemeChoice::parse("sepia"), None);
}

#[test]
fn terminal_values_are_noir_in_the_product_table() {
    let resolver = Resolver::new();
    for (token, value) in [
        ("--terminal-background-color", noir::TERMINAL_BACKGROUND),
        ("--terminal-text-color", noir::TERMINAL_FOREGROUND),
        ("--terminal-cursor-color", noir::TERMINAL_CURSOR),
        (
            "--terminal-selection-background-color",
            noir::TERMINAL_SELECTION,
        ),
    ] {
        for scheme in [Scheme::Light, Scheme::Dark] {
            assert_eq!(
                resolver.color(token, scheme).map(Rgba::to_8bit),
                Some(Rgba8::rgb(value)),
                "{token} {scheme:?}"
            );
        }
    }
    for (index, value) in noir::ANSI.iter().enumerate() {
        assert_eq!(
            resolver
                .color(&format!("--terminal-ansi-{index}"), Scheme::Light)
                .map(Rgba::to_8bit),
            Some(Rgba8::rgb(*value)),
            "ansi {index}"
        );
    }
}
