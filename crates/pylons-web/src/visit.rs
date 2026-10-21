//! What the Grid homepage's visit records of the Pylons (#12439): walking
//! into the Pylon Bay, pointing at a Pylon, and opening its card, by the
//! Pylon's kind only (never its key or name). [`send`] goes through the
//! page's `globalThis.oaTrack`, which only the Grid homepage defines, and
//! only when the browser didn't ask not to be tracked.

use pylons_view::bay::{self, Shape};

/// The word a Pylon's kind is recorded as.
#[must_use]
pub fn kind(shape: Shape) -> &'static str {
    match shape {
        Shape::Mac => "mac",
        Shape::Gpu => "gpu",
        Shape::Cpu => "cpu",
        Shape::Secure => "secure",
        Shape::Head => "head",
        Shape::Part => "part",
    }
}

/// How far past the Bay's outline the visitor counts as inside it, m, so
/// walking along its edge doesn't count as entering again and again.
const MARGIN: f64 = 1.0;

/// Whether `player` (`[x, y, z]`, m) stands in the Pylon Bay: inside its
/// outline, or within [`MARGIN`] of it when `already_in`.
#[must_use]
pub fn in_bay(player: [f64; 3], already_in: bool) -> bool {
    if !player.iter().all(|v| v.is_finite()) {
        return false;
    }
    let u = player[2] - bay::ORIGIN[1];
    let v = player[0] - bay::ORIGIN[0];
    let margin = if already_in { MARGIN } else { 0.0 };
    let [u0, u1, v0, v1] = bay::PAD;
    (u0 - margin..=u1 + margin).contains(&u) && (v0 - margin..=v1 + margin).contains(&v)
}

/// Sends one event to the page's visit, when the page keeps one.
#[cfg(target_arch = "wasm32")]
pub fn send(name: &str, detail: &str) {
    use wasm_bindgen::{JsCast, JsValue};
    let Ok(track) = js_sys::Reflect::get(&js_sys::global(), &"oaTrack".into()) else {
        return;
    };
    if let Some(track) = track.dyn_ref::<js_sys::Function>() {
        let _ = track.call2(&JsValue::UNDEFINED, &name.into(), &detail.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_word() {
        let words: Vec<_> = Shape::ALL.into_iter().map(kind).collect();
        assert_eq!(words, ["mac", "gpu", "cpu", "secure", "head", "part"]);
    }

    #[test]
    fn the_bay_is_its_outline_with_a_little_give_on_the_way_out() {
        let middle = bay::world(10.0, 10.0);
        assert!(in_bay(middle, false));
        // The spawn, by the plaza's center, is outside.
        assert!(!in_bay([0.0, 0.0, 0.0], false));
        let edge = bay::world(bay::PAD[1] + 0.5, 10.0);
        assert!(!in_bay(edge, false));
        assert!(in_bay(edge, true));
        assert!(!in_bay([f64::NAN, 0.0, 0.0], true));
    }
}
