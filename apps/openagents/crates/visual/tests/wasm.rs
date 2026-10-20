//! Execute with wasm-bindgen-test-runner in release mode. A native portable-VM
//! test cannot detect an interpreter that relies on unwinding in aborting Wasm.
#![cfg(target_family = "wasm")]

use openagents_visual::{Camera, HandlerArg, NodeKind, Quality, Visual, children};
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn charts_controls_and_software_scenes_execute_in_wasm() {
    let mut visual = Visual::load(
        r#"
        local s = ui.state { n = 1 }
        return function()
          return ui.column {
            ui.button { label = "More", on_click = function() s.n += 1 end },
            ui.chart { x = { "A", "B" }, series = { { name = "Count", values = { s.n, 3 } } } },
            ui.scene { height = 200, draw = function(scene)
              scene:box { at = { 0, 0, 0 }, size = s.n, color = "accent" }
            end },
          }
        end
        "#,
        1,
    )
    .unwrap();
    let tree = visual.render().unwrap();
    let nodes: Vec<_> = children(&tree).collect();
    let NodeKind::Button { on_click, .. } = nodes[0].kind else {
        panic!()
    };
    visual.call(on_click, HandlerArg::None).unwrap();
    let tree = visual.render().unwrap();
    let nodes: Vec<_> = children(&tree).collect();
    let NodeKind::Chart(chart) = &nodes[1].kind else {
        panic!()
    };
    assert_eq!(chart.series[0].values[0], Some(2.));
    let NodeKind::Scene(node) = &nodes[2].kind else {
        panic!()
    };
    let scene = visual.draw_scene(node.draw, 0.).unwrap();
    let raster = scene.render(
        Camera {
            yaw: node.yaw,
            pitch: node.pitch,
            zoom: node.zoom,
        },
        64.,
        64.,
        1.,
        Quality::Draft,
        &|_| [0.4, 0.6, 0.8, 1.],
    );
    assert!(raster.bgra.chunks_exact(4).any(|p| p[3] > 0));
}

#[wasm_bindgen_test]
fn script_failures_and_limits_return_errors_without_trapping_the_app() {
    for source in [
        "return function(",
        "error('broken visual')",
        "return os.execute('true')",
        "return io.open('/etc/passwd')",
        "return require('x')",
        "ui.text = nil return ui.text 'x'",
    ] {
        assert!(Visual::load(source, 1).is_err(), "{source}");
    }
    let error = Visual::load("while true do end", 1).err().unwrap();
    assert!(error.message.contains("time budget"), "{error}");
    let error = Visual::load(
        "local t = {} for i = 1, 1e9 do t[i] = string.rep('x', 1024) .. i end",
        1,
    )
    .err()
    .unwrap();
    assert!(
        error.message.contains("MB") || error.message.contains("time budget"),
        "{error}"
    );
    let mut visual = Visual::load(
        "local n = 0 return function() n += 1 if n == 1 then error('first render') end return ui.text 'recovered' end", 1,
    ).unwrap();
    assert!(visual.render().is_err());
    assert!(visual.render().is_ok());
    let mut visual = Visual::load("return function() while true do end end", 1).unwrap();
    assert!(visual.render().unwrap_err().message.contains("time budget"));
    assert!(
        Visual::load("return ui.text 'still running'", 1)
            .unwrap()
            .render()
            .is_ok()
    );
}
