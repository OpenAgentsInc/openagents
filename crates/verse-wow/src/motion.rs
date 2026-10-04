//! Explicit compatibility bindings for retained numeric animation packs.
use verse_engine::{
    assets::{Clip, Pack},
    motion::{Binding, Mode, State},
};

/// Adapts retained research models to semantic world presentation at pack admission.
/// Original pack compilers do not call this adapter.
pub fn bind(pack: &mut Pack) -> Result<(), String> {
    for model in pack
        .models
        .values_mut()
        .filter(|m| !m.bones.is_empty() && m.states.is_empty())
    {
        // Retained numeric sampling selected the first variant of a repeated ID.
        let mut ids = std::collections::BTreeSet::new();
        model.clips.retain(|clip| ids.insert(clip.id));
        if !model.clips.iter().any(|c| c.id == 0) {
            model.clips.push(Clip {
                id: 0,
                duration: 1.,
                bones: vec![],
            });
        }
        for (state, id) in [
            (State::Idle, 0),
            (State::Death, 1),
            (State::Walk, 4),
            (State::Run, 5),
            (State::Backpedal, 13),
            (State::StrafeLeft, 14),
            (State::StrafeRight, 15),
            (State::CombatReadyAlternate, 25),
            (State::Airborne, 37),
            (State::BowRelease, 46),
            (State::CombatReady, 51),
            (State::Cast, 52),
            (State::SpellRelease, 53),
            (State::Yell, 64),
            (State::Affirm, 68),
            (State::Prone, 100),
            (State::BowReady, 109),
        ] {
            model.states.insert(
                state,
                Binding {
                    clip: if model.clips.iter().any(|c| c.id == id) {
                        id
                    } else {
                        0
                    },
                    mode: if state == State::Death {
                        Mode::Hold
                    } else {
                        Mode::Loop
                    },
                    transition_seconds: if state == State::Death { 0.12 } else { 0.22 },
                },
            );
        }
    }
    pack.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_pack_admission_declares_numeric_fallbacks_without_changing_owned_bindings() {
        use verse_engine::assets::{Bone, Model};
        let mut pack = Pack {
            version: 1,
            source_revision: "test/compatibility".into(),
            textures: vec![],
            placements: vec![],
            models: std::collections::BTreeMap::from([(
                "actor".into(),
                Model {
                    states: Default::default(),
                    skin: None,
                    source: String::new(),
                    source_sha256: String::new(),
                    surfaces: vec![],
                    bones: vec![Bone {
                        parent: -1,
                        pivot: [0.; 3],
                    }],
                    clips: vec![Clip {
                        id: 4,
                        duration: 1.,
                        bones: vec![],
                    }],
                    height: 1.,
                    attachments: vec![],
                },
            )]),
        };
        pack.models.get_mut("actor").unwrap().clips.push(Clip {
            id: 4,
            duration: 2.,
            bones: vec![],
        });
        pack.validate().unwrap();
        bind(&mut pack).unwrap();
        assert_eq!(
            pack.models["actor"]
                .clips
                .iter()
                .filter(|c| c.id == 4)
                .count(),
            1
        );
        assert_eq!(
            pack.models["actor"]
                .clips
                .iter()
                .find(|c| c.id == 4)
                .unwrap()
                .duration,
            1.
        );
        assert_eq!(pack.models["actor"].states[&State::Walk].clip, 4);
        assert_eq!(pack.models["actor"].states[&State::Cast].clip, 0);
        assert_eq!(pack.models["actor"].states[&State::Death].mode, Mode::Hold);
        let saved = serde_json::to_vec(&pack).unwrap();
        bind(&mut pack).unwrap();
        assert_eq!(serde_json::to_vec(&pack).unwrap(), saved);
    }
}
