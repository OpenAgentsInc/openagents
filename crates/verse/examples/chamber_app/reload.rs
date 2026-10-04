//! Renderer-only reload admission. Collision geometry and game authority stay fixed.
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Instant,
};
use verse::{
    imported::{Instance, ReloadCandidate, ReloadSource},
    ui::Atlas,
};
use verse_engine::{
    assets::{Model, Pack},
    motion::State,
};

struct ModelContract {
    states: BTreeSet<State>,
    attachments: BTreeSet<u16>,
}
pub struct Contract {
    models: BTreeMap<String, ModelContract>,
    static_geometry: BTreeMap<String, [u8; 32]>,
    sources: Option<BTreeMap<verse_engine::inventory::AssetId, String>>,
    compiler: Option<(verse_engine::inventory::AssetId, String)>,
}
fn geometry(model: &Model) -> Result<[u8; 32], String> {
    let surfaces: Vec<_> = model
        .surfaces
        .iter()
        .map(|s| {
            (
                s.vertices.iter().map(|v| v.position).collect::<Vec<_>>(),
                &s.indices,
            )
        })
        .collect();
    Ok(Sha256::digest(serde_json::to_vec(&surfaces).map_err(|e| e.to_string())?).into())
}
impl Contract {
    pub fn new(pack: &Pack, instances: &[Instance]) -> Result<Self, String> {
        let models = pack
            .models
            .iter()
            .map(|(name, model)| {
                (
                    name.clone(),
                    ModelContract {
                        states: model.states.keys().copied().collect(),
                        attachments: model.attachments.iter().map(|a| a.id).collect(),
                    },
                )
            })
            .collect();
        let static_geometry = instances
            .iter()
            .map(|i| {
                let model = pack
                    .models
                    .get(&i.model)
                    .ok_or("Missing static reload model")?;
                Ok((i.model.clone(), geometry(model)?))
            })
            .collect::<Result<_, String>>()?;
        let sources = pack
            .inventory
            .as_ref()
            .map(|i| {
                i.assets
                    .iter()
                    .filter(|a| {
                        matches!(a.binding, verse_engine::inventory::Binding::Source { .. })
                    })
                    .map(|a| Ok((a.id.clone(), verse_engine::inventory::fingerprint(a)?.0)))
                    .collect::<Result<_, String>>()
            })
            .transpose()?;
        Ok(Self {
            models,
            static_geometry,
            sources,
            compiler: pack
                .inventory
                .as_ref()
                .map(|i| (i.compiler.clone(), i.compiler_revision.clone())),
        })
    }
    pub fn validate(&self, pack: &Pack) -> Result<(), String> {
        pack.validate()?;
        let sources = pack
            .inventory
            .as_ref()
            .map(|i| {
                i.assets
                    .iter()
                    .filter(|a| {
                        matches!(a.binding, verse_engine::inventory::Binding::Source { .. })
                    })
                    .map(|a| Ok((a.id.clone(), verse_engine::inventory::fingerprint(a)?.0)))
                    .collect::<Result<_, String>>()
            })
            .transpose()?;
        if sources != self.sources
            || pack
                .inventory
                .as_ref()
                .map(|i| (i.compiler.clone(), i.compiler_revision.clone()))
                != self.compiler
        {
            return Err("Reload changes admitted source provenance".into());
        }
        if pack.inventory.is_some() {
            verse::imported::inventory::validate_model_sources(pack)?;
        }
        for (name, contract) in &self.models {
            let model = pack
                .models
                .get(name)
                .ok_or_else(|| format!("Reload removes required model: {name}"))?;
            if !contract.states.iter().all(|s| model.states.contains_key(s))
                || !contract
                    .attachments
                    .iter()
                    .all(|id| model.attachments.iter().any(|a| a.id == *id))
            {
                return Err(format!(
                    "Reload removes required animation or attachment: {name}"
                ));
            }
        }
        for (name, expected) in &self.static_geometry {
            if geometry(&pack.models[name])? != *expected {
                return Err(format!(
                    "Renderer reload cannot change collision geometry: {name}"
                ));
            }
        }
        Ok(())
    }
}
pub struct Ready {
    pub candidate: ReloadCandidate,
    pub pack: Arc<Pack>,
    pub heights: BTreeMap<String, f32>,
    pub prepare_ms: f64,
}
pub type Pending = mpsc::Receiver<Result<Ready, String>>;
/// Parsing, decoding, validation, and upload run outside the event thread.
pub fn start(
    path: PathBuf,
    source: ReloadSource,
    atlas: Arc<Atlas>,
    contract: Arc<Contract>,
    instances: Vec<Instance>,
) -> Result<Pending, String> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("verse-asset-reload".into())
        .spawn(move || {
            let started = Instant::now();
            let result = (|| {
                let pack = Pack::read(&path)?;
                contract.validate(&pack)?;
                let prepared = verse_engine::loading::Prepared::load(
                    pack,
                    path.parent().ok_or("Missing reload root")?,
                    Default::default(),
                )?;
                let candidate = source.prepare(prepared, &atlas, &instances)?;
                let pack = candidate.pack();
                let heights = pack
                    .models
                    .iter()
                    .map(|(name, model)| (name.clone(), model.height))
                    .collect();
                Ok(Ready {
                    candidate,
                    pack,
                    heights,
                    prepare_ms: started.elapsed().as_secs_f64() * 1000.,
                })
            })();
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(receive)
}
/// Saves the fully compiled runtime pack, including installed character variants.
pub fn write_manifest(path: &Path, pack: &Pack) -> Result<(), String> {
    use std::io::Write;
    pack.validate()?;
    let temporary = path.with_extension("json.pending");
    let mut writer =
        std::io::BufWriter::new(std::fs::File::create(&temporary).map_err(|e| e.to_string())?);
    #[derive(serde::Serialize)]
    struct View<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        inventory: Option<verse_engine::inventory::Inventory>,
        version: u32,
        source_revision: &'a str,
        models: &'a BTreeMap<String, Model>,
        textures: &'a [verse_engine::assets::Texture],
        placements: &'a [verse_engine::assets::Placement],
    }
    let mut inventory = pack.inventory.clone();
    if let Some(inventory) = &mut inventory {
        verse::imported::inventory::refresh(
            inventory,
            pack,
            path.parent().ok_or("Missing manifest directory")?,
        )?;
    }
    let view = View {
        inventory,
        version: pack.version,
        source_revision: &pack.source_revision,
        models: &pack.models,
        textures: &pack.textures,
        placements: &pack.placements,
    };
    serde_json::to_writer(&mut writer, &view).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}

/// Native offscreen evidence uses the same worker admission as F5.
pub fn prove(app: &mut super::App, output: &Path) -> Result<(), String> {
    let path = app
        .reload_path
        .clone()
        .ok_or("Reload proof requires an original scene")?;
    let instances = verse::imported::chamber::static_instances(
        &app.pack,
        verse_engine::source_position(app.game.scene.origin_wow),
    );
    let checkpoint = app.game.checkpoint()?;
    let frame = app.game.frame();
    let actors = verse::imported::chamber::instances(&app.pack, &frame)?;
    let old_frame = app.renderer.as_ref().unwrap().resolve_instances(&actors)?;
    let obsolete_source = app.renderer.as_ref().unwrap().reload_source();
    let receive = start(
        path.clone(),
        app.renderer.as_ref().unwrap().reload_source(),
        app.atlas.clone(),
        app.reload_contract.clone(),
        instances.clone(),
    )?;
    let ready = receive.recv().map_err(|e| e.to_string())??;
    let preparation_ms = ready.prepare_ms;
    let at = Instant::now();
    let retired = app
        .renderer
        .as_mut()
        .unwrap()
        .commit_reload(ready.candidate)?;
    let commit_ms = at.elapsed().as_secs_f64() * 1000.;
    app.pack = ready.pack;
    app.heights = ready.heights;
    std::thread::spawn(move || drop(retired));
    let view = verse::render::View {
        view_proj: frame.view_projection(1280. / 720.),
        eye: frame.eye,
    };
    let stale_frame = app
        .renderer
        .as_mut()
        .unwrap()
        .draw_resolved(
            view,
            &old_frame,
            &verse::ui::UiBatch::default(),
            &verse::imported::chamber::combat_lighting(&app.game),
        )
        .err()
        .ok_or("Reload admitted a stale frame")?;
    let unchanged = app.draw_frame()?;
    super::save_png(&output.join("reload-unchanged.png"), &unchanged)?;
    let identical = std::fs::read(output.join("player-dead.png")).map_err(|e| e.to_string())?
        == std::fs::read(output.join("reload-unchanged.png")).map_err(|e| e.to_string())?;
    if !identical {
        return Err("Unchanged asset reload changed pixels".into());
    }

    let obsolete = start(
        path.clone(),
        obsolete_source,
        app.atlas.clone(),
        app.reload_contract.clone(),
        instances.clone(),
    )?
    .recv()
    .map_err(|e| e.to_string())??;
    let stale_candidate = app
        .renderer
        .as_mut()
        .unwrap()
        .commit_reload(obsolete.candidate)
        .err()
        .ok_or("Reload admitted an obsolete candidate")?;
    let mut invalid = (*app.pack).clone();
    invalid.textures[0].sha256 = "0".repeat(64);
    let invalid_path = path.with_file_name("invalid-reload.json");
    write_manifest(&invalid_path, &invalid)?;
    drop(invalid);
    let failed_replacement = start(
        invalid_path.clone(),
        app.renderer.as_ref().unwrap().reload_source(),
        app.atlas.clone(),
        app.reload_contract.clone(),
        instances.clone(),
    )?
    .recv()
    .map_err(|e| e.to_string())?
    .err()
    .ok_or("Reload admitted an invalid texture digest")?;
    std::fs::remove_file(invalid_path).map_err(|e| e.to_string())?;
    let prepared =
        verse_engine::loading::Prepared::load((*app.pack).clone(), &app.dir, Default::default())?;
    let source = app.renderer.as_ref().unwrap().reload_source();
    let upload_instances = instances.clone();
    let failed_upload = std::thread::spawn(move || {
        let mut atlas = verse::imported::original::atlas()?;
        atlas.width = 0;
        source
            .prepare(prepared, &atlas, &upload_instances)
            .err()
            .ok_or_else(|| "Reload admitted an invalid GPU atlas".to_string())
    })
    .join()
    .map_err(|_| "GPU rejection proof worker panicked")??;
    if !failed_upload.contains("reload upload failed") {
        return Err("GPU error scope did not report the invalid atlas".into());
    }
    let after_failure = app.draw_frame()?;
    if after_failure != unchanged {
        return Err("Failed reload changed active pixels".into());
    }
    super::save_png(&output.join("reload-after-failure.png"), &after_failure)?;

    let mut changed = (*app.pack).clone();
    changed
        .models
        .get_mut("claude")
        .ok_or("Missing Claude proof model")?
        .surfaces[0]
        .tint = [0.7, 0.12, 0.08];
    write_manifest(&path, &changed)?;
    drop(changed);
    let changed = start(
        path,
        app.renderer.as_ref().unwrap().reload_source(),
        app.atlas.clone(),
        app.reload_contract.clone(),
        instances,
    )?
    .recv()
    .map_err(|e| e.to_string())??;
    let retired = app
        .renderer
        .as_mut()
        .unwrap()
        .commit_reload(changed.candidate)?;
    app.pack = changed.pack;
    app.heights = changed.heights;
    std::thread::spawn(move || drop(retired));
    let recolored = app.draw_frame()?;
    if recolored == unchanged {
        return Err("Changed asset reload did not change pixels".into());
    }
    super::save_png(&output.join("reload-changed.png"), &recolored)?;
    if app.game.checkpoint()? != checkpoint {
        return Err("Asset reload changed world authority".into());
    }
    let evidence = serde_json::json!({
        "schema":"openagents.verse.reload-proof.v1", "preparation_ms":preparation_ms, "commit_ms":commit_ms,
        "same_gpu_device":true, "unchanged_pixels_identical":identical, "stale_frame":stale_frame,
        "stale_candidate":stale_candidate, "failed_replacement":failed_replacement,
        "failed_upload":failed_upload, "failed_reload_pixels_unchanged":true, "changed_material_pixels_differ":true,
        "world_checkpoint_unchanged":true, "world_checkpoint_sha256":format!("{:x}", Sha256::digest(&checkpoint)),
        "live_surface_proven":false, "prepared_pack":app.renderer.as_ref().unwrap().pack_receipt,
    });
    std::fs::write(
        output.join("reload.json"),
        serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reload_admits_material_changes_but_fences_collision_and_required_states() {
        let dir =
            std::env::temp_dir().join(format!("verse-reload-contract-{}", std::process::id()));
        let mut pack = verse::imported::original::generate(&dir).unwrap();
        let instances = verse::imported::chamber::static_instances(&pack, glam::Vec3::ZERO);
        let contract = Contract::new(&pack, &instances).unwrap();
        pack.models.get_mut("claude").unwrap().surfaces[0].tint = [0.7, 0.12, 0.08];
        contract.validate(&pack).unwrap();
        verse::imported::inventory::compile(&mut pack, &dir, None).unwrap();
        let pinned = Contract::new(&pack, &instances).unwrap();
        if let verse_engine::inventory::Binding::Source { origin } =
            &mut pack.inventory.as_mut().unwrap().assets[0].binding
        {
            origin.license = verse_engine::inventory::License::Research;
        }
        assert!(
            pinned
                .validate(&pack)
                .unwrap_err()
                .contains("source provenance")
        );
        pack.inventory = None;
        let state = pack
            .models
            .get_mut("adventurer")
            .unwrap()
            .states
            .remove(&State::BowRelease)
            .unwrap();
        assert!(
            contract
                .validate(&pack)
                .unwrap_err()
                .contains("animation or attachment")
        );
        pack.models
            .get_mut("adventurer")
            .unwrap()
            .states
            .insert(State::BowRelease, state);
        let static_name = instances[0].model.clone();
        pack.models.get_mut(&static_name).unwrap().surfaces[0].vertices[0].position[0] += 1.;
        assert!(
            contract
                .validate(&pack)
                .unwrap_err()
                .contains("collision geometry")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

pub struct WindowProof {
    pub output: PathBuf,
    pub frames: u64,
    pub commit: Option<serde_json::Value>,
    pub before: Option<std::thread::JoinHandle<Result<(), String>>>,
    pub duration: f64,
}
impl WindowProof {
    pub fn finish(mut self, app: &super::App) -> Result<(), String> {
        let before = self
            .before
            .take()
            .ok_or("Window proof did not capture the initial frame")?;
        if !before.is_finished() {
            return Err("Window proof image encoding did not finish".into());
        }
        before
            .join()
            .map_err(|_| "Window proof image encoder panicked")??;
        let commit = self.commit.as_ref().ok_or("Window reload did not commit")?;
        if commit["world_checkpoint_unchanged"] != true {
            return Err("Window reload changed world authority".into());
        }
        let presenter = app
            .presenter
            .as_ref()
            .ok_or("Window reload has no presenter")?;
        if !presenter.current_frame_presented(app.renderer.as_ref().unwrap()) {
            return Err("Window did not present the reloaded catalog".into());
        }
        let evidence = serde_json::json!({"schema":"openagents.verse.window-reload-proof.v1", "commit":commit,
            "presented_frames":presenter.presented_frames(), "current_catalog_presented":true,
            "scripted_human_controls":true, "completed_fireballs":app.stress.as_ref().unwrap().casts,
            "duration_seconds":self.duration, "initial_hostile_casts_deferred":true,
            "prepared_pack":app.renderer.as_ref().unwrap().pack_receipt});
        std::fs::write(
            self.output.join("window-reload.json"),
            serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}

pub fn prove_inventory(app: &mut super::App, output: &Path) -> Result<(), String> {
    use verse_engine::inventory::{Binding, License, Purpose};
    let inventory = app
        .pack
        .inventory
        .as_ref()
        .ok_or("Original pack has no inventory")?;
    inventory.verify(&app.pack)?;
    let capture = inventory.admit(&app.pack, Purpose::Capture, &inventory.roots())?;
    let local_only = inventory.assets.iter().any(|a| matches!(&a.binding, Binding::Source { origin } if origin.license == License::OwnerSuppliedLocal));
    let redistribution = inventory.admit(&app.pack, Purpose::Redistribute, &inventory.roots());
    if local_only == redistribution.is_ok() {
        return Err("Inventory distribution policy disagrees with its sources".into());
    }
    let mut altered = (*app.pack).clone();
    altered.models.get_mut("claude").unwrap().surfaces[0].tint[0] = 0.01;
    let refusal = verse_engine::loading::Prepared::load(altered, &app.dir, Default::default())
        .err()
        .ok_or("Inventory admitted an unrecorded model edit")?;
    if !refusal.contains("model content") {
        return Err("Inventory did not reject a stale model fingerprint".into());
    }
    let mut relocated = (*app.pack).clone();
    let old = app.dir.join(&relocated.textures[0].file);
    let new = app.dir.join("inventory-relocated.png");
    std::fs::rename(&old, &new).map_err(|e| e.to_string())?;
    relocated.textures[0].file = "inventory-relocated.png".into();
    let loaded = verse_engine::loading::Prepared::load(relocated, &app.dir, Default::default());
    std::fs::rename(new, old).map_err(|e| e.to_string())?;
    let loaded = loaded?;
    let relocation = loaded.receipt().inventory.as_ref().unwrap();
    if relocation.assets != capture.assets {
        return Err("File relocation changed persistent identities".into());
    }
    let evidence = serde_json::json!({"schema":"openagents.verse.inventory-proof.v1", "capture":capture,
        "redistribution_allowed":redistribution.is_ok(), "redistribution_refusal":redistribution.err(),
        "unrecorded_model_edit_refusal":refusal, "relocation_preserves_ids":true,
        "prepared_pack":app.renderer.as_ref().unwrap().pack_receipt,
        "declarations_are_not_legal_attestation":true});
    std::fs::write(
        output.join("inventory.json"),
        serde_json::to_vec_pretty(&evidence).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
