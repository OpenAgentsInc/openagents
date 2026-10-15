//! The CAD engine in the browser (docs/cad-lab/README.md, "In the browser").
//!
//! The same engine as `crates/cad` (SolidCraft's kernel on Truck, our
//! documents and checks), built for `wasm32-unknown-unknown`. A page holds
//! the documents itself (in its own storage), so the browser runs the
//! `cad_*` tools that work on a document it hands over, with the same
//! arguments and answers as the Lab's:
//!
//! - `cad_check {document}`: rebuild and check; the report (mass, centre of
//!   mass, inertia, counts) or the problems in plain words;
//! - `cad_solid {ops, sketches?, material?}`: the Solid API;
//! - `cad_faces {document, part, after?}`: faces and edges by name;
//! - `cad_import_dxf {dxf, scale?, at?}`: a DXF drawing as sketch steps;
//! - `cad_export_dxf {document, part, sketch}` or `{document, plane, at}`;
//! - `cad_export {document, format}` through [`export`] (`export_file` in
//!   JavaScript; bytes: glb, step,
//!   3mf, stl).
//!
//! Versions, renders and the file store stay with the Lab on a computer;
//! in the browser the kernel's files live in memory (`/cad/parts`), so a
//! part that hasn't changed is read back rather than rebuilt while the
//! page is open.

use serde_json::{Value, json};

/// A JSON argument given as JSON or as text holding JSON.
fn json_arg(args: &Value, key: &str) -> Result<Option<Value>, String> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
            serde_json::from_str(text)
                .map(Some)
                .map_err(|e| format!("{key} isn't valid JSON ({e})."))
        }
        other => Ok(Some(other.clone())),
    }
}

fn document(args: &Value) -> Result<Value, String> {
    json_arg(args, "document")?
        .filter(Value::is_object)
        .ok_or_else(|| "Give the document (a part.v1 or assembly.v1) as JSON.".to_owned())
}

fn list(args: &Value, key: &str) -> Result<Vec<Value>, String> {
    Ok(match json_arg(args, key)? {
        None => Vec::new(),
        Some(Value::Array(items)) => items,
        Some(other) => vec![other],
    })
}

/// Runs one tool on `args` (a JSON object); the answer is JSON, a refusal
/// `{"error": why}`.
#[must_use]
pub fn run(name: &str, args: &str) -> String {
    let args: Value = match serde_json::from_str(args) {
        Ok(v) => v,
        Err(e) => return json!({"error": format!("The arguments aren't JSON ({e}).")}).to_string(),
    };
    tool(name, &args)
        .unwrap_or_else(|why| json!({"error": why}))
        .to_string()
}

fn tool(name: &str, args: &Value) -> Result<Value, String> {
    let cache = cad::build::default_cache();
    match name {
        "cad_check" => Ok(match cad::evaluate_with(&document(args)?, &cache) {
            Ok(eval) => json!({"passed": true, "report": eval.report}),
            Err(problems) => json!({"passed": false, "problems": problems}),
        }),
        "cad_solid" => {
            let ops = list(args, "ops")?;
            if ops.is_empty() {
                return Err("Give ops: a list of direct steps.".into());
            }
            let material = args["material"]
                .as_str()
                .unwrap_or(cad::doc::DEFAULT_MATERIAL);
            let eval =
                cad::solid(&ops, &list(args, "sketches")?, material, &cache).map_err(|p| {
                    format!("Those operations don't make a valid solid: {}", p.join(" "))
                })?;
            Ok(
                json!({"mass": eval.report["mass"], "size_mm": eval.report["size_mm"],
                      "solids": eval.report["solids"], "checks": eval.report["checks"]}),
            )
        }
        "cad_faces" => {
            let part = args["part"].as_str().ok_or("Name the part (part).")?;
            cad::build::faces_of(&document(args)?, part, args["after"].as_str(), &cache)
                .map_err(|p| p.join(" "))
        }
        "cad_import_dxf" => {
            let text = args["dxf"]
                .as_str()
                .ok_or("Give the drawing's DXF text (dxf).")?;
            let at = match json_arg(args, "at")? {
                Some(v) => [
                    v[0].as_f64().ok_or("at is [x, y] in mm.")?,
                    v[1].as_f64().ok_or("at is [x, y] in mm.")?,
                ],
                None => [0.0, 0.0],
            };
            let draw = cad::dxf::import(text, args["scale"].as_f64().unwrap_or(1.0), at)?;
            Ok(json!({"draw": draw}))
        }
        "cad_export_dxf" => {
            let doc = document(args)?;
            if let Some(sketch) = args["sketch"].as_str() {
                let part = args["part"]
                    .as_str()
                    .ok_or("Name the part (part) the sketch is in.")?;
                let (dxf, entities) =
                    cad::build::sketch_dxf(&doc, part, sketch, &cache).map_err(|p| p.join(" "))?;
                return Ok(json!({"dxf": dxf, "entities": entities}));
            }
            let eval = cad::evaluate_with(&doc, &cache).map_err(|p| p.join(" "))?;
            let plane = args["plane"].as_str().unwrap_or("XY").to_uppercase();
            let (dxf, lines) =
                cad::dxf::section(&eval.built, &plane, args["at"].as_f64().unwrap_or(0.0))?;
            Ok(json!({"dxf": dxf, "entities": lines}))
        }
        "cad_export" => {
            Err("cad_export hands back bytes: call export_file(document, format).".into())
        }
        other => Err(format!(
            "{other} isn't a tool the browser runs; it runs {}.",
            TOOLS.join(", ")
        )),
    }
}

/// The tools the browser build runs.
pub const TOOLS: [&str; 6] = [
    "cad_check",
    "cad_solid",
    "cad_faces",
    "cad_import_dxf",
    "cad_export_dxf",
    "cad_export",
];

/// A document exported as `format` (glb, step, 3mf or stl).
///
/// # Errors
/// The document doesn't pass its checks, or the format is unknown.
pub fn export(document: &str, format: &str) -> Result<Vec<u8>, String> {
    let doc: Value =
        serde_json::from_str(document).map_err(|e| format!("The document isn't JSON ({e})."))?;
    let mut eval =
        cad::evaluate_with(&doc, &cad::build::default_cache()).map_err(|p| p.join(" "))?;
    cad::export::bytes(&mut eval.built, format)
}

/// The JSON Schemas of `part.v1` and `assembly.v1`.
#[must_use]
pub fn schema() -> String {
    cad::doc::schema().to_string()
}

#[cfg(target_arch = "wasm32")]
mod web {
    use wasm_bindgen::prelude::*;

    /// Runs a `cad_*` tool: `run("cad_check", JSON.stringify({document}))`.
    #[wasm_bindgen]
    pub fn run(name: &str, args: &str) -> String {
        super::run(name, args)
    }

    /// A document as file bytes: glb, step, 3mf or stl (`export` is a word
    /// JavaScript keeps for itself).
    #[wasm_bindgen]
    pub fn export_file(document: &str, format: &str) -> Result<Vec<u8>, JsValue> {
        super::export(document, format).map_err(|why| JsValue::from_str(&why))
    }

    /// The document schemas, as JSON.
    #[wasm_bindgen]
    pub fn schema() -> String {
        super::schema()
    }

    /// The tools this build runs, as JSON.
    #[wasm_bindgen]
    pub fn tools() -> String {
        serde_json::to_string(&super::TOOLS).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part() -> Value {
        json!({"kind": "part.v1", "id": "puck", "material": "Steel", "features": [
            {"id": "disc", "op": "solid.cylinder", "params": {"radius": 10, "height": 5}},
            {"id": "round", "op": "solid.fillet", "params": {"radius": 1, "edges": [{"of": "disc:top"}]}}]})
    }

    #[test]
    fn the_browser_tools_answer_like_the_labs() {
        let checked: Value =
            serde_json::from_str(&run("cad_check", &json!({"document": part()}).to_string()))
                .unwrap();
        assert_eq!(checked["passed"], true, "{checked}");
        let kg = checked["report"]["mass"]["mass_kg"].as_f64().unwrap();
        assert!(kg > 0.01 && kg < 0.0125, "{kg}");
        let faces: Value = serde_json::from_str(&run(
            "cad_faces",
            &json!({"document": part().to_string(), "part": "puck", "after": "disc"}).to_string(),
        ))
        .unwrap();
        assert!(faces.to_string().contains("disc:top"), "{faces}");
        let bad: Value = serde_json::from_str(&run("cad_render", "{}")).unwrap();
        assert!(bad["error"].as_str().unwrap().contains("cad_check"));
        let glb = export(&part().to_string(), "glb").unwrap();
        assert_eq!(&glb[..4], b"glTF");
        let step = export(&part().to_string(), "step").unwrap();
        assert!(String::from_utf8_lossy(&step).contains("ISO-10303-21"));
        let stl = export(&part().to_string(), "stl").unwrap();
        assert!(stl.len() > 84, "{}", stl.len());
    }
}
