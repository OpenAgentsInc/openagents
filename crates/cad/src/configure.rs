//! Bounded, explicitly chosen dimensions for a shared design snapshot.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub path: String,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

fn literal(value: &Value) -> Option<(f64, String)> {
    if let Some(number) = value.as_f64() {
        return number.is_finite().then_some((number, String::new()));
    }
    let text = value.as_str()?.trim();
    let number = text
        .strip_suffix("mm")
        .unwrap_or(text)
        .trim()
        .parse::<f64>()
        .ok()?;
    number.is_finite().then(|| {
        (
            number,
            if text.ends_with("mm") {
                " mm".into()
            } else {
                String::new()
            },
        )
    })
}
fn escaped(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// Numeric feature fields and literal part parameters that an owner may expose.
pub fn options(document: &Value) -> Vec<Value> {
    fn walk(value: &Value, path: &str, label: &str, out: &mut Vec<Value>, depth: usize) {
        if depth > 12 || out.len() >= 128 {
            return;
        }
        if let Some(number) = value.as_f64().filter(|n| n.is_finite()) {
            out.push(json!({"path":path,"label":label,"value":number}));
            return;
        }
        match value {
            Value::Object(fields) => {
                for (key, v) in fields {
                    walk(
                        v,
                        &format!("{path}/{}", escaped(key)),
                        &format!("{label} {key}"),
                        out,
                        depth + 1,
                    )
                }
            }
            Value::Array(fields) => {
                for (i, v) in fields.iter().enumerate() {
                    walk(
                        v,
                        &format!("{path}/{i}"),
                        &format!("{label} {}", i + 1),
                        out,
                        depth + 1,
                    )
                }
            }
            _ => (),
        }
    }
    fn part(part: &Value, path: &str, label: &str, out: &mut Vec<Value>) {
        for (i, p) in part["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some((value, _)) = literal(&p["expression"]) {
                out.push(json!({"path":format!("{path}/parameters/{i}/expression"),"label":format!("{label} {}",p["name"].as_str().unwrap_or("parameter")),"value":value}));
            }
        }
        for (i, f) in part["features"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if f["suppressed"] != true {
                walk(
                    &f["params"],
                    &format!("{path}/features/{i}/params"),
                    &format!("{label} {}", f["id"].as_str().unwrap_or("feature")),
                    out,
                    0,
                );
            }
        }
    }
    let mut out = Vec::new();
    if document["kind"] == "part.v1" {
        part(
            document,
            "",
            document["id"].as_str().unwrap_or("part"),
            &mut out,
        );
    } else if let Some(parts) = document["parts"].as_object() {
        for (id, p) in parts {
            part(p, &format!("/parts/{}", escaped(id)), id, &mut out);
        }
    }
    out.truncate(128);
    out
}

/// Validate controls against the snapshot, then change a private copy.
/// A complete CAD rebuild must still succeed before the result is used.
pub fn apply(
    document: &Value,
    controls: &[Control],
    values: Option<&[f64]>,
) -> Result<Value, String> {
    if controls.is_empty()
        || controls.len() > 8
        || values.is_some_and(|v| v.len() != controls.len())
    {
        return Err("Choose from one to eight sliders, with one value for each.".into());
    }
    let options = options(document);
    let mut seen = std::collections::BTreeSet::new();
    let mut result = document.clone();
    for (i, c) in controls.iter().enumerate() {
        let option = options
            .iter()
            .find(|o| o["path"] == c.path)
            .ok_or("Choose a numeric feature dimension or a literal part parameter.")?;
        let initial = option["value"].as_f64().unwrap();
        if !seen.insert(&c.path)
            || c.label.trim().is_empty()
            || c.label.len() > 80
            || c.label.chars().any(char::is_control)
            || ![c.min, c.max, c.step].iter().all(|n| n.is_finite())
            || c.min >= c.max
            || c.step <= 0.0
            || c.min.abs() > 1_000_000.0
            || c.max.abs() > 1_000_000.0
            || c.step > c.max - c.min
            || initial < c.min
            || initial > c.max
        {
            return Err("Give each slider a name, a positive step, and limits that include its starting value.".into());
        }
        let value = values.map_or(initial, |v| v[i]);
        if !value.is_finite() || value < c.min || value > c.max {
            return Err(format!("Keep {} between {} and {}.", c.label, c.min, c.max));
        }
        let target = result
            .pointer_mut(&c.path)
            .ok_or("This dimension isn't in the design.")?;
        let (_, unit) = literal(target).ok_or("Choose a numeric dimension.")?;
        *target = if target.is_string() {
            json!(format!("{value}{unit}"))
        } else {
            json!(value)
        };
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chosen_values_change_only_the_copy_and_refuse_other_fields() {
        let source = json!({"kind":"part.v1","id":"plate","parameters":[{"name":"spacing","expression":"24 mm"}],"features":[{"id":"body","op":"solid.box","params":{"height":4,"center":[0,0,2]}}]});
        let controls = vec![
            Control {
                path: "/parameters/0/expression".into(),
                label: "Hole spacing".into(),
                min: 10.,
                max: 40.,
                step: 1.,
            },
            Control {
                path: "/features/0/params/height".into(),
                label: "Thickness".into(),
                min: 2.,
                max: 12.,
                step: 0.5,
            },
        ];
        let changed = apply(&source, &controls, Some(&[30., 8.])).unwrap();
        assert_eq!(changed["parameters"][0]["expression"], "30 mm");
        assert_eq!(changed["features"][0]["params"]["height"], 8.);
        assert_eq!(source["features"][0]["params"]["height"], 4);
        assert!(apply(&source, &controls, Some(&[300., 8.])).is_err());
        let mut forbidden = controls.clone();
        forbidden[0].path = "/id".into();
        assert!(apply(&source, &forbidden, None).is_err());
        let mut duplicate = controls.clone();
        duplicate.push(controls[0].clone());
        assert!(apply(&source, &duplicate, None).is_err());
    }
}
