use glam::Vec3;

use super::*;
use crate::backend::{MISS, NEAREST, SHADOW};

/// The fixture's settings at test quality.
fn quick() -> Settings {
    Settings {
        vertex_rays: 32,
        probe_rays: 64,
        sun_rays: 2,
        ..fixture::settings()
    }
}

fn cpu_bake(scene: &Scene, settings: &Settings, threads: usize) -> (Products, Stats) {
    let mut backend = CpuBackend::new(&scene.triangles, threads);
    bake(scene, &fixture::light(), settings, &mut backend, threads).unwrap()
}

/// The vertex nearest `p` facing `n`.
fn vertex_at(scene: &Scene, p: Vec3, n: Vec3) -> usize {
    (0..scene.vertices.len())
        .filter(|&i| Vec3::from(scene.vertices[i].normal).dot(n) > 0.9)
        .min_by(|&a, &b| {
            let d = |i: usize| Vec3::from(scene.vertices[i].pos).distance(p);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

#[test]
fn the_fixture_darkens_under_the_roof_and_sees_the_sun_in_the_open() {
    let scene = Scene::new(&fixture::scene()).unwrap();
    let (products, stats) = cpu_bake(&scene, &quick(), 4);
    assert_eq!(products.vertex_light.len(), scene.vertices.len());
    assert!(products.vertex_light.iter().all(|l| l[3] > 0));
    // Ground beside the house wall, under the eaves, and open ground.
    let open = vertex_at(&scene, Vec3::new(6.0, 0.0, 10.0), Vec3::Y);
    let eaves = vertex_at(&scene, Vec3::new(-4.0, 0.0, 0.0), Vec3::Y);
    assert!(products.vertex_open[open] > 0.95);
    assert!(products.vertex_open[eaves] < products.vertex_open[open] - 0.2);
    assert!(products.vertex_ambient[eaves][1] < products.vertex_ambient[open][1]);
    // The ground inside the house sees no sun from any direction.
    let inside = vertex_at(&scene, Vec3::new(-4.0, 0.0, -3.0), Vec3::Y);
    assert_eq!(products.vertex_sun.len(), 3);
    for sun in &products.vertex_sun {
        assert_eq!(sun[inside], 0.0);
        assert_eq!(sun[open], 1.0);
    }
    assert_eq!(stats.pass_ms.len(), 2);
    assert!(stats.nearest_rays > 0 && stats.shadow_rays > 0);
    // A probe under the eaves, at (-4, 0.5, 0), holds less light than one
    // in the open at (10, 0.5, 10).
    let dims = products.probes.dims;
    let probe = |x: u32, z: u32| products.probes.data[(x + z * dims[0] * dims[1]) as usize][0];
    assert!(probe(4, 6) < probe(11, 11));
}

#[test]
fn a_second_bounce_brightens_shade_and_changes_the_key() {
    let scene = Scene::new(&fixture::scene()).unwrap();
    let one = Settings {
        bounces: 1,
        ..quick()
    };
    let (single, _) = cpu_bake(&scene, &one, 4);
    let (double, _) = cpu_bake(&scene, &quick(), 4);
    assert_ne!(single.bake_key, double.bake_key);
    assert_ne!(single.vertex_ambient, double.vertex_ambient);
    // Sun visibility and sky openness do not depend on bounces.
    assert_eq!(single.vertex_open, double.vertex_open);
    assert_eq!(single.vertex_sun, double.vertex_sun);
}

#[test]
fn a_rebake_on_any_thread_count_reproduces_the_digest() {
    let scene = Scene::new(&fixture::scene()).unwrap();
    let settings = quick();
    let (a, _) = cpu_bake(&scene, &settings, 1);
    let (b, _) = cpu_bake(&scene, &settings, 3);
    let (c, _) = cpu_bake(&Scene::new(&fixture::scene()).unwrap(), &settings, 4);
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a.digest(), c.digest());
    assert_eq!(a, c);
    // Another seed samples differently.
    let reseeded = Settings {
        seed: 7,
        ..settings
    };
    assert_ne!(cpu_bake(&scene, &reseeded, 2).0.digest(), a.digest());
}

#[test]
fn the_key_follows_the_scene_the_light_and_the_settings() {
    let scene = Scene::new(&fixture::scene()).unwrap();
    let light = fixture::light();
    let settings = quick();
    let key = bake_key(&scene, &light, &settings);
    assert_eq!(
        key,
        bake_key(&Scene::new(&fixture::scene()).unwrap(), &light, &settings)
    );
    let mut moved = fixture::scene();
    moved.placements[1].transform *= glam::Mat4::from_translation(Vec3::X * 0.01);
    assert_ne!(
        key,
        bake_key(&Scene::new(&moved).unwrap(), &light, &settings)
    );
    let dimmer = Light {
        sky: 1_000.0,
        ..light
    };
    assert_ne!(key, bake_key(&scene, &dimmer, &settings));
    let finer = Settings {
        vertex_rays: 64,
        ..settings
    };
    assert_ne!(key, bake_key(&scene, &light, &finer));
}

#[test]
fn products_round_trip_and_keep_layers_they_do_not_know() {
    let scene = Scene::new(&fixture::scene()).unwrap();
    let (mut products, _) = cpu_bake(&scene, &quick(), 2);
    products.extra.push(products::RawLayer {
        entry: products::LayerEntry {
            name: "lightmap.0".into(),
            format: "u8".into(),
            components: 1,
            count: 3,
            offset: 0,
        },
        bytes: vec![1, 2, 3],
    });
    let bytes = products.encode();
    let mut read = Products::decode(&bytes).unwrap();
    assert_eq!(read.extra[0].bytes, vec![1, 2, 3]);
    assert_eq!(read.encode(), bytes);
    // Offsets are assigned on encoding; everything else survives.
    read.extra[0].entry.offset = 0;
    assert_eq!(read, products);
    assert!(Products::decode(b"nope").is_err());
    let mut newer = bytes.clone();
    newer[4] = 9;
    assert!(Products::decode(&newer).unwrap_err().contains("format 9"));
    assert!(Products::decode(&bytes[..bytes.len() - 8]).is_err());
}

/// Three glass panes, each stopping 30 percent, stacked above the origin,
/// and a solid roof above them.
fn panes() -> Vec<scene::Triangle> {
    let mut triangles = Vec::new();
    for (i, (y, opacity)) in [(1.0, 0.3), (2.0, 0.3), (3.0, 0.3), (5.0, 1.0)]
        .into_iter()
        .enumerate()
    {
        for (k, corners) in [
            [
                Vec3::new(-1.0, y, -1.0),
                Vec3::new(1.0, y, -1.0),
                Vec3::new(1.0, y, 1.0),
            ],
            [
                Vec3::new(-1.0, y, -1.0),
                Vec3::new(1.0, y, 1.0),
                Vec3::new(-1.0, y, 1.0),
            ],
        ]
        .into_iter()
        .enumerate()
        {
            let base = (i * 2 + k) as u32 * 3;
            triangles.push(scene::Triangle {
                corners,
                normal: Vec3::Y,
                albedo: Vec3::splat(0.5),
                opacity,
                vertices: [base, base + 1, base + 2],
            });
        }
    }
    triangles
}

fn pane_rays() -> Vec<Ray> {
    let up = Vec3::new(0.1, 1.0, 0.05).normalize();
    vec![
        // Through all three panes to the roof.
        Ray::new(Vec3::ZERO, up, 10.0, NEAREST),
        // Short of the roof: three panes pass 0.343.
        Ray::new(Vec3::ZERO, up, 4.0, NEAREST),
        Ray::new(Vec3::ZERO, up, 4.0, SHADOW),
        // Sideways: nothing.
        Ray::new(Vec3::ZERO, Vec3::X, 10.0, NEAREST),
        // Down from above the panes onto the top one.
        Ray::new(Vec3::new(0.2, 4.0, 0.2), -Vec3::Y, 10.0, NEAREST),
    ]
}

#[test]
fn the_cpu_backend_multiplies_partial_occluders_and_stops_at_solid_ones() {
    let mut backend = CpuBackend::new(&panes(), 2);
    let hits = backend.trace(&pane_rays()).unwrap();
    assert_eq!(hits[0].transmittance, 0.0);
    assert!((hits[0].distance - 1.0 / Vec3::new(0.1, 1.0, 0.05).normalize().y).abs() < 1e-4);
    assert!(hits[0].triangle < 2);
    assert!((hits[1].transmittance - 0.343).abs() < 1e-4);
    assert!((hits[2].transmittance - 0.343).abs() < 1e-4);
    assert_eq!(hits[2].triangle, MISS);
    assert_eq!(hits[3], RayHit::CLEAR);
    // Down the panes' shared diagonal, each pane counts once.
    assert!((hits[4].transmittance - 0.343).abs() < 1e-4);
    assert!((hits[4].distance - 1.0).abs() < 1e-4);
    assert!((4..6).contains(&hits[4].triangle));
}

#[cfg(feature = "gpu")]
mod gpu {
    use super::*;
    use crate::gpu::GpuBackend;

    fn gpu(triangles: &[scene::Triangle]) -> Option<GpuBackend> {
        match GpuBackend::new(triangles) {
            Ok(backend) => Some(backend),
            Err(reason) => {
                eprintln!("skipped: no ray-query adapter ({reason})");
                None
            }
        }
    }

    #[test]
    fn the_gpu_traces_partial_occluders_as_the_cpu_does() {
        let Some(mut gpu) = gpu(&panes()) else {
            return;
        };
        let mut cpu = CpuBackend::new(&panes(), 2);
        let rays = pane_rays();
        let expected = cpu.trace(&rays).unwrap();
        let got = gpu.trace(&rays).unwrap();
        for (e, g) in expected.iter().zip(&got) {
            assert!(
                (e.transmittance - g.transmittance).abs() < 1e-5,
                "{e:?} {g:?}"
            );
            assert_eq!(e.triangle == MISS, g.triangle == MISS, "{e:?} {g:?}");
            if e.triangle != MISS {
                assert!((e.distance - g.distance).abs() < 1e-3, "{e:?} {g:?}");
                // Each pane is two triangles; either may take a ray on
                // their shared edge.
                assert_eq!(e.triangle / 2, g.triangle / 2, "{e:?} {g:?}");
            }
        }
    }

    #[test]
    fn both_backends_meet_the_face_of_a_thin_slab_that_faces_the_ray() {
        // The top and bottom of a slab of no thickness, in both orders and
        // both windings, so neither list order nor winding decides.
        let corners = [
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(0.0, 1.0, 1.0),
        ];
        let reversed = [corners[0], corners[2], corners[1]];
        let face = |corners, normal: Vec3| scene::Triangle {
            corners,
            normal,
            albedo: Vec3::ONE,
            opacity: 1.0,
            vertices: [0; 3],
        };
        let triangles = vec![
            face(corners, Vec3::Y),
            face(reversed, -Vec3::Y),
            face(reversed.map(|c| c + Vec3::X * 3.0), Vec3::Y),
            face(corners.map(|c| c + Vec3::X * 3.0), -Vec3::Y),
        ];
        let Some(mut gpu) = gpu(&triangles) else {
            return;
        };
        let mut cpu = CpuBackend::new(&triangles, 1);
        let rays = [0.0, 3.0]
            .into_iter()
            .flat_map(|x| {
                [
                    Ray::new(Vec3::new(x, 3.0, 0.0), -Vec3::Y, 10.0, NEAREST),
                    Ray::new(Vec3::new(x, -2.0, 0.0), Vec3::Y, 10.0, NEAREST),
                ]
            })
            .collect::<Vec<_>>();
        let expected = cpu.trace(&rays).unwrap();
        assert_eq!(
            expected.iter().map(|h| h.triangle).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let got = gpu.trace(&rays).unwrap();
        for (e, g) in expected.iter().zip(&got) {
            assert_eq!(e.triangle, g.triangle, "{e:?} {g:?}");
        }
    }

    #[test]
    fn the_fixture_bakes_alike_on_the_gpu_and_the_cpu() {
        let scene = Scene::new(&fixture::scene()).unwrap();
        let Some(mut gpu) = gpu(&scene.triangles) else {
            return;
        };
        let settings = quick();
        let (cpu, _) = cpu_bake(&scene, &settings, 4);
        let (on_gpu, _) = bake(&scene, &fixture::light(), &settings, &mut gpu, 4).unwrap();
        assert_eq!(cpu.bake_key, on_gpu.bake_key);
        let agreement = cpu.compare(&on_gpu).unwrap();
        eprintln!("{}: {agreement:?}", gpu.name());
        for spread in [
            agreement.ambient,
            agreement.open,
            agreement.sun,
            agreement.probes,
        ] {
            assert!(spread.within(&GPU_TOLERANCE), "{agreement:?}");
        }
        // A rebake on the GPU reproduces its own digest.
        let (again, _) = bake(&scene, &fixture::light(), &settings, &mut gpu, 4).unwrap();
        assert_eq!(again.digest(), on_gpu.digest());
    }
}
