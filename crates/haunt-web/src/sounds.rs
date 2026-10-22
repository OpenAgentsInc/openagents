//! The sounds the page makes, by id: every effect, bed and music track in
//! the library has a recipe in the page's sound module, and this list is
//! what the library's tests hold it to.

/// Effects the page plays (`sound::Sound::play`).
pub const EFFECTS: [&str; 29] = [
    "creak-door",
    "creak-rocker",
    "footsteps-upstairs",
    "howl-wolf",
    "thunder",
    "crow",
    "owl",
    "hiss-cat",
    "laugh-evil",
    "cackle-witch",
    "chains",
    "heartbeat",
    "organ-sting",
    "scream-kid",
    "whisper",
    "chime-clock",
    "bubbling",
    "flutter-bat",
    "music-box",
    "candle-gust",
    "door-slam",
    "rattle-bones",
    "groan",
    "skitter",
    "gallop",
    "squeak",
    "piano-tune",
    "book-thud",
    "drip",
];

/// Ambience beds (`sound::Sound::bed`).
pub const BEDS: [&str; 6] = [
    "wind",
    "rain",
    "clock-tick",
    "fireplace",
    "cauldron",
    "thunder-distant",
];

/// Music tracks (`sound::Sound::music`).
pub const MUSIC: [&str; 1] = ["bassline-minor"];

#[cfg(test)]
mod tests {
    use super::*;
    use haunt_assets::SoundKind;

    #[test]
    fn every_library_sound_is_made_in_the_page() {
        for sound in &haunt_assets::catalog().sounds {
            let made = match sound.kind {
                SoundKind::Effect => EFFECTS.contains(&sound.id.as_str()),
                SoundKind::Bed => BEDS.contains(&sound.id.as_str()),
                SoundKind::Music => MUSIC.contains(&sound.id.as_str()),
            };
            assert!(made, "{} has no recipe", sound.id);
            assert!(sound.source.starts_with("made in the page"), "{}", sound.id);
        }
        let all = EFFECTS.len() + BEDS.len() + MUSIC.len();
        assert_eq!(all, haunt_assets::catalog().sounds.len());
    }
}
