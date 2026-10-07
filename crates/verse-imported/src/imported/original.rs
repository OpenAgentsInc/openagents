//! Original content compilation and application UI atlas.
use crate::ui::Atlas;
pub use verse_content::compiler::original::generate;
/// Builds original UI art and uses the bundled OFL font, Paper Mono.
pub fn atlas() -> Result<Atlas, String> {
    let font = crate::ui::MONO_FONT;
    let mut atlas = Atlas::from_font(font, 54.)?;
    for (name, size) in [
        ("small", 10.),
        ("combat", 28.),
        ("hotkey", 12.),
        ("numbers", 14.),
    ] {
        atlas.add_font(name, font, size * 3.)?;
    }
    for name in [
        "unit-frame",
        "elite-frame",
        "unit-name",
        "unit-skull",
        "status-bar",
        "nameplate-border",
        "action-frame",
        "bow-icon",
        "fire-bolt-icon",
        "magic-missile-icon",
        "fireball-icon",
        "misty-step-icon",
        "thunderwave-icon",
        "web-icon",
        "grease-icon",
        "light-icon",
        "shield-icon",
        "portrait-adventurer",
        "portrait-claude",
        "portrait-cultist",
        "spell-slot-empty",
    ]
    .into_iter()
    .chain(verse_world::spells::CATALOG.iter().map(|s| s.icon))
    {
        if name.ends_with("-icon") {
            let icon = super::icons::icon(name)
                .ok_or_else(|| format!("No hotbar icon is registered for {name}"))?;
            atlas.add_sprite(
                name,
                super::icons::SIZE,
                super::icons::SIZE,
                &super::icons::rasterize(icon)?,
            )?;
            continue;
        }
        let mut pixels = vec![];
        for y in 0..64 {
            for x in 0..64 {
                let border = match name {
                    "action-frame" => {
                        (14..=49).contains(&x)
                            && (14..=49).contains(&y)
                            && (x < 17 || x > 46 || y < 17 || y > 46)
                    }
                    "nameplate-border" => {
                        (1..=55).contains(&x)
                            && (36..=58).contains(&y)
                            && (x < 3 || x > 53 || y < 39 || y > 55)
                    }
                    "unit-frame" | "elite-frame" => false,
                    _ => x < 3 || y < 3 || x > 60 || y > 60,
                };
                let color = if border {
                    [140, 110, 65, 255]
                } else if name == "status-bar" {
                    [255; 4]
                } else if name == "nameplate-border"
                    || name == "action-frame"
                    || name == "unit-frame"
                    || name == "elite-frame"
                {
                    [0; 4]
                } else {
                    [18, 21, 29, 255]
                };
                pixels.extend(color);
            }
        }
        atlas.add_sprite(name, 64, 64, &pixels)?;
    }
    atlas.use_logical_metrics(3.);
    Ok(atlas)
}

#[cfg(test)]
mod tests {
    #[test]
    fn application_atlas_keeps_all_original_icons_and_fonts() {
        super::atlas().unwrap();
    }
}
