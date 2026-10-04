//! Authored dialogue over current, server-admitted giver interactions.
use crate::ui::{Atlas, UiBatch};
use verse_world::service::{progression::Marker, view::View};

#[derive(Default)]
pub struct Panel {
    quest: usize,
    page: usize,
}
#[derive(Debug, PartialEq)]
pub enum Action {
    Accept(u64, verse_engine::core::LifeId),
    Claim(u64),
    Close,
}
fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[0] < r[0] + r[2] && p[1] >= r[1] && p[1] < r[1] + r[3]
}
fn lines(text: &str, columns: usize) -> Vec<String> {
    let mut result = Vec::new();
    let columns = columns.max(1);
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.len() + word.len() + 1 > columns {
                result.push(std::mem::take(&mut line));
            }
            for chunk in word.as_bytes().chunks(columns) {
                let chunk = std::str::from_utf8(chunk).unwrap();
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(chunk);
                if line.len() >= columns {
                    result.push(std::mem::take(&mut line));
                }
            }
        }
        result.push(line);
    }
    result
}
impl Panel {
    pub fn reset(&mut self) {
        self.quest = 0;
        self.page = 0;
    }
    pub fn contains(&self, view: &View, point: [f32; 2]) -> bool {
        view.interaction().is_some() && inside([20., 160., 500., 420.], point)
    }
    pub fn draw(&mut self, ui: &mut UiBatch, atlas: &Atlas, view: &View) {
        let Some(interaction) = view.interaction() else {
            return;
        };
        let quests = view.interaction_quests();
        if quests.is_empty() {
            return;
        }
        self.quest = self.quest.min(quests.len() - 1);
        let quest = quests[self.quest];
        let name = view
            .replica()
            .latest()
            .and_then(|s| {
                s.presentation
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == interaction.giver)
            })
            .map_or("Quest giver", |p| p.actor.name.as_str());
        ui.rect(atlas, 20., 160., 500., 420., [0.06, 0.035, 0.02, 1.]);
        ui.frame(atlas, 20., 160., 500., 420., 2., [0.7, 0.54, 0.27, 1.]);
        let gold = [1., 0.82, 0.45, 1.];
        ui.text(atlas, 36., 174., name, gold);
        ui.text(atlas, 36., 204., &quest.name, gold);
        let text = lines(
            quest
                .dialogue_text()
                .unwrap_or("Complete the objective and return."),
            46,
        );
        let mut text = text;
        for item in &quest.items {
            text.extend(lines(
                &format!("Reward: item {} x{}", item.id, item.count),
                46,
            ));
        }
        self.page = self.page.min(text.len().saturating_sub(1) / 10);
        for (i, line) in text.iter().skip(self.page * 10).take(10).enumerate() {
            ui.text(
                atlas,
                36.,
                238. + i as f32 * 21.,
                line,
                [0.92, 0.85, 0.7, 1.],
            );
        }
        ui.text(
            atlas,
            36.,
            454.,
            &format!(
                "Objective: {} / {}   XP: {}",
                quest.progress, quest.goal, quest.experience
            ),
            gold,
        );
        ui.text(
            atlas,
            36.,
            480.,
            &format!(
                "Quest {} / {}   Text {} / {}",
                self.quest + 1,
                quests.len(),
                self.page + 1,
                text.len().div_ceil(10)
            ),
            gold,
        );
        for (x, label) in [
            (36., "< Quest"),
            (140., "Quest >"),
            (250., "< Text"),
            (350., "Text >"),
        ] {
            ui.text(atlas, x, 510., label, gold);
        }
        let label = match quest.marker() {
            Some(Marker::Available) => "Accept",
            Some(Marker::TurnIn) => "Complete",
            _ => "In progress",
        };
        ui.text(atlas, 36., 550., label, gold);
        ui.text(atlas, 430., 550., "Close", gold);
    }
    pub fn click(&mut self, view: &View, point: [f32; 2]) -> Option<Action> {
        if !self.contains(view, point) {
            return None;
        }
        let quests = view.interaction_quests();
        if inside([420., 540., 90., 30.], point) {
            return Some(Action::Close);
        }
        if inside([36., 500., 400., 30.], point) {
            match point[0] as u32 {
                0..140 => {
                    self.quest = self.quest.saturating_sub(1);
                    self.page = 0;
                }
                140..250 => {
                    self.quest = (self.quest + 1).min(quests.len().saturating_sub(1));
                    self.page = 0;
                }
                250..350 => self.page = self.page.saturating_sub(1),
                _ => self.page = self.page.saturating_add(1),
            }
            return None;
        }
        let quest = quests.get(self.quest.min(quests.len().saturating_sub(1)))?;
        if !inside([36., 540., 150., 30.], point) || !quest.interactable {
            return None;
        }
        match quest.marker()? {
            Marker::Available => Some(Action::Accept(quest.id, quest.giver_life?)),
            Marker::TurnIn => Some(Action::Claim(quest.id)),
            Marker::Active => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        gateway: verse_world::service::auth::Gateway,
        connection: verse_world::service::auth::ConnectionId,
        request: u64,
        view: View,
        panel: Panel,
    }
    impl Fixture {
        fn new() -> Self {
            Self::with_pack(None)
        }
        fn with_pack(pack: Option<&verse_engine::assets::Pack>) -> Self {
            use verse_world::service::{Chamber, auth::Gateway};
            let scene = verse_engine::director::Scene::from_json(include_bytes!(
                "../../../../assets/verse/original/ritual-quests.json"
            ))
            .unwrap();
            let mut game = verse_world::play::Game::combat_in(scene, false, 120).unwrap();
            if let Some(pack) = pack {
                if pack.source_revision == "verse-bestiary-ritual-v1" {
                    game.scene
                        .actors
                        .iter_mut()
                        .find(|a| a.id == 1)
                        .unwrap()
                        .scale = 6. / (pack.models["claude"].height * 0.9144);
                }
                super::super::props::admit_collision(pack, &mut game).unwrap();
            }
            game.time = game.scene.cut_at;
            game.tick(0., [0.; 2]).unwrap();
            game.encounter
                .as_mut()
                .unwrap()
                .postpone_casts_until(600.)
                .unwrap();
            let mut gateway = Gateway::new(Chamber::new(game).unwrap())
                .unwrap()
                .with_progression(
                    serde_json::from_slice(include_bytes!(
                        "../../../../assets/verse/original/ritual-progression.json"
                    ))
                    .unwrap(),
                )
                .unwrap()
                .with_rewards(
                    serde_json::from_slice(include_bytes!(
                        "../../../../assets/verse/original/ritual-rewards.json"
                    ))
                    .unwrap(),
                )
                .unwrap();
            let key = secp256k1::Keypair::from_secret_key(
                &secp256k1::Secp256k1::new(),
                &secp256k1::SecretKey::from_byte_array([131; 32]).unwrap(),
            );
            let public = key.x_only_public_key().0.serialize();
            gateway.enroll_primary(public).unwrap();
            let (connection, challenge) = gateway.open(0).unwrap();
            gateway
                .authenticate(
                    connection,
                    0,
                    public,
                    secp256k1::Secp256k1::new()
                        .sign_schnorr_no_aux_rand(&challenge.signing_digest(public), &key)
                        .to_byte_array(),
                )
                .unwrap();
            let mut fixture = Self {
                gateway,
                connection,
                request: 0,
                view: View::new(120, 12., 0).unwrap(),
                panel: Panel::default(),
            };
            fixture.refresh();
            fixture
                .view
                .open_giver(fixture.view.quest_markers().keys().next().copied().unwrap())
                .unwrap();
            fixture
        }
        fn send(
            &mut self,
            body: verse_world::service::wire::Body,
        ) -> verse_world::service::wire::Response {
            use verse_world::service::wire::{Request, VERSION};
            self.request += 1;
            let bytes = serde_json::to_vec(&Request {
                version: VERSION,
                request_id: self.request,
                body,
            })
            .unwrap();
            serde_json::from_slice(
                &self
                    .gateway
                    .dispatch_json(self.connection, 0, &bytes)
                    .unwrap(),
            )
            .unwrap()
        }
        fn refresh(&mut self) {
            use verse_world::service::wire::Body;
            let response = self.send(Body::Snapshot {});
            self.view.push_snapshot(&response).unwrap();
            let response = self.send(Body::Inventory {});
            self.view.push_inventory(&response).unwrap();
        }
        fn action(&mut self) -> verse_world::service::wire::Response {
            use verse_world::service::wire::Body;
            let own = self.gateway.admission(self.connection).unwrap();
            let body = match self.panel.click(&self.view, [60., 555.]).unwrap() {
                Action::Accept(quest, giver) => Body::AcceptQuest {
                    life: own.actor().into(),
                    epoch: own.epoch(),
                    quest,
                    giver: giver.into(),
                },
                Action::Claim(quest) => Body::ClaimQuest {
                    life: own.actor().into(),
                    epoch: own.epoch(),
                    quest,
                },
                Action::Close => panic!("Expected quest action"),
            };
            self.send(body)
        }
        fn defeat(&mut self, actor: u64) {
            use verse_world::Intent;
            let target = self
                .view
                .replica()
                .latest()
                .unwrap()
                .presentation
                .actors
                .iter()
                .find(|a| a.life.actor == actor)
                .unwrap()
                .life
                .into();
            for _ in 0..3 {
                if self
                    .view
                    .replica()
                    .latest()
                    .unwrap()
                    .presentation
                    .actors
                    .iter()
                    .all(|a| a.life.actor != actor || a.health <= 0)
                {
                    break;
                }
                let command = self
                    .gateway
                    .admission(self.connection)
                    .unwrap()
                    .command(
                        self.gateway.game().authority_tick,
                        Intent::Cast {
                            ability: verse_world::play::Ability::MagicMissile,
                            target: Some(target),
                            aim: [0., 0., 1.],
                        },
                    )
                    .unwrap();
                self.gateway.submit(self.connection, command).unwrap();
                for _ in 0..120 {
                    self.gateway.tick(1. / 30.).unwrap();
                }
                self.refresh();
            }
            assert!(
                self.view
                    .replica()
                    .latest()
                    .unwrap()
                    .presentation
                    .actors
                    .iter()
                    .all(|a| a.life.actor != actor || a.health <= 0),
                "Target was not defeated: {:?}",
                self.view.replica().latest().unwrap()
            );
        }
    }
    #[test]
    fn authored_dialogue_buttons_follow_real_combat_and_retry_safe_turn_in() {
        use verse_world::service::wire::Reply;
        let mut f = Fixture::new();
        assert_eq!(
            f.view.interaction_quests()[0].marker(),
            Some(Marker::Available)
        );
        assert!(matches!(
            f.action().body,
            Reply::QuestAccepted { quest: 101, .. }
        ));
        f.refresh();
        assert_eq!(
            f.view.interaction_quests()[0].marker(),
            Some(Marker::Active)
        );
        assert_eq!(f.panel.click(&f.view, [60., 555.]), None);
        f.defeat(2);
        assert_eq!(
            f.view.interaction_quests()[0].marker(),
            Some(Marker::TurnIn)
        );
        assert!(matches!(
            f.action().body,
            Reply::QuestClaimed { quest: 101, .. }
        ));
        f.refresh();
        assert_eq!(f.view.inventory().unwrap().experience, 75);
        assert_eq!(f.view.interaction_quests()[0].id, 102);
        assert!(matches!(
            f.action().body,
            Reply::QuestAccepted { quest: 102, .. }
        ));
        f.refresh();
        f.defeat(3);
        assert_eq!(
            f.view.interaction_quests()[0].marker(),
            Some(Marker::TurnIn)
        );
        assert!(matches!(
            f.action().body,
            Reply::QuestClaimed { quest: 102, .. }
        ));
        let own = f.gateway.admission(f.connection).unwrap();
        assert!(matches!(
            f.send(verse_world::service::wire::Body::ClaimQuest {
                life: own.actor().into(),
                epoch: own.epoch(),
                quest: 102
            })
            .body,
            Reply::QuestClaimed { quest: 102, .. }
        ));
        f.refresh();
        assert_eq!(f.view.inventory().unwrap().experience, 175);
        assert!(f.view.interaction().is_none());
        assert!(!f.panel.contains(&f.view, [60., 555.]));
    }
    #[test]
    #[ignore = "Explicit GPU quest dialogue acceptance capture"]
    fn capture_authored_giver_dialogue() {
        let output = std::path::PathBuf::from(
            std::env::var_os("VERSE_GIVER_CAPTURE").expect("Explicit capture directory"),
        );
        std::fs::create_dir_all(&output).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let mut pack = super::super::original::generate(dir.path()).unwrap();
        let dressed = std::env::var_os("VERSE_GIVER_DRESSED").is_some();
        if dressed {
            let root =
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse");
            let characters = dir.path().join("source-characters");
            super::super::inventory::snapshot_characters(
                &root.join("characters/quaternius"),
                &characters,
            )
            .unwrap();
            super::super::characters::install(&mut pack, dir.path(), &characters, "male-ranger")
                .unwrap();
            pack.source_revision = "verse-universal-ritual-v1".into();
            let props = dir.path().join("source-props");
            super::super::inventory::snapshot_props(&root.join("props/quaternius"), &props)
                .unwrap();
            super::super::props::install(&mut pack, dir.path(), &props).unwrap();
            if let Some(source) = std::env::var_os("VERSE_GIVER_BESTIARY") {
                let frozen = dir.path().join("Puglin.glb");
                super::super::inventory::snapshot_bestiary(std::path::Path::new(&source), &frozen)
                    .unwrap();
                super::super::characters::install_bestiary(
                    &mut pack,
                    dir.path(),
                    &frozen,
                    &characters.join("animations.glb"),
                )
                .unwrap();
                super::super::inventory::compile(&mut pack, dir.path(), Some(&frozen)).unwrap();
            } else {
                super::super::inventory::compile(&mut pack, dir.path(), None).unwrap();
            }
        }
        let atlas = super::super::original::atlas().unwrap();
        let mut renderer = super::super::Renderer::new(
            pack.clone(),
            dir.path(),
            1920,
            1080,
            &atlas,
            &super::super::chamber::static_instances(&pack, glam::Vec3::ZERO),
        )
        .unwrap();
        renderer.set_overlay_size(1280., 720.);
        let eye = glam::Vec3::new(0., 4., -28.);
        let camera = crate::render::View {
            view_proj: glam::Mat4::perspective_rh(60f32.to_radians(), 1280. / 720., 0.1, 100.)
                * glam::Mat4::look_at_rh(eye, glam::Vec3::new(0., 1., -20.), glam::Vec3::Y),
            eye,
        };
        let mut f = Fixture::with_pack(if dressed { Some(&pack) } else { None });
        for stage in ["offer", "active", "turn-in"] {
            if stage == "active" {
                f.action();
                f.refresh();
            }
            if stage == "turn-in" {
                f.defeat(2);
            }
            let frame = f
                .view
                .frame(
                    1.,
                    verse_world::service::view::Camera {
                        eye,
                        target: glam::Vec3::new(0., 1., -20.),
                        fov: 60.,
                    },
                )
                .unwrap()
                .unwrap();
            let mut ui = super::super::overlay::cinematic_with_markers(
                &atlas,
                &frame,
                &pack
                    .models
                    .iter()
                    .map(|(id, m)| (id.clone(), m.height))
                    .collect(),
                camera.view_proj,
                1280.,
                720.,
                &f.view.quest_markers(),
            );
            f.panel.draw(&mut ui, &atlas, &f.view);
            let rendered = super::super::chamber::remote_scene(
                &pack,
                &f.view,
                1.,
                verse_world::service::view::Camera {
                    eye,
                    target: glam::Vec3::new(0., 1., -20.),
                    fov: 60.,
                },
                glam::Vec3::ZERO,
                false,
                f.gateway.game().player,
            )
            .unwrap()
            .unwrap();
            let pixels = renderer
                .draw(camera, &rendered.instances, &ui, &rendered.lighting)
                .unwrap();
            let mut encoder = png::Encoder::new(
                std::fs::File::create(output.join(format!("{stage}.png"))).unwrap(),
                1920,
                1080,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
    }
    #[test]
    fn dialogue_wrap_retains_words_and_long_tokens() {
        assert_eq!(
            lines("one two three\nfour", 7),
            vec!["one two", "three", "four"]
        );
        assert_eq!(lines("abcdefghijk", 4), vec!["abcd", "efgh", "ijk"]);
    }
}
