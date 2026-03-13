#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackagedRiveAsset {
    pub id: &'static str,
    pub file_name: &'static str,
    pub runtime_path: &'static str,
    pub bytes: &'static [u8],
}

pub const SIMPLE_FUI_HUD_ASSET_ID: &str = "simple_fui_hud";
pub const SIMPLE_FUI_HUD_FILE_NAME: &str = "simple-fui-hud.riv";
pub const SIMPLE_FUI_HUD_RUNTIME_PATH: &str = "resources/rive/simple-fui-hud.riv";

const SIMPLE_FUI_HUD_BYTES: &[u8] = include_bytes!("../resources/rive/simple-fui-hud.riv");

pub fn simple_fui_hud_asset() -> PackagedRiveAsset {
    PackagedRiveAsset {
        id: SIMPLE_FUI_HUD_ASSET_ID,
        file_name: SIMPLE_FUI_HUD_FILE_NAME,
        runtime_path: SIMPLE_FUI_HUD_RUNTIME_PATH,
        bytes: SIMPLE_FUI_HUD_BYTES,
    }
}

pub fn simple_fui_hud_bytes() -> &'static [u8] {
    SIMPLE_FUI_HUD_BYTES
}

#[cfg(test)]
mod tests {
    use super::{simple_fui_hud_asset, simple_fui_hud_bytes};
    use wgpui::{Bounds, RiveController};

    #[test]
    fn packaged_hud_asset_has_runtime_metadata() {
        let asset = simple_fui_hud_asset();
        assert_eq!(asset.file_name, "simple-fui-hud.riv");
        assert_eq!(asset.runtime_path, "resources/rive/simple-fui-hud.riv");
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
}
