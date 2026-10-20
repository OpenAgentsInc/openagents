//! The desktop app's offline demo data, compiled from the client crate's own
//! files so the web page shows exactly what `--demo` shows on the desktop.
#![allow(dead_code, unused_imports)]

#[path = "../../../crates/client/src/demo/fixtures.rs"]
pub(crate) mod fixtures;
#[path = "../../../crates/client/src/demo/png.rs"]
pub(crate) mod png;
#[path = "../../../crates/client/src/demo/transcripts.rs"]
pub(crate) mod transcripts;
#[path = "../../../crates/client/src/demo/visuals.rs"]
pub(crate) mod visuals;

#[cfg(test)]
mod visual_tests {
    use super::visuals;
    use openagents_visual::{Camera, Node, NodeKind, Quality, Visual};

    fn chart(node: &Node) -> Option<&openagents_visual::Chart> {
        if let NodeKind::Chart(chart) = &node.kind {
            return Some(chart);
        }
        openagents_visual::children(node).find_map(chart)
    }

    #[test]
    fn recorded_chart_and_scene_have_drawable_content() {
        // These are the exact scripts and visual ids in the desktop's
        // recorded demo chat, running on the browser's Rust interpreter.
        let (id, _, script) = visuals::DEMO_VISUALS[0];
        let mut visual = Visual::load(script, openagents_visual::seed_for(id)).unwrap();
        let tree = visual.render().unwrap();
        let chart = chart(&tree).expect("the recorded dashboard includes a chart");
        assert_eq!(chart.series.len(), 2);
        assert!(!chart.series[0].values.is_empty());

        let (id, _, script) = visuals::DEMO_VISUALS[2];
        let mut visual = Visual::load(script, openagents_visual::seed_for(id)).unwrap();
        let tree = visual.render().unwrap();
        let NodeKind::Scene(node) = tree.kind else {
            panic!("recorded 3D visual");
        };
        let scene = visual.draw_scene(node.draw, 0.).unwrap();
        let camera = Camera {
            yaw: node.yaw,
            pitch: node.pitch,
            zoom: node.zoom,
        };
        let raster = scene.render(camera, 96., 96., 1., Quality::Draft, &|_| {
            [0.4, 0.6, 0.8, 1.]
        });
        assert_eq!(raster.bgra.len(), 96 * 96 * 4);
        assert!(raster.bgra.chunks_exact(4).filter(|p| p[3] > 0).count() > 100);
        let rotated = scene.render(
            Camera {
                yaw: camera.yaw + 0.5,
                ..camera
            },
            96.,
            96.,
            1.,
            Quality::Draft,
            &|_| [0.4, 0.6, 0.8, 1.],
        );
        assert_ne!(raster.bgra, rotated.bgra, "orbit changes the picture");
    }

    #[test]
    fn browser_interpreter_keeps_the_visual_sandbox() {
        for source in [
            "return os.execute('true')",
            "return io.open('/etc/passwd')",
            "return require('x')",
            "ui.text = nil return ui.text 'x'",
        ] {
            assert!(Visual::load(source, 1).is_err(), "{source}");
        }
        let started = std::time::Instant::now();
        let error = Visual::load("while true do end", 1).err().unwrap();
        assert!(error.message.contains("time budget"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        let mut visual = Visual::load("return function() while true do end end", 1).unwrap();
        assert!(visual.render().unwrap_err().message.contains("time budget"));
    }
}
