//! `openagents cad`: CAD Lab from the terminal (docs/cad-lab/mvp.md). The
//! same tools the web chat calls, on the CAD Lab service on this computer
//! (`cad-lab serve`, `http://127.0.0.1:8794`), which is started when it
//! isn't running and its program is here.

use std::path::PathBuf;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents cad COMMAND [DOC] [options]
  list  The CAD documents.
  open DOC [--version N]  Its parts, instances, joints, mass and checks.
  check DOC [--version N]  Rebuild it and run the checks again.
  versions DOC  Who made each version and what it changed.
  compare DOC [--a N] [--b N]  What changed between two versions.
  render DOC [--view hero|rear|plan|front|side|iso|under|detail]
                 [--focus ID,ID] [--version N] [--width W] [--height H]
                 A picture of a version.
  export DOC [--formats glb,3mf,step,stl,urdf,feed] [--version N]  Files of a
                 version (urdf: a robot for Robot Lab; feed: its mass
                 properties for Lagrange 2).
  place DOC [--zone everglade|meteor-showcase|meteor-stress-test]
                 [--at X,Z] [--heading DEG] [--scale S] [--version N] [--remove]
                 Put a version in a Verse zone; a newer one replaces it.
  drawing DOC [--views front,top,right,iso] [--sections front@0]
                 [--angle third|first] [--sheet A4|A3|A2] [--title TEXT]
                 [--formats svg,pdf,dxf] [--version N]
                 A 2D drawing: projected views with hidden lines dashed,
                 sections, dimensions and a title block.
  tool NAME --json ARGS  Any cad_* tool with its JSON arguments (new_part,
                 add_feature, edit_feature, set_part, assemble ...).
Design parts and assemblies on this computer (docs/cad-lab/mvp.md). Every
edit is rebuilt, checked (closed solids, no undeclared overlaps) and measured
before it's saved as a version. CAD_LAB_URL moves the Lab; CAD_LAB_BIN names
the program that starts it when it isn't running.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("open", Effect::ReadOnly),
    Declared::computer("check", Effect::ReadOnly),
    Declared::computer("versions", Effect::ReadOnly),
    Declared::computer("compare", Effect::ReadOnly),
    Declared::computer("render", Effect::LocalWrite),
    Declared::computer("export", Effect::LocalWrite),
    Declared::computer("place", Effect::LocalWrite),
    Declared::computer("drawing", Effect::LocalWrite),
    Declared::computer("tool", Effect::LocalWrite),
];

fn url() -> String {
    std::env::var("CAD_LAB_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8794".into())
        .trim_end_matches('/')
        .to_owned()
}

/// The `cad-lab` program: `CAD_LAB_BIN`, beside this one, or on PATH.
fn program() -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("cad-lab")));
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("cad-lab"))
            .find(|p| p.is_file())
    });
    [
        std::env::var_os("CAD_LAB_BIN").map(PathBuf::from),
        beside,
        on_path,
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

fn up(http: &reqwest::blocking::Client, base: &str) -> bool {
    http.get(format!("{base}/healthz"))
        .timeout(Duration::from_secs(1))
        .send()
        .is_ok_and(|r| r.status().is_success())
}

fn ensure(http: &reqwest::blocking::Client, base: &str) -> Result<(), String> {
    if up(http, base) {
        return Ok(());
    }
    let program = program().ok_or_else(|| {
        format!(
            "CAD Lab isn't running at {base}; start it with: ~/work/openagents/scripts/dev/full-local.sh start --cad"
        )
    })?;
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab/cad");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(dir.join("serve.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new(&program)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("CAD Lab couldn't start ({e})"))?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if up(http, base) {
            return Ok(());
        }
    }
    Err(format!(
        "CAD Lab didn't start; see {}",
        dir.join("serve.log").display()
    ))
}

/// The tool and its arguments for the command line, or the usage problem.
pub(crate) fn request(words: &[String]) -> Result<(String, Value), String> {
    let args = Args::parse(words, &["remove"])?;
    let positional = args.positional();
    let Some(command) = positional.first() else {
        return Err("say which command".into());
    };
    let allowed: &[&str] = match command.as_str() {
        "list" | "versions" => &[],
        "open" | "check" => &["version"],
        "compare" => &["a", "b"],
        "render" => &["view", "focus", "version", "width", "height", "zoom", "yaw"],
        "export" => &["formats", "version"],
        "place" => &["zone", "at", "heading", "scale", "version"],
        "drawing" => &[
            "views", "sections", "angle", "sheet", "title", "formats", "version",
        ],
        "tool" => &["json"],
        other => return Err(format!("unknown command `{other}`")),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|n| !allowed.contains(n))
    {
        return Err(format!("`cad {command}` takes no --{name}"));
    }
    if args.switch("remove") && command != "place" {
        return Err(format!("`cad {command}` takes no --remove"));
    }
    if command == "tool" {
        let Some(name) = positional.get(1) else {
            return Err("`cad tool` needs a tool name, e.g. add_feature".into());
        };
        let name = if name.starts_with("cad_") {
            name.clone()
        } else {
            format!("cad_{name}")
        };
        let call: Value = match args.option("json") {
            Some(text) if std::path::Path::new(text).is_file() => {
                let text = std::fs::read_to_string(text).map_err(|e| format!("{text}: {e}"))?;
                serde_json::from_str(&text).map_err(|e| format!("--json isn't JSON ({e})"))?
            }
            Some(text) => {
                serde_json::from_str(text).map_err(|e| format!("--json isn't JSON ({e})"))?
            }
            None => json!({}),
        };
        if !call.is_object() {
            return Err("--json must be a JSON object".into());
        }
        return Ok((name, call));
    }
    if positional.len() > 2 || (command == "list" && positional.len() > 1) {
        return Err(format!(
            "unexpected word `{}`",
            positional.last().map_or("", |w| w.as_str())
        ));
    }
    let mut call = json!({});
    if command != "list" {
        let Some(doc) = positional.get(1) else {
            return Err(format!("`cad {command}` needs the document, e.g. rover"));
        };
        call["doc"] = json!(doc);
    }
    for name in allowed {
        if let Some(value) = args.option(name) {
            call[*name] = match *name {
                "focus" | "formats" | "views" | "sections" => json!(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                ),
                "version" | "a" | "b" | "width" | "height" => value
                    .parse::<u64>()
                    .map_or_else(|_| json!(value), |n| json!(n)),
                "zoom" | "yaw" | "heading" | "scale" => value
                    .parse::<f64>()
                    .map_or_else(|_| json!(value), |n| json!(n)),
                "at" => {
                    let numbers: Result<Vec<f64>, _> =
                        value.split(',').map(|n| n.trim().parse::<f64>()).collect();
                    match numbers {
                        Ok(list) if matches!(list.len(), 2 | 3) => json!(list),
                        _ => return Err("--at is X,Z or X,Y,Z in metres".into()),
                    }
                }
                _ => json!(value),
            };
        }
    }
    if args.switch("remove") {
        call["remove"] = json!(true);
    }
    Ok((format!("cad_{command}"), call))
}

/// The result in words.
fn words_for(value: &Value) -> String {
    let mut text = String::new();
    if let Some(list) = value["documents"].as_array() {
        for doc in list {
            text.push_str(&format!(
                "{} \"{}\" ({} versions, {} kg)\n",
                doc["id"].as_str().unwrap_or_default(),
                doc["title"].as_str().unwrap_or_default(),
                doc["versions"],
                doc["mass_kg"]
            ));
        }
    }
    if let Some(list) = value["versions"].as_array() {
        for v in list {
            text.push_str(&format!(
                "v{}: {} (by {}, {} kg, {} parts)\n",
                v["n"],
                v["summary"].as_str().unwrap_or_default(),
                v["author"].as_str().unwrap_or_default(),
                v["mass_kg"],
                v["parts"]
            ));
        }
    }
    if let (Some(doc), Some(n)) = (value["doc"].as_str(), value["version"].as_u64()) {
        text.push_str(&format!("{doc} v{n}"));
        if let Some(summary) = value["summary"].as_str() {
            text.push_str(&format!(": {summary}"));
        }
        text.push('\n');
    }
    let mass = if value["mass"].is_object() {
        &value["mass"]
    } else {
        &value["report"]["mass"]
    };
    if let Some(kg) = mass["mass_kg"].as_f64() {
        text.push_str(&format!(
            "Mass {kg} kg, centre of mass {} mm\n",
            mass["center_of_mass_mm"]
        ));
    }
    if value["passed"] == false {
        text.push_str("The checks failed:\n");
    }
    for key in ["problems", "differences"] {
        for line in value[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            text.push_str(&format!("  {line}\n"));
        }
    }
    if let Some(path) = value["path"].as_str() {
        text.push_str(&format!("Picture: {path}\n"));
    }
    for f in value["files"].as_array().into_iter().flatten() {
        text.push_str(&format!(
            "{}: {} ({} bytes)\n",
            f["format"].as_str().unwrap_or_default(),
            f["path"].as_str().unwrap_or_default(),
            f["bytes"]
        ));
    }
    text.trim().to_owned()
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty()
        || words
            .first()
            .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let (tool, call) = match request(words) {
        Ok(found) => found,
        Err(message) => return output.usage("cad", &message, USAGE),
    };
    let base = url();
    let http = match reqwest::blocking::Client::builder()
        .default_headers(crate::keys::test_run_headers())
        .timeout(Duration::from_secs(600))
        .build()
    {
        Ok(http) => http,
        Err(e) => return output.fail("cad", &e.to_string()),
    };
    if let Err(message) = ensure(&http, &base) {
        return output.fail("cad", &message);
    }
    let result: Value = match http
        .post(format!("{base}/api/tool/{tool}"))
        .json(&call)
        .send()
        .and_then(|r| r.json())
    {
        Ok(result) => result,
        Err(e) => {
            return output.fail(
                "cad",
                &format!("CAD Lab didn't answer ({})", e.without_url()),
            );
        }
    };
    if let Some(error) = result["error"].as_str() {
        return output.fail("cad", error);
    }
    let failed = result["passed"] == false;
    output.emit(&result, words_for);
    if failed { crate::EXIT_FAILURE } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn commands_become_cad_tool_calls() {
        let (tool, call) = request(&words("compare rover --a 1")).unwrap();
        assert_eq!(tool, "cad_compare");
        assert_eq!(call, json!({"doc": "rover", "a": 1}));
        let (tool, call) =
            request(&words("render rover --view rear --focus camera_head,mast")).unwrap();
        assert_eq!(tool, "cad_render");
        assert_eq!(call["focus"], json!(["camera_head", "mast"]));
        let (tool, call) = request(&words("tool add_feature --json {\"doc\":\"rover\"}")).unwrap();
        assert_eq!(tool, "cad_add_feature");
        assert_eq!(call["doc"], "rover");
        assert_eq!(request(&words("list")).unwrap().0, "cad_list");
        let (tool, call) = request(&words(
            "place rover --zone everglade --at 3,-4 --heading 30 --version 38",
        ))
        .unwrap();
        assert_eq!(tool, "cad_place");
        assert_eq!(
            call,
            json!({"doc": "rover", "zone": "everglade", "at": [3.0, -4.0], "heading": 30.0, "version": 38})
        );
        let (_, call) = request(&words("place rover --remove")).unwrap();
        assert_eq!(call["remove"], true);
        assert!(request(&words("place rover --at north")).is_err());
        assert!(request(&words("open rover --remove")).is_err());
        let (tool, call) = request(&words(
            "drawing rover --views front,top --sections front@0 --sheet A4",
        ))
        .unwrap();
        assert_eq!(tool, "cad_drawing");
        assert_eq!(
            call,
            json!({"doc": "rover", "views": ["front", "top"], "sections": ["front@0"], "sheet": "A4"})
        );
    }

    #[test]
    fn mistakes_are_plain() {
        assert!(
            request(&words("open"))
                .unwrap_err()
                .contains("needs the document")
        );
        assert!(
            request(&words("open rover --quality x"))
                .unwrap_err()
                .contains("takes no --quality")
        );
        assert!(
            request(&words("delete rover"))
                .unwrap_err()
                .contains("unknown command")
        );
        assert!(
            request(&words("tool add_feature --json [1]"))
                .unwrap_err()
                .contains("JSON object")
        );
    }

    #[test]
    fn results_read_in_words() {
        let text = words_for(
            &json!({"doc": "rover", "version": 3, "summary": "Six wheels",
            "mass": {"mass_kg": 41.2, "center_of_mass_mm": [1, 2, 3]}}),
        );
        assert!(
            text.contains("rover v3: Six wheels") && text.contains("Mass 41.2 kg"),
            "{text}"
        );
        let text = words_for(&json!({"passed": false, "problems": ["a and b overlap"]}));
        assert!(text.contains("checks failed") && text.contains("a and b overlap"));
    }
}
