//! Face pulls expressed as feature edits, shared by the Lab and browser.
use serde_json::{Value, json};

/// Changes the feature that made a named face. `delta` is an outward pull
/// in millimeters. Returns the changed feature and dimension for the UI.
/// The caller must rebuild and check the complete document before saving.
pub fn apply(document: &mut Value, part: &str, face: &str, delta: f64) -> Result<Value, String> {
    if !delta.is_finite() || delta.abs() > 100_000.0 {
        return Err("Choose a face movement within 100 meters.".into());
    }
    let (feature, role) = face.split_once(':').ok_or("Pick a named face first.")?;
    let role = role.split('#').next().unwrap_or(role);
    let body = if document["kind"] == "part.v1" && document["id"] == part {
        document
    } else {
        document["parts"]
            .get_mut(part)
            .ok_or("That part isn't in this design.")?
    };
    let f = body["features"]
        .as_array_mut()
        .and_then(|features| features.iter_mut().find(|f| f["id"] == feature))
        .ok_or("The feature that made this face couldn't be found.")?;
    if f["suppressed"] == true {
        return Err("This feature is switched off.".into());
    }
    let op = f["op"].as_str().unwrap_or_default().to_owned();
    let p = &mut f["params"];
    let mut shift = None;
    let key = match op.as_str() {
        "solid.extrude" if role == "top" => "distance",
        "solid.box" => {
            let (axis, sign, key) = match role {
                "+x" => (0, 1.0, "length"),
                "-x" => (0, -1.0, "length"),
                "+y" => (1, 1.0, "width"),
                "-y" => (1, -1.0, "width"),
                "+z" => (2, 1.0, "height"),
                "-z" => (2, -1.0, "height"),
                _ => return Err("Pick one of the box's six flat faces.".into()),
            };
            shift = Some((axis, sign));
            key
        }
        "solid.fillet" if role.starts_with("blend:") => "radius",
        _ => {
            return Err(
                "Pick an extrusion's end, a box face, or a rounded fillet face to pull it.".into(),
            );
        }
    };
    if p["operation"] == "cut" {
        return Err("Edit this cut's dimensions in chat.".into());
    }
    let old = p[key]
        .as_f64()
        .ok_or("This dimension uses a formula. Change its parameter in chat.")?;
    let value = old + delta;
    if !value.is_finite() || value <= 0.001 || value > 1_000_000.0 {
        return Err("Keep the dimension above 0.001 mm and below 1000 meters.".into());
    }
    // Keep the opposite box face fixed instead of moving both faces.
    if let Some((axis, sign)) = shift {
        let mut center = match p.get("center") {
            None => [0.0; 3],
            Some(v) => crate::doc::triple(v)
                .ok_or("This box's center uses a formula. Change it in chat.")?,
        };
        center[axis] += sign * delta / 2.0;
        p["center"] = json!(center);
    }
    p[key] = json!(value);
    Ok(json!({"feature":feature,"dimension":key,"value":value,"unit":"mm"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn box_pulls_keep_the_opposite_face_and_expressions_are_not_overwritten() {
        let mut doc = json!({"kind":"part.v1","id":"plate","features":[{"id":"body","op":"solid.box","params":{"length":20,"width":10,"height":5,"center":[0,0,2.5]}}]});
        let edit = apply(&mut doc, "plate", "body:-x", 4.0).unwrap();
        assert_eq!(edit["value"], 24.0);
        assert_eq!(doc["features"][0]["params"]["center"][0], -2.0);
        assert_eq!(doc["features"][0]["params"]["height"], 5);
        assert!(apply(&mut doc, "plate", "body:-x", -25.0).is_err());
        doc["features"][0]["params"]["length"] = json!("$length");
        assert!(
            apply(&mut doc, "plate", "body:+x", 2.0)
                .unwrap_err()
                .contains("formula")
        );
        assert!(apply(&mut doc, "missing", "body:+x", 2.0).is_err());
    }
    #[test]
    fn extrusion_and_fillet_pulls_target_only_their_own_feature() {
        let mut doc = json!({"parts":{"p":{"features":[{"id":"body","op":"solid.extrude","params":{"distance":5,"sketch":"outline"}},{"id":"round","op":"solid.fillet","params":{"radius":1}}]}}});
        assert_eq!(
            apply(&mut doc, "p", "body:top#[hole:side]", 2.0).unwrap()["value"],
            7.0
        );
        assert!(apply(&mut doc, "p", "body:side:l1", 2.0).is_err());
        assert_eq!(
            apply(&mut doc, "p", "round:blend:body:top", 0.5).unwrap()["value"],
            1.5
        );
        assert!(apply(&mut doc, "p", "round:blend:x", f64::NAN).is_err());
    }
}
