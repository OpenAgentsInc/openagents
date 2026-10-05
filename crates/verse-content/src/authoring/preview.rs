//! Read-only scene views and an independently owned, admitted preview authority.
use super::*;
use glam::{DVec3, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::path::Path;
use verse_world::service::auth::Gateway;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewReport {
    pub schema: String,
    pub zone: String,
    pub content: [u8; 32],
    pub tick: u64,
    pub actors: Vec<serde_json::Value>,
    pub quests: Vec<verse_world::service::progression::Progress>,
    pub timeline: Vec<TimelineCue>,
    pub yell: Option<verse_engine::director::Cue>,
    pub render_instances: usize,
    pub geometry: physics::queries::SceneSnapshot,
    pub navigation: Option<serde_json::Value>,
}
/// Candidate reload must finish before it can replace this local running world.
pub struct Preview {
    document: Document,
    pack: Pack,
    scene: Scene,
    gateway: Gateway,
    content: [u8; 32],
    tick: u64,
}
impl Preview {
    pub fn new(doc: &Document, base: &Pack, assets: &Path) -> Result<Self> {
        let (pack, scene, gateway) = admit(doc, base)?;
        let mut content = checked(
            "assets",
            crate::remote_content::admit(&pack, &scene, assets, &doc.outfits, &doc.equipment),
        )?;
        content = checked("authored", doc.authored.bind_content(content))?;
        content = checked(
            "gameplay",
            verse_world::content::bind_gameplay(
                content,
                &verse_world::content::Gameplay {
                    authored_combat_health: doc.social_profile.is_none(),
                    rewards: &doc.rewards,
                    progression: &doc.progression,
                    items: &doc.items,
                    outfits: &doc.outfits,
                    equipment: &doc.equipment,
                },
            ),
        )?;
        if let Some(profile) = &doc.social_profile {
            use sha2::{Digest, Sha256};
            let mut digest = Sha256::new();
            digest.update(b"verse.hosted.social.content.v1\0");
            digest.update(content);
            digest.update(checked("social_profile", profile.digest())?);
            content = digest.finalize().into();
        }
        let gateway = checked("content", gateway.with_content(content))?;
        let preview = Self {
            document: doc.clone(),
            pack,
            scene,
            gateway,
            content,
            tick: 0,
        };
        preview.report(None)?;
        Ok(preview)
    }
    pub fn content(&self) -> [u8; 32] {
        self.content
    }
    pub fn gateway(&self) -> &Gateway {
        &self.gateway
    }
    pub fn reload(&mut self, doc: &Document, base: &Pack, assets: &Path) -> Result<()> {
        let candidate = Self::new(doc, base, assets)?;
        *self = candidate;
        Ok(())
    }
    pub fn step(&mut self, ticks: u32) -> Result<PreviewReport> {
        if ticks > 18_000 {
            return Err(Diagnostic::at(
                "preview",
                "ticks",
                "Preview steps must be at most 18000 ticks",
            ));
        }
        for _ in 0..ticks {
            checked("preview", self.gateway.tick(1. / 30.))?;
            self.tick += 1;
        }
        self.report(None)
    }
    /// Compile capsule clearance over the same query geometry the authority uses.
    pub fn report(&self, navigation: Option<f64>) -> Result<PreviewReport> {
        let frame = self.gateway.game().frame();
        let mut instances: Vec<_> = frame
            .actors
            .iter()
            .filter(|a| a.visible)
            .map(|a| verse_engine::presentation::Instance {
                mount: None,
                actor: a.life,
                model: a.actor.model.clone(),
                transform: Mat4::from_translation(a.actor.position)
                    * Mat4::from_rotation_y(a.actor.yaw)
                    * Mat4::from_scale(Vec3::splat(a.actor.scale))
                    * crate::basis(),
                animation: a.animation,
                time: a.animation_time,
                animation_epoch: None,
                emission: Vec3::ONE,
            })
            .collect();
        instances.extend(self.pack.placements.iter().map(|p| {
            verse_engine::presentation::Instance {
                mount: None,
                actor: None,
                model: p.model.clone(),
                transform: crate::basis()
                    * Mat4::from_scale_rotation_translation(
                        Vec3::splat(p.scale),
                        Quat::from_array(p.rotation),
                        p.position.into(),
                    ),
                animation: 0.into(),
                time: 0.,
                animation_epoch: None,
                emission: Vec3::ONE,
            }
        }));
        let catalog = checked("pack", verse_engine::residency::Catalog::new(&self.pack))?;
        let lighting = verse_engine::lighting::Lighting::default();
        let render = checked(
            "preview.render",
            verse_engine::render_world::RenderWorld::extract(
                &catalog,
                verse_engine::presentation::View {
                    view_proj: Mat4::IDENTITY,
                    eye: Vec3::ZERO,
                },
                &instances,
                &[],
                &lighting,
            ),
        )?;
        checked("preview.render", render.validate(&catalog))?;
        let navigation = if let Some(half) = navigation {
            if !half.is_finite() || !(2. ..=64.).contains(&half) {
                return Err(Diagnostic::at(
                    "preview",
                    "navigation.half",
                    "Choose a half extent from 2 to 64 meters",
                ));
            }
            let nav = checked(
                "preview.navigation",
                self.gateway.game().navigation_preview(
                    DVec3::new(-half, -5., -half),
                    DVec3::new(half, 10., half),
                    1.,
                ),
            )?;
            Some(
                serde_json::json!({"half":half,"cell":1.0,"stats":nav.stats,"nodes":nav.nodes(),"tiles":nav.tiles()}),
            )
        } else {
            None
        };
        let actors = frame
            .actors
            .iter()
            .map(|a| {
                serde_json::json!({"id":a.actor.id, "life":a.life,
            "name":a.actor.name,"model":a.actor.model,"position":a.actor.position,"health":a.health,
            "animation":a.animation,"visible":a.visible})
            })
            .collect();
        Ok(PreviewReport {
            schema: "verse.author.preview.v1".into(),
            zone: self.document.zone.clone(),
            content: self.content,
            tick: self.tick,
            actors,
            quests: self
                .gateway
                .quest_log(self.gateway.game().player_life().actor),
            timeline: self.document.timeline.clone(),
            yell: frame.yell,
            render_instances: instances.len(),
            geometry: checked(
                "preview.collision",
                self.gateway.game().collision_geometry(),
            )?,
            navigation,
        })
    }
    /// A standalone scene, collision, clearance, and timeline view; no JavaScript.
    pub fn svg(&self, navigation: Option<f64>) -> Result<String> {
        let report = self.report(navigation)?;
        let half = navigation.unwrap_or(32.);
        let point = |x: f64, z: f64| {
            (
                (x / (half * 2.) + 0.5) * 780. + 10.,
                (z / (half * 2.) + 0.5) * 600. + 70.,
            )
        };
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"900\" viewBox=\"0 0 1000 900\"><rect width=\"1000\" height=\"900\" fill=\"#101724\"/><g font-family=\"sans-serif\" font-size=\"14\" fill=\"#e4e9f0\"><text x=\"20\" y=\"25\">{} · authority tick {}</text><text x=\"20\" y=\"48\">Meters in world X/Z · amber: collision · cyan: admitted navigation spans · green: friendly NPC</text>",
            escape(&report.zone),
            report.tick
        );
        if let Some(nav) = &report.navigation {
            if let Some(nodes) = nav["nodes"].as_array() {
                for node in nodes {
                    if let Some(p) = node["feet"].as_array() {
                        let (x, z) =
                            point(p[0].as_f64().unwrap_or(0.), p[2].as_f64().unwrap_or(0.));
                        svg.push_str(&format!(
                            "<circle cx=\"{x}\" cy=\"{z}\" r=\"2\" fill=\"#51cddd\"/>"
                        ));
                    }
                }
            }
        }
        for shape in &report.geometry.colliders {
            use physics::queries::GeometrySnapshot;
            match &shape.geometry {
                GeometrySnapshot::Box { min, max } => {
                    let mut low = DVec3::splat(f64::INFINITY);
                    let mut high = DVec3::splat(f64::NEG_INFINITY);
                    for x in [min.x, max.x] {
                        for y in [min.y, max.y] {
                            for z in [min.z, max.z] {
                                let world =
                                    shape.pose.rotation * DVec3::new(x, y, z) + shape.pose.position;
                                low = low.min(world);
                                high = high.max(world);
                            }
                        }
                    }
                    let (x, z) = point(low.x, low.z);
                    let (xx, zz) = point(high.x, high.z);
                    svg.push_str(&format!("<rect x=\"{x}\" y=\"{z}\" width=\"{}\" height=\"{}\" fill=\"#a6813320\" stroke=\"#e0aa45\"/><title>Collider {}</title>",xx-x,zz-z,shape.key.life.entity));
                }
                GeometrySnapshot::Triangles { triangles } => {
                    for triangle in triangles {
                        let points = triangle
                            .0
                            .iter()
                            .map(|p| {
                                let p = shape.pose.rotation * *p + shape.pose.position;
                                let (x, z) = point(p.x, p.z);
                                format!("{x},{z}")
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        svg.push_str(&format!("<polygon points=\"{points}\" fill=\"none\" stroke=\"#a68133\" stroke-width=\"0.5\"/>"));
                    }
                }
                GeometrySnapshot::Capsule { .. } => (),
            }
        }
        for p in &self.pack.placements {
            let pos = crate::basis().transform_point3(p.position.into());
            let (x, z) = point(pos.x as f64, pos.z as f64);
            svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"5\" height=\"5\" fill=\"#8c9cb0\"><title>{}</title></rect>",x-2.5,z-2.5,escape(&p.model)));
        }
        for actor in &report.actors {
            let id = actor["id"].as_u64().unwrap_or(0);
            let p = &actor["position"];
            let (x, z) = point(p[0].as_f64().unwrap_or(0.), p[2].as_f64().unwrap_or(0.));
            let color = if self.scene.actors.iter().any(|a| a.id == id && a.friendly) {
                "#69df9e"
            } else {
                "#e68b8b"
            };
            svg.push_str(&format!("<circle cx=\"{x}\" cy=\"{z}\" r=\"5\" fill=\"{color}\"/><text x=\"{}\" y=\"{}\">{}: {}</text>",x+7.,z, id, escape(actor["name"].as_str().unwrap_or(""))));
        }
        svg.push_str("<text x=\"20\" y=\"710\">Authored timeline (seconds; cues retain their stable IDs)</text><line x1=\"20\" y1=\"735\" x2=\"980\" y2=\"735\" stroke=\"#8c9cb0\"/>");
        for (index, cue) in self.document.timeline.iter().enumerate() {
            let x = 20. + 960. * cue.cue.at / self.scene.duration;
            svg.push_str(&format!("<circle cx=\"{x}\" cy=\"735\" r=\"4\" fill=\"#51cddd\"/><text x=\"{x}\" y=\"{}\">#{} @ {:.2}s · actor {}</text>",760+index%5*22,cue.id,cue.cue.at,cue.cue.actor));
        }
        svg.push_str("</g></svg>");
        Ok(svg)
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
