#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackagedRiveAsset {
    pub id: &'static str,
    pub file_name: &'static str,
    pub runtime_path: &'static str,
    pub description: &'static str,
    pub default_artboard: &'static str,
    pub default_scene: &'static str,
    pub bytes: &'static [u8],
}

pub const SIMPLE_FUI_HUD_ASSET_ID: &str = "simple_fui_hud";
pub const SIMPLE_FUI_HUD_FILE_NAME: &str = "simple-fui-hud.riv";
pub const SIMPLE_FUI_HUD_RUNTIME_PATH: &str = "resources/rive/simple-fui-hud.riv";
pub const SIMPLE_FUI_HUD_FIXTURE_B_ASSET_ID: &str = "simple_fui_hud_fixture_b";
pub const SIMPLE_FUI_HUD_FIXTURE_B_FILE_NAME: &str = "simple-fui-hud-fixture-b.riv";
pub const SIMPLE_FUI_HUD_FIXTURE_B_RUNTIME_PATH: &str =
    "resources/rive/simple-fui-hud-fixture-b.riv";

const SIMPLE_FUI_HUD_BYTES: &[u8] = include_bytes!("../resources/rive/simple-fui-hud.riv");
const SIMPLE_FUI_HUD_FIXTURE_B_BYTES: &[u8] =
    include_bytes!("../resources/rive/simple-fui-hud-fixture-b.riv");

const PACKAGED_RIVE_ASSETS: [PackagedRiveAsset; 2] = [
    PackagedRiveAsset {
        id: SIMPLE_FUI_HUD_ASSET_ID,
        file_name: SIMPLE_FUI_HUD_FILE_NAME,
        runtime_path: SIMPLE_FUI_HUD_RUNTIME_PATH,
        description: "Primary packaged simple FUI HUD asset.",
        default_artboard: "default",
        default_scene: "default",
        bytes: SIMPLE_FUI_HUD_BYTES,
    },
    PackagedRiveAsset {
        id: SIMPLE_FUI_HUD_FIXTURE_B_ASSET_ID,
        file_name: SIMPLE_FUI_HUD_FIXTURE_B_FILE_NAME,
        runtime_path: SIMPLE_FUI_HUD_FIXTURE_B_RUNTIME_PATH,
        description: "Second deterministic packaged fixture using the same HUD bytes to exercise multi-asset bring-up.",
        default_artboard: "default",
        default_scene: "default",
        bytes: SIMPLE_FUI_HUD_FIXTURE_B_BYTES,
    },
];

pub fn simple_fui_hud_asset() -> PackagedRiveAsset {
    packaged_rive_asset(SIMPLE_FUI_HUD_ASSET_ID).expect("simple_fui_hud must stay in the manifest")
}

pub fn simple_fui_hud_bytes() -> &'static [u8] {
    SIMPLE_FUI_HUD_BYTES
}

pub fn default_packaged_rive_asset() -> PackagedRiveAsset {
    PACKAGED_RIVE_ASSETS[0]
}

pub fn packaged_rive_assets() -> &'static [PackagedRiveAsset] {
    &PACKAGED_RIVE_ASSETS
}

pub fn packaged_rive_asset(id: &str) -> Option<PackagedRiveAsset> {
    PACKAGED_RIVE_ASSETS
        .iter()
        .copied()
        .find(|asset| asset.id == id)
}

pub fn next_packaged_rive_asset(current_id: &str) -> PackagedRiveAsset {
    cycle_packaged_rive_asset(current_id, 1)
}

pub fn previous_packaged_rive_asset(current_id: &str) -> PackagedRiveAsset {
    cycle_packaged_rive_asset(current_id, -1)
}

fn cycle_packaged_rive_asset(current_id: &str, delta: isize) -> PackagedRiveAsset {
    let current_index = PACKAGED_RIVE_ASSETS
        .iter()
        .position(|asset| asset.id == current_id)
        .unwrap_or(0) as isize;
    let asset_count = PACKAGED_RIVE_ASSETS.len() as isize;
    let next_index = (current_index + delta).rem_euclid(asset_count) as usize;
    PACKAGED_RIVE_ASSETS[next_index]
}

#[cfg(test)]
mod tests {
    use super::{
        SIMPLE_FUI_HUD_FIXTURE_B_ASSET_ID, next_packaged_rive_asset, packaged_rive_asset,
        packaged_rive_assets, previous_packaged_rive_asset, simple_fui_hud_asset,
        simple_fui_hud_bytes,
    };
    use wgpui::{Bounds, RiveController};

    #[test]
    fn packaged_hud_asset_has_runtime_metadata() {
        let asset = simple_fui_hud_asset();
        assert_eq!(asset.file_name, "simple-fui-hud.riv");
        assert_eq!(asset.runtime_path, "resources/rive/simple-fui-hud.riv");
        assert_eq!(asset.default_artboard, "default");
        assert_eq!(asset.default_scene, "default");
        assert!(!asset.bytes.is_empty());
    }

    #[test]
    fn packaged_hud_asset_renders_a_first_frame() {
        let mut controller = RiveController::from_bytes(simple_fui_hud_bytes())
            .expect("packaged HUD asset should instantiate");
        let batch = controller.render_batch(Bounds::new(0.0, 0.0, 480.0, 320.0));
        assert!(
            !batch.commands.is_empty() || !batch.images.is_empty(),
            "packaged HUD asset should emit vector or image content",
        );
        assert_eq!(controller.metrics().scene_name, "default");
        assert!(controller.metrics().artboard_size.width > 0.0);
        assert!(controller.metrics().artboard_size.height > 0.0);
    }

    #[test]
    fn packaged_registry_exposes_second_fixture_and_wraps() {
        let assets = packaged_rive_assets();
        assert_eq!(assets.len(), 2);
        let second = packaged_rive_asset(SIMPLE_FUI_HUD_FIXTURE_B_ASSET_ID)
            .expect("fixture asset should resolve from the manifest");
        assert!(second.file_name.ends_with("fixture-b.riv"));
        assert_eq!(
            next_packaged_rive_asset(second.id).id,
            simple_fui_hud_asset().id,
            "cycling forward from the second asset should wrap to the default asset"
        );
        assert_eq!(
            previous_packaged_rive_asset(simple_fui_hud_asset().id).id,
            second.id,
            "cycling backward from the default asset should wrap to the second asset"
        );
    }

    #[test]
    fn packaged_second_fixture_renders_a_first_frame() {
        let asset = packaged_rive_asset(SIMPLE_FUI_HUD_FIXTURE_B_ASSET_ID)
            .expect("fixture asset should resolve");
        let mut controller =
            RiveController::from_bytes(asset.bytes).expect("fixture asset should instantiate");
        let batch = controller.render_batch(Bounds::new(0.0, 0.0, 480.0, 320.0));
        assert!(
            !batch.commands.is_empty() || !batch.images.is_empty(),
            "fixture asset should emit vector or image content",
        );
    }
}
