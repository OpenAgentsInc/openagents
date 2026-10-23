use super::*;
fn file(length: usize, count: usize) -> FileSpec {
    FileSpec {
        url: "part.bin".into(),
        sha256: "0".repeat(64),
        byte_length: length,
        encoding: None,
        decoded_byte_length: None,
        decoded_sha256: None,
        count,
        stride: 32,
        width: 1,
        height: 1,
        triangle_count: length / 36,
    }
}
fn manifest() -> Manifest {
    Manifest {
        schema: "native-town.v1".into(),
        vertices: file(96, 3),
        indices: file(12, 3),
        atlas: file(4, 0),
        collision: file(36, 0),
        draws: vec![Draw {
            first_index: 0,
            index_count: 3,
            model: identity(),
            atlas_rect: [0., 0., 1., 1.],
            uv_scale: unit_uv(),
            alpha_cutoff: 0.,
            bounds: Bounds {
                min: [0.; 3],
                max: [1.; 3],
            },
        }],
        spawn: [0.; 3],
        bounds: Bounds {
            min: [0.; 3],
            max: [1.; 3],
        },
        walk_bounds: None,
    }
}
fn floor() -> Vec<f32> {
    vec![
        -10., 0., -10., 10., 0., -10., -10., 0., 10., 10., 0., -10., 10., 0., 10., -10., 0., 10.,
    ]
}
#[test]
fn manifest_limits_reject_invalid_indices_bounds_and_numbers() {
    let mut m = manifest();
    assert!(validate_manifest(&m).is_ok());
    m.draws[0].index_count = 6;
    assert!(validate_manifest(&m).is_err());
    m.draws[0].index_count = 3;
    m.vertices.stride = 48;
    assert!(validate_manifest(&m).is_err());
    m.vertices.stride = 32;
    m.atlas.width = 4096;
    assert!(validate_manifest(&m).is_err());
    m.atlas.width = 1;
    m.draws[0].bounds.min[0] = 2.;
    assert!(validate_manifest(&m).is_err());
    assert!(decode_floats(&f32::NAN.to_le_bytes()).is_err());
    assert!(decode_floats(&[1, 2, 3]).is_err());
}
#[test]
fn compressed_files_require_exact_decoded_length_and_hash() {
    use std::io::Write;
    let raw = b"owned triangle data";
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(raw).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut spec = file(bytes.len(), 0);
    spec.encoding = Some("gzip".into());
    spec.decoded_byte_length = Some(raw.len());
    spec.decoded_sha256 = Some(digest(raw));
    assert_eq!(decode_file(bytes.clone(), &spec).unwrap(), raw);
    spec.decoded_byte_length = Some(raw.len() - 1);
    assert!(decode_file(bytes.clone(), &spec).is_err());
    spec.decoded_byte_length = Some(raw.len());
    spec.decoded_sha256 = Some("0".repeat(64));
    assert!(decode_file(bytes, &spec).is_err());
}
#[test]
fn capsule_walks_on_source_floor_and_stops_at_triangle_wall() {
    let mut tris = floor();
    tris.extend([
        -2., 0., 2., 2., 0., 2., -2., 3., 2., 2., 0., 2., 2., 3., 2., -2., 3., 2.,
    ]);
    let world = Collision::new(&tris).unwrap();
    let mut p = Vec3::ZERO;
    let mut grounded = false;
    for _ in 0..100 {
        p = world.move_capsule(p, Vec3::Z * 0.06, -0.004, &mut grounded);
    }
    assert!((p.y - 0.).abs() < 0.001);
    assert!(p.z > 1.5 && p.z < 1.71, "{p:?}");
    assert!(grounded);
    assert!(Collision::new(&[f32::NAN; 9]).is_err());
    assert!(Collision::new(&[f32::MAX; 9]).is_err());
}
#[test]
fn viewport_cap_preserves_aspect_and_rejects_zero() {
    assert_eq!(extent(2560, 1440, 2.).unwrap(), (1280, 720));
    let (w, h) = extent(1320, 2868, 3.).unwrap();
    assert!(w <= 720 && h <= 1280);
    assert!((w as f32 / h as f32 - 1320. / 2868.).abs() < 0.001);
    assert!(extent(0, 2, 1.).is_err());
}
#[test]
fn frame_counters_do_not_claim_requested_refresh_rate() {
    let directory = tempfile::tempdir().unwrap();
    let renderer = MedievalTownRenderer::new(directory.path().display().to_string());
    let metrics: serde_json::Value = serde_json::from_str(&renderer.metrics()).unwrap();
    assert_eq!(metrics["ready"], false);
    assert_eq!(metrics["completedFrames"], 0);
    assert_eq!(metrics["fps"], 0.);
    assert!(
        renderer
            .frame(f64::NAN, 0., 0., 0., 0., false, false)
            .is_err()
    );
}

#[test]
#[ignore = "requires the private cooked Medieval Town package"]
fn private_native_package_parses_and_walks_on_its_source_collision() {
    let root = PathBuf::from(
        std::env::var("OPENAGENTS_NATIVE_TOWN_FIXTURE").expect("set private package directory"),
    );
    let bytes = std::fs::read(root.join("native-town.json")).unwrap();
    assert_eq!(digest(&bytes), MANIFEST_SHA256);
    let manifest: Manifest = serde_json::from_slice(&bytes).unwrap();
    validate_manifest(&manifest).unwrap();
    let mut parts = Vec::new();
    for spec in [
        &manifest.vertices,
        &manifest.indices,
        &manifest.atlas,
        &manifest.collision,
    ] {
        let bytes = std::fs::read(root.join(&spec.url)).unwrap();
        assert_eq!(bytes.len(), spec.byte_length);
        assert_eq!(digest(&bytes), spec.sha256);
        parts.push(decode_file(bytes, spec).unwrap());
    }
    let mesh = decode_mesh(manifest, parts).unwrap();
    let mut camera = Camera::new(mesh.spawn);
    for i in 0..300 {
        camera.update(&mesh, i as f64 / 60., 0., 0.5, 0., 0., false, false);
        assert!(
            camera.position.y > -2.,
            "Lost source ground at tick {i}: {:?}",
            camera.position
        );
    }
    assert!(
        camera.position.z > mesh.spawn.z + 3.,
        "{:?}",
        camera.position
    );
    assert!(camera.position.y > 0.);
    assert!(camera.position.y < 5.);
    camera = Camera::new(mesh.spawn);
    camera.yaw = std::f32::consts::PI * 1.25;
    for i in 0..300 {
        camera.update(&mesh, i as f64 / 60., 0., 1., 0., 0., false, false);
        assert!(camera.position.y > 0.);
    }
    let stopped = camera.position;
    assert!(
        (2.2..2.5).contains(&stopped.x) && (27.0..28.0).contains(&stopped.z),
        "The forecourt architecture did not stop the capsule: {stopped:?}"
    );
    for i in 300..360 {
        camera.update(&mesh, i as f64 / 60., 0., 1., 0., 0., false, false);
    }
    assert!(
        (camera.position - stopped).length() < 0.01,
        "The capsule crossed the source wall: {:?}",
        camera.position
    );
}

#[test]
fn native_town_shader_parses_and_validates() {
    let module = naga::front::wgsl::parse_str(include_str!("town.wgsl")).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .unwrap();
}

#[test]
fn rounded_capsule_crosses_a_thin_pavement_bevel_without_sinking_or_sticking() {
    let mut triangles = floor();
    triangles.extend([
        -3., 0.02, 2., 3., 0.02, 2., -3., 0.02, 2.04, 3., 0.02, 2., 3., 0.02, 2.04, -3., 0.02, 2.04,
    ]);
    triangles.extend([
        -3., 0., 2., 3., 0., 2., -3., 0.02, 2., 3., 0., 2., 3., 0.02, 2., -3., 0.02, 2.,
    ]);
    let world = Collision::new(&triangles).unwrap();
    let mut p = Vec3::ZERO;
    let mut grounded = true;
    for _ in 0..120 {
        p = world.move_capsule(p, Vec3::Z * 0.06, -0.004, &mut grounded);
        assert!(p.y >= 0.);
    }
    assert!(
        p.z > 5.9,
        "Rounded feet stuck on a two-centimeter pavement edge: {p:?}"
    );
}
