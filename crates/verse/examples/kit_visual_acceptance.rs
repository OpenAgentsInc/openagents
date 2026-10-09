//! Private, source-bound kit merge, grade, and damage captures.
//! Compile the filtered ignored tests, then run them under remote quiet/GPU leases.
//! All inputs and outputs must stay outside Git.

use glam::{Mat4, Vec3};
use serde_json::json;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use verse::{
    mesh::Mesh,
    pbr::{
        Key, Neon,
        textured::{Primitive, TexturedMesh, TexturedScene},
    },
    render::{Offscreen, View},
    ui::{Atlas, UiBatch},
};

fn private(path: PathBuf) -> PathBuf {
    let path = path.canonicalize().unwrap();
    assert!(!path.ancestors().any(|p| p.join(".git").exists()));
    path
}

fn output() -> PathBuf {
    let path = PathBuf::from(std::env::var_os("VERSE_KIT_VISUAL_OUTPUT").unwrap());
    std::fs::create_dir_all(&path).unwrap();
    private(path)
}

fn bounds(scene: &TexturedScene) -> (Vec3, Vec3) {
    let merged = scene.merge().unwrap();
    merged.indices.iter().fold(
        (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
        |(low, high), &i| {
            let p = Vec3::from(merged.vertices[i as usize].pos);
            (low.min(p), high.max(p))
        },
    )
}

fn flatten(scene: &TexturedScene) -> TexturedScene {
    let merged = scene.merge().unwrap();
    let mut mesh = TexturedMesh::default();
    for batch in &merged.batches {
        let mut primitive = Primitive {
            material: batch.material,
            vertices: Vec::new(),
            indices: Vec::new(),
        };
        let mut remap = BTreeMap::new();
        for &i in &merged.indices[batch.first as usize..(batch.first + batch.count) as usize] {
            let index = *remap.entry(i).or_insert_with(|| {
                primitive.vertices.push(merged.vertices[i as usize]);
                (primitive.vertices.len() - 1) as u32
            });
            primitive.indices.push(index);
        }
        mesh.primitives.push(primitive);
    }
    let mut result = TexturedScene {
        images: scene.images.clone(),
        materials: scene.materials.clone(),
        ..TexturedScene::default()
    };
    let mesh = result.add_mesh(mesh);
    result.place(mesh, Mat4::IDENTITY);
    result
}

fn stage(center: Vec3, extent: f32) -> Mesh {
    let mut neon = Neon::neutral(0.0);
    // Keep the 120 m inspection beyond the bare world's 110 m fog end visible.
    neon.fog_start = 1_000.0;
    neon.fog_end = 2_000.0;
    neon.key = Some(Key {
        dir: Vec3::new(0.55, 0.7, 0.45).normalize(),
        illuminance: 4200.0,
        angular_radius: 0.035,
        rim_dir: Vec3::new(-0.4, 0.35, -0.85).normalize(),
        rim_illuminance: 2000.0,
        rim_angular_radius: 0.12,
        sky: 420.0,
        ground: 60.0,
        ev100: 10.0,
        shadow_center: center,
        shadow_half: extent,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    Mesh {
        neon: Some(neon),
        ..Mesh::default()
    }
}

fn write_png(path: &Path, rgba: &[u8]) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), 1280, 800);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
}

fn capture(scene: TexturedScene, view: View, dynamic: &Mesh) -> Vec<u8> {
    let world = Mesh {
        textured: Some(Arc::new(scene)),
        ..Mesh::default()
    };
    let mut renderer = Offscreen::new(
        1280,
        800,
        &world,
        &Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )
    .unwrap();
    renderer.render(view, dynamic, &UiBatch::default()).unwrap()
}

fn demo() {
    let output = output();
    let input = private(PathBuf::from(
        std::env::var_os("VERSE_KIT_VISUAL_INPUT").unwrap(),
    ));
    let mut source = TexturedScene::default();
    let model = source.import_gltf(&input).unwrap();
    source.place(model, Mat4::IDENTITY);
    // Match the actual kit's texture edge before applying its grade.
    for image in &mut source.images {
        let (width, height, rgba) = verse::zones::everglade_pack::compile::downscale(
            &image.rgba,
            image.width,
            image.height,
            512,
        );
        image.width = width;
        image.height = height;
        image.rgba = rgba;
    }
    let merged = flatten(&source);
    let (low, high) = bounds(&source);
    assert_eq!((low, high), bounds(&merged));
    assert_eq!(
        source.merge().unwrap().indices.len(),
        merged.merge().unwrap().indices.len()
    );
    let center = (low + high) / 2.0;
    let extent = (high - low).length();
    let eye = center + Vec3::new(-0.9, 0.55, 1.2).normalize() * extent * 1.3;
    let view = View {
        eye,
        view_proj: Mat4::perspective_rh(0.7, 1.6, 0.1, 500.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
    };
    let dynamic = stage(center, extent);
    let before = capture(source.clone(), view, &dynamic);
    assert!(
        before
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|&c| c > 32)),
        "The demo house must be visible"
    );
    let after = capture(merged.clone(), view, &dynamic);
    write_png(&output.join("demo-source.png"), &before);
    write_png(&output.join("demo-merged.png"), &after);
    let delta: u64 = before
        .iter()
        .zip(&after)
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    let max = before
        .iter()
        .zip(&after)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    let mut graded = merged.clone();
    for image in &mut graded.images {
        verse::zones::everglade_pack::kit::grade(image.width, image.height, &mut image.rgba);
    }
    assert_ne!(graded.images, merged.images);
    let graded_rgba = capture(graded, view, &dynamic);
    write_png(&output.join("grade-before.png"), &after);
    write_png(&output.join("grade-after.png"), &graded_rgba);
    let record = json!({"label":"exported Unreal demo input", "input":input,
        "scope":"Same original exported actor and Verse base-color conversion; not an Unreal-engine screenshot",
        "low":low.to_array(),"high":high.to_array(),"eye":eye.to_array(),"target":center.to_array(),
        "triangles":source.merge().unwrap().indices.len()/3,"materials":source.materials.len(),
        "before_batches":source.merge().unwrap().batches.len(),"after_batches":merged.merge().unwrap().batches.len(),
        "matching_pixel_max_delta":max,"matching_pixel_mean_delta":delta as f64/before.len() as f64,
        "grade_changed_pixel_channels":after.iter().zip(&graded_rgba).filter(|(a,b)|a!=b).count()});
    std::fs::write(
        output.join("demo-report.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    println!("{record}");
    assert!(max <= 1, "The merged demo changes matching pixels by {max}");
}

fn damage() {
    use verse::zones::{
        everglade::{demolition::town::Town, layout, scene},
        everglade_pack::{self, kit},
    };
    let output = output();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pack = everglade_pack::ZonePack::load_local(
        &repo
            .join(everglade_pack::PACK_DIRECTORY)
            .join(format!("{}.vtp", everglade_pack::PACK_SHA256)),
    )
    .unwrap();
    assert!(kit::installed(&pack));
    let house = layout::first_town_houses()
        .into_iter()
        .find(|h| h.center == [-22.0, -48.0])
        .expect("The capture cabin exists");
    let mut placements = Vec::new();
    house.raise(&mut placements);
    let (scene, _) = scene::build_painted(&pack, &placements, layout::paint).unwrap();
    assert_eq!(scene.detail_groups.len(), 1);
    let scene = Arc::new(scene);
    let mut town =
        Town::standalone_with_houses(&pack, &placements, scene.clone(), &[house]).unwrap();
    town.set_numbers(false);
    let world = Mesh {
        textured: Some(scene.clone()),
        ..Mesh::default()
    };
    let (low, high) = bounds(&scene);
    let center = (low + high) / 2.0;
    let eye = center + Vec3::new(0.0, 8.0, 120.0);
    let view = View {
        eye,
        view_proj: Mat4::perspective_rh(0.17, 1.6, 0.1, 500.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
    };
    let mut renderer = Offscreen::new(
        1280,
        800,
        &world,
        &Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )
    .unwrap();
    let player = verse::controller::PlayerController::new(eye, std::f32::consts::PI);
    let dynamic = stage(center, (high - low).length());
    let mut rows = Vec::new();
    let mut images = Vec::new();
    for step in 0..4 {
        if step == 1 || step == 2 {
            let [x, z] = house.world([if step == 1 { -2.0 } else { 2.0 }, house.depth / 2.0]);
            assert!(
                !town
                    .blast(Vec3::new(x, house.floor() + 2.0, z), 2.5, 1000, Vec3::Z)
                    .is_empty()
            );
            town.tick(1.0 / 60.0, &player);
            assert!(!scene.edits.group_fallbacks().is_empty());
        } else if step == 3 {
            // This is the Town::restore path called by the R intent.
            town.restore();
            town.tick(1.0 / 60.0, &player);
            assert!(scene.edits.group_fallbacks().is_empty());
            assert_eq!(town.hidden(), 0);
        }
        let mut frame = dynamic.clone();
        frame.figure = town.own_figure();
        let rgba = renderer.render(view, &frame, &UiBatch::default()).unwrap();
        let name = [
            "far-before",
            "far-first-damage",
            "far-repeated-damage",
            "far-restored",
        ][step];
        write_png(&output.join(format!("{name}.png")), &rgba);
        rows.push(json!({"name":name,"hidden":town.hidden(),"fallbacks":scene.edits.group_fallbacks(),
            "index_revision":scene.edits.revision(),"draw_stats":format!("{:?}",renderer.draw_stats())}));
        images.push(rgba);
    }
    let record = json!({"eye":eye.to_array(),"target":center.to_array(),"horizontal_distance":120,
        "vertical_offset":8,"fov_radians":0.17,"scope":"Actual private house and Town destruction over one persistent GPU renderer",
        "steps":rows,"first_damage_changes_pixels":images[0]!=images[1],
        "repeated_damage_changes_pixels":images[1]!=images[2],"restored_pixels_match":images[0]==images[3]});
    std::fs::write(
        output.join("damage-report.json"),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
    println!("{record}");
    assert!(
        images[0]
            .chunks_exact(4)
            .any(|p| p[..3].iter().any(|&c| c > 32)),
        "The far house must be visible"
    );
    assert!(
        images[0] != images[1],
        "First damage must change the far house"
    );
    assert!(
        images[1] != images[2],
        "Repeated damage must change the far house"
    );
    assert!(
        images[0] == images[3],
        "Restoring must redraw the original far house"
    );
}

fn main() {
    panic!("Run a filtered ignored private_visual_tests test under its leases");
}

#[cfg(test)]
mod private_visual_tests {
    fn run(f: fn()) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .unwrap()
            .join()
            .unwrap();
    }
    #[test]
    #[ignore = "Private exported demo and remote offscreen GPU lease"]
    fn same_demo_merge_and_grade() {
        run(super::demo);
    }
    #[test]
    #[ignore = "Private candidate kit and remote offscreen GPU lease"]
    fn far_damage_and_restore() {
        run(super::damage);
    }
}
