# Firm reason: 6 (Blender's FBX and glTF SDK operators have no C ABI).
# Data import/export only. Coax supplies a private staging path and admits
# the exported documents before publishing any animation package.
import sys
import bpy


def main():
    source, output = sys.argv[sys.argv.index("--") + 1:]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=source, use_anim=True)
    bpy.ops.export_scene.gltf(
        filepath=output,
        export_format="GLTF_SEPARATE",
        export_materials="NONE",
        export_skins=True,
        export_all_influences=True,
        export_morph=True,
        export_animations=True,
        export_animation_mode="ACTIONS",
        export_force_sampling=True,
    )


if __name__ == "__main__":
    main()
