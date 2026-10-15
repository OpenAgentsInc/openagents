use super::*;

use std::os::unix::fs::PermissionsExt;

fn owner(account: &str) -> String {
    crate::chat_store::account_owner(account)
}

/// A stand-in for `cad-lab`: `tools` prints three declarations, and
/// `tool NAME -` saves the next version of `bracket` the way the Lab lays
/// it out, with a render in `out/`, and answers as the Lab does.
const FAKE: &str = r#"#!/bin/sh
if [ "$1" = tools ]; then
  echo '{"tools":[{"name":"cad_list","description":"List the CAD documents in CAD Lab on this computer.","parameters":{}},{"name":"cad_place","description":"x","parameters":{}},{"name":"cad_import_step","description":"d","parameters":{"type":"object","properties":{"path":{},"step":{}},"required":["path"]}}]}'
  exit 0
fi
args=$(cat)
case "$args" in *slow*) sleep 30 ;; esac
case "$2" in cad_list) echo '{"documents":[]}'; exit 0 ;; esac
doc=bracket
n=1
while [ -f "$CAD_LAB_DIR/$doc/v$n/doc.json" ]; do n=$((n+1)); done
d="$CAD_LAB_DIR/$doc/v$n"
mkdir -p "$d/out"
printf '{"kind":"part.v1","id":"%s","title":"Fan bracket","n":%s}' "$doc" "$n" > "$d/doc.json"
printf '{"n":%s,"summary":"step %s","author":"chat","created":1760400000,"report":{"mass":{"mass_kg":0.05}}}' "$n" "$n" > "$d/version.json"
printf '[]' > "$d/assets.json"
printf 'glTF' > "$d/model.glb"
printf '{"kind":"cad-view.v1"}' > "$d/view.json"
printf 'png' > "$d/out/hero-1x1.png"
printf '{"saved":true,"doc":"%s","version":%s,"path":"%s/x","card":{"card":"cad_lab","doc":"%s","version":%s,"exports":[{"format":"step","path":"%s/out/x.step","url":"http://127.0.0.1:8794/api/out"}]}}' "$doc" "$n" "$d" "$doc" "$n" "$d"
"#;

struct Fixture {
    _root: tempfile::TempDir,
    store: Store,
    program: PathBuf,
    root: PathBuf,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let program = root.path().join("cad-lab");
    // The child has a fixed production PATH; Nix keeps test tools elsewhere.
    let path = std::env::var("PATH").unwrap().replace('\'', "'\\''");
    let fake = FAKE.replacen(
        "#!/bin/sh\n",
        &format!("#!/bin/sh\nPATH='{path}'\nexport PATH\n"),
        1,
    );
    std::fs::write(&program, fake).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let store = Store::local(root.path().join("store"));
    Fixture {
        root: root.path().to_owned(),
        _root: root,
        store,
        program,
    }
}

fn engine(f: &Fixture, cache: &str) -> Engine {
    let mut engine = Engine::new(f.program.clone());
    engine.cache = f.root.join(cache);
    engine
}

#[test]
fn a_scope_is_an_accounts_own_or_a_projects() {
    let me = owner("acct_1");
    let mine = Scope::personal(&me).unwrap();
    assert_eq!(mine.folder, "cad/me");
    assert!(mine.edits);
    assert!(Scope::personal("0123456789abcdef0123456789abcdef").is_none());
    assert!(Scope::for_turn("0123456789abcdef0123456789abcdef", None, None).is_none());
    assert_eq!(Scope::for_turn(&me, None, None), Some(mine.clone()));
    // A project chat whose project isn't known here gets no CAD tools.
    assert!(Scope::for_turn(&me, Some("prj_1"), None).is_none());
    let project = Scope::project(&owner("acct_home"), "prj_1", false, &me).unwrap();
    assert_eq!(project.folder, "cad/p-prj_1");
    assert!(Scope::project(&me, "../x", true, &me).is_none());
    // A card made here names its scope; reading it back never edits.
    let card = json!({"where": project.where_json()});
    let back = Scope::of_card(&card).unwrap();
    assert_eq!(back.folder, project.folder);
    assert!(!back.edits);
    let forged = json!({"where": {"home": me, "folder": "cad/p-other", "project": null}});
    assert!(Scope::of_card(&forged).is_none());
    assert_eq!(
        project.page("bracket", Some(3)),
        "/cad/bracket?v=3&project=prj_1"
    );
    assert_eq!(mine.page("bracket", None), "/cad/bracket");
}

#[test]
fn steps_are_checked_before_the_engine_runs() {
    let me = owner("acct_1");
    let editor = Scope::personal(&me).unwrap();
    let viewer = Scope::project(&me, "prj_1", false, &me).unwrap();
    let edit = json!({"doc": "bracket", "part": "plate", "features": []});
    assert!(admitted(&editor, "cad_add_feature", &edit).is_ok());
    let refused = admitted(&viewer, "cad_add_feature", &edit).unwrap_err();
    assert!(refused.contains("only its editors"), "{refused}");
    assert!(admitted(&viewer, "cad_open", &json!({"doc": "bracket"})).is_ok());
    assert!(
        admitted(&editor, "cad_place", &json!({}))
            .unwrap_err()
            .contains("Verse")
    );
    assert!(admitted(&editor, "cad_import_step", &json!({"path": "/etc/passwd"})).is_err());
    assert!(admitted(&editor, "shell_run", &json!({})).is_err());
    assert!(admitted(&editor, "cad_open", &json!({"doc": "../x"})).is_err());
    assert!(
        admitted(
            &editor,
            "cad_export",
            &json!({"doc": "a", "formats": ["urdf"]})
        )
        .is_err()
    );
    let big = json!({"doc": "a", "features": "x".repeat(MAX_ARGS_BYTES)});
    assert!(
        admitted(&editor, "cad_add_feature", &big)
            .unwrap_err()
            .contains("smaller steps")
    );
    let render = admitted(
        &editor,
        "cad_render",
        &json!({"doc": "a", "width": 9000, "height": 9000}),
    )
    .unwrap();
    assert_eq!(
        (render["width"].as_u64(), render["height"].as_u64()),
        (Some(1600), Some(1000))
    );
    let named = named_docs(
        &json!({"doc": "rover", "templates": {"w": {"from": {"doc": "wheel"}}},
        "document": "{\"parts\": {\"x\": {\"from\": {\"doc\": \"axle\"}}}}"}),
    );
    assert_eq!(named.len(), 3, "{named:?}");
}

#[test]
fn the_tools_offered_here_leave_out_what_needs_a_mac() {
    let tools = hosted_tools(&[
        json!({"name": "cad_list", "description": "List the CAD documents in CAD Lab on this computer.", "parameters": {}}),
        json!({"name": "cad_place", "description": "x", "parameters": {}}),
        json!({"name": "cad_import_step", "description": "d",
               "parameters": {"properties": {"path": {}, "step": {}}, "required": ["path"]}}),
    ]);
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, ["cad_list", "cad_import_step", "cad_import_file"]);
    assert_eq!(tools[0]["description"], "List the CAD documents.");
    assert!(tools[1]["parameters"]["properties"].get("path").is_none());
    assert_eq!(tools[1]["parameters"]["required"], json!([]));
}

#[tokio::test]
async fn versions_are_kept_per_scope_and_read_back_on_another_server() {
    let f = fixture();
    let me = owner("acct_1");
    let scope = Scope::personal(&me).unwrap();
    let first = engine(&f, "cache-a");
    assert_eq!(first.tools().await.unwrap().len(), 3);
    let one = first
        .run(
            &f.store,
            &scope,
            "cad_new_part",
            &json!({"doc": "bracket", "part": "plate"}),
        )
        .await;
    assert_eq!(one["version"], 1, "{one}");
    // Files by name, not where this server keeps them; the card says where.
    assert_eq!(one["card"]["exports"][0]["path"], "x.step");
    assert!(one["card"]["exports"][0].get("url").is_none());
    assert_eq!(one["card"]["where"]["folder"], "cad/me");
    // Another server (an empty working copy) reads version 1 back and
    // makes version 2.
    let second = engine(&f, "cache-b");
    let two = second
        .run(
            &f.store,
            &scope,
            "cad_add_feature",
            &json!({"doc": "bracket", "part": "plate"}),
        )
        .await;
    assert_eq!(two["version"], 2, "{two}");
    let index = load_index(&f.store, &scope).await.unwrap();
    let entry = &index.docs["bracket"];
    assert_eq!((entry.versions, entry.title.as_str()), (2, "Fan bracket"));
    assert_eq!(entry.history.len(), 2);
    assert_eq!(entry.history[1].summary, "step 2");
    for name in ["doc.json", "version.json", "view.json", "model.glb"] {
        assert!(
            read_file(&f.store, &scope, "bracket", 2, name)
                .await
                .is_some(),
            "{name}"
        );
    }
    assert_eq!(
        read_file(&f.store, &scope, "bracket", 1, "hero-1x1.png")
            .await
            .as_deref(),
        Some(&b"png"[..])
    );
    assert!(
        read_file(&f.store, &scope, "bracket", 1, "../index.json")
            .await
            .is_none()
    );
    // The big files leave this server once kept.
    assert!(
        !f.root
            .join("cache-b")
            .join(scope.local())
            .join("bracket/v2/model.glb")
            .exists()
    );
    // Another account sees none of it.
    let theirs = Scope::personal(&owner("acct_2")).unwrap();
    assert!(load_index(&f.store, &theirs).await.unwrap().docs.is_empty());
    assert!(
        read_file(&f.store, &theirs, "bracket", 1, "doc.json")
            .await
            .is_none()
    );
    // A project's viewer can't change it, and can't delete it.
    let viewer = Scope::project(&me, "prj_1", false, &owner("acct_3")).unwrap();
    let refused = second
        .run(
            &f.store,
            &viewer,
            "cad_add_feature",
            &json!({"doc": "bracket"}),
        )
        .await;
    assert!(refused["error"].as_str().unwrap().contains("editors"));
    assert!(delete(&f.store, &viewer, "bracket").await.is_err());
    // Deleting takes every version, and the working copy follows.
    delete(&f.store, &scope, "bracket").await.unwrap();
    assert!(load_index(&f.store, &scope).await.unwrap().docs.is_empty());
    assert!(
        read_file(&f.store, &scope, "bracket", 1, "doc.json")
            .await
            .is_none()
    );
    let again = second
        .run(
            &f.store,
            &scope,
            "cad_new_part",
            &json!({"doc": "bracket", "part": "plate"}),
        )
        .await;
    assert_eq!(again["version"], 1, "{again}");
}

#[tokio::test]
async fn one_person_cant_run_the_engine_without_end() {
    let f = fixture();
    let me = owner("acct_1");
    let e = engine(&f, "cache");
    for _ in 0..RUNS_PER_HOUR {
        assert!(e.admit(&me));
    }
    assert!(!e.admit(&me));
    assert!(e.admit(&owner("acct_2")));
}

#[test]
fn the_page_lists_versions_and_files() {
    let me = owner("acct_1");
    let scope = Scope::project(&me, "prj_1", true, &me).unwrap();
    let entry = Entry {
        title: "Fan bracket".into(),
        kind: "assembly.v1".into(),
        versions: 2,
        history: vec![
            Version {
                n: 1,
                summary: "New part: bracket".into(),
                author: "chat".into(),
                created: 1_760_400_000,
                mass_kg: Some(0.021),
            },
            Version {
                n: 2,
                summary: "bracket: changed body".into(),
                author: "chat".into(),
                created: 1_760_400_100,
                mass_kg: Some(0.026),
            },
        ],
    };
    let record = json!({"summary": "bracket: changed body", "report": {"mass": {"mass_kg": 0.026, "center_of_mass_mm": [0, 0, 3]}}});
    let html =
        doc_content(&scope, "bracket", &entry, 2, &record, Some("token"), true).into_string();
    assert!(
        html.contains("/cad/bracket/file?v=2&amp;name=view.json&amp;project=prj_1"),
        "{html}"
    );
    assert!(html.contains("data-cad-download=\"step\""));
    assert!(html.contains("data-cad-pull=\"/cad/bracket/pull\""));
    assert!(html.contains("data-cad-pull-save"));
    assert!(html.contains("href=\"/cad/bracket?v=1&amp;project=prj_1\""));
    assert!(html.contains("Delete this design"));
    assert!(
        html.contains("Oct 14, 2025") || html.contains("Oct 13, 2025"),
        "{html}"
    );
    assert!(!html.contains("style="));
    let violations = oa_copy::violations(&html, &[]);
    assert!(violations.is_empty(), "{violations:?}");
    // A viewer sees no delete, and without the browser engine no buttons
    // that can't work.
    let plain = doc_content(&scope, "bracket", &entry, 2, &record, None, false).into_string();
    assert!(!plain.contains("Delete this design"));
    assert!(!plain.contains("data-cad-download"));
    assert!(!plain.contains("data-cad-pull-save"));
    assert!(plain.contains("download=1"));
}

/// The real engine end to end (`OPENAGENTS_CAD_ENGINE=target/release/cad-lab
/// cargo test -p openagents-web --lib real_engine -- --ignored`): a part is
/// made, kept, picked by its face name and made thicker, as a chat would.
#[tokio::test]
#[ignore = "needs a built cad-lab"]
async fn real_engine_makes_a_part_and_changes_a_picked_face() {
    let program =
        PathBuf::from(std::env::var("OPENAGENTS_CAD_ENGINE").expect("set OPENAGENTS_CAD_ENGINE"));
    let f = fixture();
    let mut e = Engine::new(program);
    e.cache = f.root.join("real");
    let scope = Scope::personal(&owner("acct_1")).unwrap();
    let made = e
        .run(
            &f.store,
            &scope,
            "cad_new_part",
            &json!({"doc": "plate", "part": "plate", "material": "Aluminum", "features": [
                {"id": "outline", "op": "sketch", "params": {"plane": "XY", "draw": [
                    {"op": "sketch.rectangle.center", "params": {"center": [0, 0], "corner": [20, 20]}}]}},
                {"id": "body", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 3}}]}),
        )
        .await;
    assert_eq!(made["version"], 1, "{made}");
    let view: Value = serde_json::from_slice(
        &read_file(&f.store, &scope, "plate", 1, "view.json")
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        view["solids"][0]["face_names"]
            .to_string()
            .contains("body:top"),
        "{}",
        view["solids"][0]["face_names"]
    );
    let kg1 = made["mass"]["mass_kg"].as_f64().unwrap();
    let thicker = e
        .run(
            &f.store,
            &scope,
            "cad_edit_feature",
            &json!({"doc": "plate", "part": "plate", "feature": "body", "params": {"distance": 8}}),
        )
        .await;
    assert_eq!(thicker["version"], 2, "{thicker}");
    let kg2 = thicker["mass"]["mass_kg"].as_f64().unwrap();
    assert!((kg2 / kg1 - 8.0 / 3.0).abs() < 0.01, "{kg1} {kg2}");
    assert!(
        read_file(&f.store, &scope, "plate", 2, "hero.png")
            .await
            .is_some()
    );
}

#[tokio::test]
async fn face_pulls_check_current_version_and_editor_rights_before_running() {
    let f = fixture();
    let e = engine(&f, "face-pulls");
    let scope = Scope::personal(&owner("acct_1")).unwrap();
    e.run(
        &f.store,
        &scope,
        "cad_new_part",
        &json!({"doc":"bracket","part":"bracket"}),
    )
    .await;
    let stale = e
        .run(
            &f.store,
            &scope,
            "cad_drag_face",
            &json!({"doc":"bracket","base_version":0,"part":"bracket","face":"body:+z","delta":2}),
        )
        .await;
    assert!(
        stale["error"].as_str().unwrap().contains("Reload"),
        "{stale}"
    );
    let mut viewer = scope.clone();
    viewer.edits = false;
    assert!(admitted(&viewer, "cad_drag_face", &json!({"doc":"bracket"})).is_err());
    let current = e
        .run(
            &f.store,
            &scope,
            "cad_drag_face",
            &json!({"doc":"bracket","base_version":1,"part":"bracket","face":"body:+z","delta":2}),
        )
        .await;
    assert_eq!(current["version"], 2, "{current}");
    assert_eq!(
        load_index(&f.store, &scope).await.unwrap().docs["bracket"].versions,
        2
    );
}

#[tokio::test]
#[ignore = "needs a built cad-lab"]
async fn real_engine_imports_attached_models_and_another_server_restores_their_sources() {
    let program = PathBuf::from(std::env::var_os("OPENAGENTS_CAD_ENGINE").unwrap());
    let f = fixture();
    let mut first = Engine::new(program.clone());
    first.cache = f.root.join("import-first");
    let scope = Scope::personal(&owner("acct_1")).unwrap();
    let made=first.run(&f.store,&scope,"cad_from_json",&json!({"doc":"source","document":{"kind":"part.v1","id":"box","features":[{"id":"body","op":"solid.box","params":{"length":10,"width":12,"height":14,"center":[0,0,7]}}]}})).await;
    assert_eq!(made["saved"], true, "{made}");
    let exports = first
        .run(
            &f.store,
            &scope,
            "cad_export",
            &json!({"doc":"source","formats":["step","stl"]}),
        )
        .await;
    assert!(exports["files"].is_array(), "{exports}");
    let chat = "12345678-1234-4234-8234-123456789abc";
    for (format, kind) in [
        ("step", crate::chat_files::Kind::Step),
        ("stl", crate::chat_files::Kind::Stl),
    ] {
        let bytes = read_file(
            &f.store,
            &scope,
            "source",
            1,
            &format!("out/source-v1.{format}"),
        )
        .await
        .unwrap();
        assert_eq!(crate::chat_files::check(&bytes).unwrap(), kind);
        let file = crate::chat_files::save(
            &f.store,
            &scope.actor,
            chat,
            &format!("source.{format}"),
            kind,
            bytes,
        )
        .await
        .unwrap();
        let doc = format!("imported-{format}");
        let args = json!({"doc":doc,"part":"model","feature":"source","chat":chat,"file":file.id});
        let uploaded = first.run(&f.store, &scope, "cad_import_file", &args).await;
        assert_eq!(uploaded["saved"], true, "{uploaded}");
        if format == "stl" {
            assert_eq!(uploaded["card"]["mesh_bodies"], 1);
        }
        let other = Scope::personal(&owner("acct_other")).unwrap();
        assert!(
            first
                .run(&f.store, &other, "cad_import_file", &args)
                .await
                .get("error")
                .is_some()
        );
        let mut second = Engine::new(program.clone());
        second.cache = f.root.join(format!("import-second-{format}"));
        let reopened = second
            .run(&f.store, &scope, "cad_check", &json!({"doc":doc}))
            .await;
        assert!(reopened.get("error").is_none(), "{reopened}");
        let view = read_file(&f.store, &scope, &doc, 1, "view.json")
            .await
            .unwrap();
        let view: Value = serde_json::from_slice(&view).unwrap();
        assert!(!view["solids"].as_array().unwrap().is_empty());
        let bundle = imports::bundle(&f.store, &scope, &doc, 1).await.unwrap();
        assert_eq!(bundle.len(), 1);
    }
}

#[test]
fn manufacturing_cards_offer_named_face_controls_in_plain_words() {
    let report = json!({"profile":{"process":"print","min_wall_mm":0.8,"nozzle_mm":0.4,"overhang_degrees":45},"complete":true,"findings":[{"part":"plate","instance":"plate","face":"body:+z","check":"wall","value":0.3,"limit":0.8,"unit":"mm","message":"Wall is thinner than the selected minimum."}]});
    let card = json!({"doc":"plate","version":1,"title":"Plate","manufacturing":report});
    let html =
        crate::cad_lab::card_markup("12345678-1234-4234-8234-123456789abc", &card).into_string();
    assert!(html.contains("data-cad-findings"));
    assert!(html.contains("data-cad-faces"));
    assert!(html.contains("body:+z"));
    assert!(html.contains("0.30 mm"));
    assert!(oa_copy::violations(&html, &[]).is_empty());
    let failed=crate::cad_lab::manufacturing(&json!({"profile":{"process":"print"},"complete":false,"error":"The model couldn't be checked."})).into_string();
    assert!(!failed.contains("No problems found"));
    assert!(failed.contains("couldn&#39;t") || failed.contains("couldn't"));
}
