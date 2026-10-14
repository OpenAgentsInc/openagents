//! CAD Lab in the web chat (docs/cad-lab/README.md, docs/cad-lab/mvp.md).
//!
//! When this server is told where a CAD Lab runs (`OPENAGENTS_CAD_LAB`, a
//! loopback URL: the Lab keeps its versions on the Mac it runs on), its
//! tools (`cad_open`, `cad_new_part`, `cad_add_feature`, `cad_edit_feature`,
//! `cad_set_part`, `cad_assemble`, `cad_check`, `cad_versions`,
//! `cad_compare`, `cad_export`, `cad_render`, `cad_list`; the Lab serves
//! their declarations at `/api/tools`) join the other Labs' in one chat
//! turn ([`crate::labs`]): the model's typed function choice is the route.
//! No words of the message are matched here.
//!
//! A tool's result carries a card (`"card": "cad_lab"`): the version, its
//! mass properties and counts. [`thread_entry`] draws it under the answer:
//! a 3D view of the version (WebGL, from its glTF), its picture when WebGL
//! isn't there, the numbers, a render or exports when the turn made them,
//! and Compare with the version before.

use std::time::Duration;

use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::Value;

use crate::App;

/// How long one Lab call may take (an edit rebuilds and checks the model).
const LAB_DEADLINE: Duration = Duration::from_secs(110);
/// The card kind a CAD Lab tool returns.
pub(crate) const CARD: &str = "cad_lab";

/// The CAD Lab this server talks to, when one is configured: loopback only.
pub(crate) fn lab_url() -> Option<String> {
    loopback(&std::env::var("OPENAGENTS_CAD_LAB").ok()?)
}

fn loopback(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/').to_owned();
    (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")).then_some(url)
}

fn client(deadline: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder().timeout(deadline).build().ok()
}

async fn get_json(url: &str, deadline: Duration) -> Result<Value, String> {
    client(deadline)
        .ok_or("no HTTP client")?
        .get(url)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

async fn manifest(lab: &str) -> Result<(Vec<Value>, Vec<Value>), String> {
    let tools = get_json(&format!("{lab}/api/tools"), Duration::from_secs(3)).await?;
    let docs = get_json(&format!("{lab}/api/docs"), Duration::from_secs(3)).await?;
    Ok((
        tools["tools"].as_array().cloned().unwrap_or_default(),
        docs["documents"].as_array().cloned().unwrap_or_default(),
    ))
}

/// One tool call on the Lab: its JSON result.
pub(crate) async fn run_tool(lab: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    if !name.starts_with("cad_")
        || !name[4..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_')
    {
        return Err(format!("{name} isn't a CAD Lab tool"));
    }
    client(LAB_DEADLINE)
        .ok_or("no HTTP client")?
        .post(format!("{lab}/api/tool/{name}"))
        .json(arguments)
        .send()
        .await
        .map_err(|e| format!("CAD Lab didn't answer ({})", e.without_url()))?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// The CAD Lab on this Mac, when one is configured and answering.
pub(crate) async fn manifest_here() -> Option<crate::labs::Lab> {
    let lab = lab_url()?;
    match manifest(&lab).await {
        Ok((tools, documents)) => Some(crate::labs::Lab {
            kind: crate::labs::Kind::Cad,
            url: lab,
            tools,
            documents,
        }),
        Err(why) => {
            eprintln!("openagents-web: cad lab: not answering at {lab}: {why}");
            None
        }
    }
}

/// What the model is told about CAD Lab: the tools, the documents, and a
/// short guide to the features (the kernel's conventions, in our words).
pub(crate) fn guidance(documents: &[Value]) -> String {
    let mut out = String::from(GUIDE);
    out.push_str("\nCAD documents:\n");
    if documents.is_empty() {
        out.push_str("- none yet\n");
    }
    for doc in documents {
        out.push_str(&format!(
            "- {} \"{}\" ({}, {} versions)\n",
            doc["id"].as_str().unwrap_or_default(),
            doc["title"].as_str().unwrap_or_default().replace('"', "'"),
            doc["kind"].as_str().unwrap_or_default(),
            doc["versions"].as_u64().unwrap_or_default(),
        ));
    }
    out
}

const GUIDE: &str = r##"

This chat can design parts and assemblies in CAD Lab, on this Mac, with the cad_* tools. When the message is about a CAD model (make or change a part, place parts, joints, a lid that opens, mass, checks, compare versions, renders, exports), use the cad_* tools, always naming the document (doc). The chat shows a card with the 3D model under your answer, so don't paste links; say in a sentence or two, as "I", what you built and the numbers the tool returned (mass, centre of mass). Every edit is rebuilt and checked; when a tool refuses, read the reason, fix your numbers, and try again (up to three tries) before telling the person what stopped it.

How the model works (millimetres and degrees; X forward, Y left, Z up; the ground is z = 0):
- A document is an assembly: parts (each a feature history), instances that place parts, joints, and contacts.
- cad_new_part makes a part with its features and places one instance with the part's id (place: false to only define it). A part is modelled in its own frame; an instance places that frame at "at" (mm) turned by "rotate" ([rx,ry,rz] degrees, X then Y then Z), relative to its "parent" instance's frame (or the world). Model a part once and place it many times (wheels).
- A feature is {"id":"lowercase_id","op":"...","params":{...}}. Ids are unique in the part; later features refer to earlier ones by id.
- Solid shortcuts: solid.box {length (X), width (Y), height (Z), center:[x,y,z]}; solid.cylinder {radius, height, axis:"X"|"Y"|"Z", base:[x,y,z] = centre of the end it grows from, along +axis}; solid.sphere {radius, center}; solid.torus {major, minor, center}.
- Booleans: give "operation":"join" (add to the part's solid), "cut" (remove), "intersect", or "new" (a separate solid in the part). The first solid needs no operation.
- Sketches: {"id":"profile","op":"sketch","params":{"plane":PLANE,"offset":mm?,"draw":[{"op":"sketch.rectangle.center","params":{"center":[x,y],"corner":[x,y]}}, ...]}}. PLANE is "XY" (sketch x→X, y→Y, extrudes +Z), "YZ" (x→Y, y→Z, extrudes +X), or a frame {"origin":[x,y,z],"x_dir":[..],"y_dir":[..]} (extrudes along x_dir × y_dir). For a side profile in X and Z use {"origin":[0,0,0],"x_dir":[1,0,0],"y_dir":[0,0,1]} (extrudes toward -Y). Avoid "XZ" (its sketch y is world -Z).
- Drawing steps: sketch.rectangle.center {center, corner}; sketch.rectangle.rounded {p0, p1, radius}; sketch.circle.center {center, radius}; sketch.polygon.inscribed {center, radius, sides}; sketch.slot.center_to_center {p0, p1, width} (a rounded bar between two centres: good for arms and links); sketch.line {points:[[x,y],...], closed:true} (any outline); sketch.arc.three_point {start, end, through}; sketch.text {text, at:[x,y] (baseline start), height}.
- solid.extrude {sketch, distance, direction?: "positive"|"negative"|"symmetric" (symmetric puts the distance on EACH side), operation?}; solid.revolve {sketch, axis: "x"|"y" (the sketch's own axes) or "X"|"Y"|"Z", angle? (default 360), operation?}; solid.loft {sections:[{sketch},{sketch}]}; solid.sweep and solid.pipe follow a path sketch.
- Edges and faces are picked by a point on them [x,y,z] in the part's frame: solid.fillet {edges:[[x,y,z],...], radius}; solid.chamfer {edges, distance}; solid.shell {faces:[[x,y,z]] (the faces to remove, e.g. the top face's centre), thickness}: hollow boxes, trays and bays. Fillet outside edges before you shell. A box's vertical corner edge is at [±length/2, ±width/2, its middle height].
- Repeats: solid.pattern.circular {features:[ids], axis:"X"|"Y"|"Z" or {origin, dir}, count} (copies keep the colour of the solid they copy, even when it's coloured later); solid.pattern.rectangular {features:[ids], dir1:[x,y,z], count1, spacing1, dir2?, count2?, spacing2?}; solid.mirror {features:[ids], plane:"XY"|"XZ"|"YZ"}. Pattern a small feature (a tread, a hole) right after you make it, before big cuts: it builds much faster.
- Finish: a part's finish is gloss, satin or matte (cad_new_part or cad_set_part finish); glossy moulded plastic is gloss.
- Colour: the part's color (#rrggbb); a solid of its own colour: make it with "operation":"new" and a feature {"id":"..","op":"appearance.assign","params":{"bodies":["that feature's id"],"color":"#..."}}.
- Materials: Aluminum, Titanium, Steel, Stainless Steel, Brass, Copper, ABS Plastic, PLA, Nylon, Polycarbonate, Glass, Carbon Fibre, Rubber, Circuit Board, Solar Cell, Gold, Foam, Kapton, PVC. Mass comes from the material's density, so hollow big boxes (solid.shell) the way they'd really be built.
- Instances may not overlap. Parts that are meant to touch or sit in each other (a shaft in a boss, a board on a floor) go in contacts [["a","b"]]; a joint's parent and child may touch.
- Joints (cad_assemble joints): {"id","type":"revolute"|"slider"|"cylindrical"|"planar"|"spherical"|"gear"|"fastened","parent","child","origin":[x,y,z],"axis":[x,y,z],"value":deg or mm,"limits":[[min,max]]}. origin and axis are in the parent's frame and the parent must be the child's parent instance. To open a lid, set its hinge's value (cad_assemble with the same joint id and a new value). planar: value [u mm, v mm, deg] in the plane normal to axis; spherical: value [rx,ry,rz] deg about origin, no axis; gear: no value, {"driver": joint id, "ratio": turns per driver turn (negative for meshing gears)}.
- Subassemblies: cad_assemble templates {"bogie":{"instances":[...],"joints":[...]}} defines one from this document's parts; an instance {"id":"bogie_l","assembly":"bogie"} places a copy, whose instances are then named by path ("bogie_l.wheel"). cad_from_json writes a whole document at once.
- Make each change in as few calls as you can: one cad_assemble can move instances and set joints together. When a check refuses an edit, change only what it names (move the instance it names, or declare the contact), not the parts' histories. To undo changes to a part, cad_set_part with from_version (the version whose part to bring back).
- Work it out as numbers first: where each part sits, where it touches its neighbours. When a check refuses an overlap, it says which instances and where; move one, or declare the contact if they are meant to touch.
- To look at it: cad_render {view: hero|rear|plan|front|side|iso|under|detail, focus?: [instance ids]}. To compare: cad_compare (a = the earlier version). Exports: cad_export (glb for the web and Verse, 3mf, step: exact solids).
"##;

/// The CAD Lab cards a reply keeps.
pub(crate) fn cards(meta: &openagents_chat::router::Meta) -> Vec<Value> {
    meta.cards
        .iter()
        .filter(|card| card["card"] == CARD)
        .take(1)
        .cloned()
        .collect()
}

// ---- the card -------------------------------------------------------------------

fn s<'a>(card: &'a Value, key: &str) -> &'a str {
    card[key].as_str().unwrap_or_default()
}

/// A document id or file name as the page's address may carry it.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || "-_.+".contains(*c))
        .collect()
}

fn mass_line(mass: &Value) -> String {
    let Some(kg) = mass["mass_kg"].as_f64() else {
        return String::new();
    };
    let c = &mass["center_of_mass_mm"];
    let at = |i: usize| c[i].as_f64().unwrap_or(0.0);
    // A small part reads in grams and tenths of a millimetre (a 30 g toy
    // isn't "0.03 kg at (0, 0, 14) mm").
    if kg < 1.0 {
        // Never "-0.0".
        let tenth = |i: usize| (at(i) * 10.0).round() / 10.0 + 0.0;
        return format!(
            "Mass {:.1} g · centre of mass ({:.1}, {:.1}, {:.1}) mm",
            kg * 1000.0,
            tenth(0),
            tenth(1),
            tenth(2)
        );
    }
    format!(
        "Mass {kg:.2} kg · centre of mass ({:.0}, {:.0}, {:.0}) mm",
        at(0),
        at(1),
        at(2)
    )
}

fn counts_line(card: &Value) -> String {
    let n = |k: &str| card[k].as_u64().unwrap_or(0);
    let mut line = String::new();
    if let Some(size) = card["size_mm"].as_array().filter(|s| s.len() == 3) {
        let d = |i: usize| size[i].as_f64().unwrap_or(0.0);
        line.push_str(&format!("{:.0} × {:.0} × {:.0} mm · ", d(0), d(1), d(2)));
    }
    let plural = |count: u64, one: &str, many: &str| {
        format!("{count} {}", if count == 1 { one } else { many })
    };
    line.push_str(&format!(
        "{}, {} placed, {}, {}",
        plural(n("parts"), "part", "parts"),
        n("instances"),
        plural(n("features"), "feature", "features"),
        plural(n("joints"), "joint", "joints")
    ));
    line
}

fn file_href(chat: &str, doc: &str, version: u64, name: &str) -> String {
    format!(
        "/chat/{chat}/cad/file?doc={}&v={version}&name={}",
        clean(doc),
        clean(name)
    )
}

fn out_href(chat: &str, doc: &str, version: u64, name: &str) -> String {
    format!(
        "/chat/{chat}/cad/out?doc={}&v={version}&name={}",
        clean(doc),
        clean(name)
    )
}

/// The card for a CAD Lab result: the 3D view (or the picture), the
/// version, mass properties and counts, a render or exports, Compare.
pub(crate) fn card_markup(chat: &str, card: &Value) -> Markup {
    let doc = s(card, "doc");
    let version = card["version"].as_u64().unwrap_or(1);
    let render = card["render"]["file"].as_str();
    html! {
        div.oa-cad-card {
            link rel="stylesheet" href=(ASSET_CSS);
            script src=(ASSET_JS) defer {}
            p.oa-cad-title {
                strong { (s(card, "title")) " · v" (version) }
                " " span.oa-cad-dim { (s(card, "summary")) }
            }
            @if let Some(file) = render {
                img.oa-cad-render src=(out_href(chat, doc, version, file))
                    alt={ (s(card, "title")) ", " (card["render"]["view"].as_str().unwrap_or("view")) " view" };
            } @else {
                div.oa-cad-view data-cad-model=(file_href(chat, doc, version, "model.glb")) {
                    img.oa-cad-still src=(file_href(chat, doc, version, "hero.png"))
                        alt={ (s(card, "title")) ", version " (version) };
                    canvas.oa-cad-canvas hidden aria-label={ (s(card, "title")) ", version " (version) ", drag to turn" } {}
                }
            }
            p.oa-cad-meta { (mass_line(&card["mass"])) }
            p.oa-cad-meta { (counts_line(card)) }
            @if let Some(compare) = card["compare"].as_object() {
                p.oa-cad-dim { "Since v" (compare.get("a").and_then(Value::as_u64).unwrap_or(0)) ":" }
                ul.oa-cad-list {
                    @for line in compare.get("differences").and_then(Value::as_array).into_iter().flatten() {
                        li { (line.as_str().unwrap_or_default()) }
                    }
                }
            }
            @if let Some(files) = card["exports"].as_array().filter(|f| !f.is_empty()) {
                p.oa-cad-meta {
                    "Exports: "
                    @for (i, f) in files.iter().enumerate() {
                        @if i > 0 { " · " }
                        @let name = f["path"].as_str().and_then(|p| p.rsplit('/').next()).unwrap_or_default();
                        a href=(out_href(chat, doc, version, name)) download { (f["format"].as_str().unwrap_or_default().to_uppercase()) }
                    }
                    " (STEP holds the exact solids)"
                }
            }
            div.oa-cad-actions {
                @if version > 1 && card["compare"].is_null() {
                    button.oa-cad-button type="button"
                        hx-get=(format!("/chat/{chat}/cad/card?doc={}&v={version}&a={}", clean(doc), version - 1))
                        hx-target="closest .oa-cad-card" hx-swap="outerHTML" { "Compare with v" (version - 1) }
                }
                @if render.is_some() {
                    button.oa-cad-button type="button"
                        hx-get=(format!("/chat/{chat}/cad/card?doc={}&v={version}", clean(doc)))
                        hx-target="closest .oa-cad-card" hx-swap="outerHTML" { "Turn it in 3D" }
                }
                @if let Some(versions) = card["versions"].as_array() {
                    span.oa-cad-dim { (versions.len()) @if versions.len() == 1 { " version" } @else { " versions" } }
                }
            }
        }
    }
}

/// Where a reply with a CAD Lab card shows it.
pub(crate) fn thread_entry(chat: &str, reply: Option<&openagents_chat::router::Meta>) -> Markup {
    match reply.and_then(|meta| meta.cards.iter().find(|card| card["card"] == CARD)) {
        Some(card) => card_markup(chat, card),
        None => html! {},
    }
}

const ASSET_CSS: &str = "/chat/cad/card.css";
pub(crate) const ASSET_JS: &str = "/chat/cad/card.js";

const CSS: &str = r".oa-cad-card{margin:12px 0;padding:12px;border:1px solid rgba(127,127,127,.28);border-radius:12px;max-width:960px}
.oa-cad-title{margin:0 0 8px}.oa-cad-dim{opacity:.7}
.oa-cad-view{position:relative;width:100%;aspect-ratio:16/10;border-radius:8px;overflow:hidden;background:linear-gradient(#eef1f5,#cfd5de)}
.oa-cad-view img,.oa-cad-canvas{position:absolute;inset:0;width:100%;height:100%;display:block;object-fit:contain;touch-action:none;cursor:grab}
.oa-cad-render{width:100%;border-radius:8px;display:block}
.oa-cad-meta{margin:8px 0 0;font-size:13px;opacity:.85}
.oa-cad-list{margin:6px 0 0;padding-left:20px;font-size:13px}
.oa-cad-actions{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin-top:8px;font-size:13px}
.oa-cad-button{padding:6px 12px;border-radius:8px;border:1px solid rgba(127,127,127,.35);background:transparent;color:inherit;font:inherit}
";

/// The viewer: reads the version's glTF (CAD Lab writes it: one node per
/// solid, positions, normals, indices, a colour per material) and turns it
/// with the pointer. Without WebGL2 the picture stays.
const JS: &str = r"(function () {
  if (window.__oaCad) return;
  window.__oaCad = 1;
  function parse(buf) {
    var dv = new DataView(buf);
    if (dv.getUint32(0, true) !== 0x46546c67) return null;
    var jl = dv.getUint32(12, true);
    var json = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 20, jl)));
    var bin = 20 + jl + 8;
    var out = [];
    json.meshes.forEach(function (m) {
      m.primitives.forEach(function (p) {
        var acc = function (i, T, n) { var a = json.accessors[i], v = json.bufferViews[a.bufferView]; return new T(buf, bin + (v.byteOffset || 0), a.count * n); };
        var mat = json.materials[p.material] || {pbrMetallicRoughness: {baseColorFactor: [0.7, 0.7, 0.7, 1]}};
        var c = mat.pbrMetallicRoughness.baseColorFactor.map(function (x) { return Math.pow(x, 1 / 2.2); });
        var rough = mat.pbrMetallicRoughness.roughnessFactor;
        out.push({pos: acc(p.attributes.POSITION, Float32Array, 3), nor: acc(p.attributes.NORMAL, Float32Array, 3), idx: acc(p.indices, Uint32Array, 1), color: c, gloss: 1 - (rough === undefined ? 0.6 : rough)});
      });
    });
    return out;
  }
  function shader(gl, type, src) { var s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); return s; }
  function start(view) {
    if (view.__started) return;
    view.__started = 1;
    var canvas = view.querySelector('canvas');
    var gl = canvas.getContext('webgl2', {antialias: true});
    if (!gl) return;
    fetch(view.getAttribute('data-cad-model')).then(function (r) { return r.ok ? r.arrayBuffer() : null; }).then(function (buf) {
      var prims = buf && parse(buf);
      if (!prims || !prims.length) return;
      var prog = gl.createProgram();
      gl.attachShader(prog, shader(gl, gl.VERTEX_SHADER, '#version 300 es\nin vec3 p;in vec3 n;uniform mat4 m;out vec3 vn;void main(){vn=n;gl_Position=m*vec4(p,1.0);}'));
      gl.attachShader(prog, shader(gl, gl.FRAGMENT_SHADER, '#version 300 es\nprecision highp float;in vec3 vn;uniform vec3 c;uniform vec3 l;uniform vec3 e;uniform float g;out vec4 o;void main(){vec3 nn=normalize(vn);float d=max(dot(nn,l),0.0);float f=max(dot(nn,normalize(vec3(-0.4,0.3,-0.6))),0.0);float s=pow(max(dot(reflect(-l,nn),e),0.0),6.0+90.0*g*g)*0.6*g*g;o=vec4(c*(0.38+0.55*d+0.18*f)+vec3(s),1.0);}'));
      gl.linkProgram(prog);
      var lo = [1e9, 1e9, 1e9], hi = [-1e9, -1e9, -1e9];
      var draws = prims.map(function (p) {
        for (var i = 0; i < p.pos.length; i += 3) for (var k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], p.pos[i + k]); hi[k] = Math.max(hi[k], p.pos[i + k]); }
        var vao = gl.createVertexArray(); gl.bindVertexArray(vao);
        var b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b); gl.bufferData(gl.ARRAY_BUFFER, p.pos, gl.STATIC_DRAW);
        var lp = gl.getAttribLocation(prog, 'p'); gl.enableVertexAttribArray(lp); gl.vertexAttribPointer(lp, 3, gl.FLOAT, false, 0, 0);
        var nb = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, nb); gl.bufferData(gl.ARRAY_BUFFER, p.nor, gl.STATIC_DRAW);
        var ln = gl.getAttribLocation(prog, 'n'); gl.enableVertexAttribArray(ln); gl.vertexAttribPointer(ln, 3, gl.FLOAT, false, 0, 0);
        var ib = gl.createBuffer(); gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib); gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, p.idx, gl.STATIC_DRAW);
        return {vao: vao, count: p.idx.length, color: p.color, gloss: p.gloss};
      });
      var center = [0, 1, 2].map(function (k) { return (lo[k] + hi[k]) / 2; });
      var radius = Math.max(1e-3, Math.hypot(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]) / 2);
      var yaw = 2.2, pitch = 0.38, zoom = 1;
      function frame() {
        var w = canvas.clientWidth * devicePixelRatio, h = canvas.clientHeight * devicePixelRatio;
        if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
        gl.viewport(0, 0, w, h);
        gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
        gl.enable(gl.DEPTH_TEST);
        var dist = radius * 2.6 / zoom, cp = Math.cos(pitch);
        var eye = [center[0] + dist * Math.sin(yaw) * cp, center[1] + dist * Math.sin(pitch), center[2] + dist * Math.cos(yaw) * cp];
        var f = [center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]], fl = Math.hypot(f[0], f[1], f[2]); f = f.map(function (x) { return x / fl; });
        var r = [f[1] * 0 - f[2] * 1, f[2] * 0 - f[0] * 0, f[0] * 1 - f[1] * 0], rl = Math.hypot(r[0], r[1], r[2]) || 1; r = r.map(function (x) { return x / rl; });
        var u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
        var t = 1 / Math.tan(0.5 * 0.6), a = w / h, near = dist * 0.05, far = dist * 4;
        var view = [r[0], u[0], -f[0], 0, r[1], u[1], -f[1], 0, r[2], u[2], -f[2], 0,
          -(r[0] * eye[0] + r[1] * eye[1] + r[2] * eye[2]), -(u[0] * eye[0] + u[1] * eye[1] + u[2] * eye[2]), f[0] * eye[0] + f[1] * eye[1] + f[2] * eye[2], 1];
        var proj = [t / a, 0, 0, 0, 0, t, 0, 0, 0, 0, (far + near) / (near - far), -1, 0, 0, 2 * far * near / (near - far), 0];
        var m = new Float32Array(16);
        for (var i = 0; i < 4; i++) for (var j = 0; j < 4; j++) { var s = 0; for (var k = 0; k < 4; k++) s += proj[k * 4 + j] * view[i * 4 + k]; m[i * 4 + j] = s; }
        gl.useProgram(prog);
        gl.uniformMatrix4fv(gl.getUniformLocation(prog, 'm'), false, m);
        var lx = -f[0] + u[0] * 0.6 + r[0] * 0.3, ly = -f[1] + u[1] * 0.6 + r[1] * 0.3, lz = -f[2] + u[2] * 0.6 + r[2] * 0.3, ll = Math.hypot(lx, ly, lz);
        gl.uniform3f(gl.getUniformLocation(prog, 'l'), lx / ll, ly / ll, lz / ll);
        gl.uniform3f(gl.getUniformLocation(prog, 'e'), -f[0], -f[1], -f[2]);
        draws.forEach(function (d) { gl.uniform3f(gl.getUniformLocation(prog, 'c'), d.color[0], d.color[1], d.color[2]); gl.uniform1f(gl.getUniformLocation(prog, 'g'), d.gloss); gl.bindVertexArray(d.vao); gl.drawElements(gl.TRIANGLES, d.count, gl.UNSIGNED_INT, 0); });
      }
      canvas.hidden = false;
      var still = view.querySelector('img'); if (still) still.hidden = true;
      frame();
      var drag = null;
      canvas.addEventListener('pointerdown', function (e) { drag = [e.clientX, e.clientY]; canvas.setPointerCapture(e.pointerId); });
      canvas.addEventListener('pointerup', function () { drag = null; });
      canvas.addEventListener('pointermove', function (e) {
        if (!drag) return;
        yaw -= (e.clientX - drag[0]) * 0.01; pitch = Math.max(-1.4, Math.min(1.4, pitch + (e.clientY - drag[1]) * 0.01));
        drag = [e.clientX, e.clientY]; requestAnimationFrame(frame);
      });
      canvas.addEventListener('wheel', function (e) { e.preventDefault(); zoom = Math.max(0.3, Math.min(8, zoom * Math.exp(-e.deltaY * 0.001))); requestAnimationFrame(frame); }, {passive: false});
      new ResizeObserver(function () { requestAnimationFrame(frame); }).observe(canvas);
    });
  }
  function scan() { document.querySelectorAll('[data-cad-model]').forEach(start); }
  document.addEventListener('DOMContentLoaded', scan);
  document.addEventListener('htmx:afterSettle', scan);
  new MutationObserver(scan).observe(document.documentElement, {childList: true, subtree: true});
  scan();
})();
";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/cad/card", get(card))
        .route("/chat/{id}/cad/file", get(file))
        .route("/chat/{id}/cad/out", get(out))
        .route(
            ASSET_CSS,
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS) }),
        )
        .route(
            ASSET_JS,
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    JS,
                )
            }),
        )
}

#[derive(Deserialize, Debug)]
pub(crate) struct CardQuery {
    doc: String,
    v: u64,
    #[serde(default)]
    a: Option<u64>,
}

/// The card again, as the Lab has it (Compare, or back to the 3D view).
async fn card(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<CardQuery>,
) -> Response {
    if let Err(response) = crate::pages::chat::load(&app, &headers, &id).await {
        return response;
    }
    let Some(lab) = lab_url() else {
        return Html("<p>CAD Lab isn't running on this Mac.</p>").into_response();
    };
    let mut url = format!("{lab}/api/card?doc={}&v={}", clean(&query.doc), query.v);
    if let Some(a) = query.a {
        url.push_str(&format!("&a={a}"));
    }
    match get_json(&url, Duration::from_secs(10)).await {
        Ok(fresh) if fresh["card"] == CARD => {
            Html(card_markup(&id, &fresh).into_string()).into_response()
        }
        Ok(other) => Html(
            html! { p { (other["error"].as_str().unwrap_or("CAD Lab has no card for that.")) } }
                .into_string(),
        )
        .into_response(),
        Err(_) => Html("<p>CAD Lab isn't answering on this Mac.</p>").into_response(),
    }
}

#[derive(Deserialize, Debug)]
pub(crate) struct FileQuery {
    doc: String,
    v: u64,
    name: String,
}

async fn proxy(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    path: &str,
    query: &FileQuery,
) -> Response {
    if let Err(response) = crate::pages::chat::load(app, headers, id).await {
        return response;
    }
    let Some(lab) = lab_url() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(http) = client(Duration::from_secs(60)) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let url = format!(
        "{lab}{path}?doc={}&v={}&name={}",
        clean(&query.doc),
        query.v,
        clean(&query.name)
    );
    let Ok(upstream) = http.get(url).send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let kind = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let Ok(bytes) = upstream.bytes().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    (
        status,
        [
            (header::CONTENT_TYPE, kind),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=3600"),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// A version's 3D model or picture, from the Lab.
async fn file(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Response {
    proxy(&app, &headers, &id, "/api/file", &query).await
}

/// A render or an export of a version, from the Lab.
async fn out(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<FileQuery>,
) -> Response {
    proxy(&app, &headers, &id, "/api/out", &query).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "card": "cad_lab", "doc": "rover", "title": "Scientific rover", "version": 3,
            "summary": "Six wheels", "size_mm": [1100.0, 900.0, 640.0],
            "mass": {"mass_kg": 41.25, "center_of_mass_mm": [12.0, -3.0, 310.0]},
            "parts": 6, "instances": 14, "features": 40, "joints": 10,
            "versions": [{"n": 1}, {"n": 2}, {"n": 3}]
        })
    }

    #[test]
    fn a_card_shows_the_model_and_its_mass() {
        let html = card_markup("c1", &sample()).into_string();
        assert!(
            html.contains(
                "data-cad-model=\"/chat/c1/cad/file?doc=rover&amp;v=3&amp;name=model.glb\""
            ),
            "{html}"
        );
        assert!(
            html.contains("Mass 41.25 kg · centre of mass (12, -3, 310) mm"),
            "{html}"
        );
        assert!(html.contains("1100 × 900 × 640 mm · 6 parts, 14 placed, 40 features, 10 joints"));
        assert_eq!(
            mass_line(&json!({"mass_kg": 0.031_42, "center_of_mass_mm": [0.0, -0.04, 13.86]})),
            "Mass 31.4 g · centre of mass (0.0, 0.0, 13.9) mm"
        );
        assert!(html.contains("Compare with v2"));
        assert!(!html.contains("style=") && !html.contains("onclick"));
        assert!(
            oa_copy::violations(&html, &[]).is_empty(),
            "{:?}",
            oa_copy::violations(&html, &[])
        );
    }

    #[test]
    fn a_card_shows_a_render_exports_and_differences() {
        let mut card = sample();
        card["render"] = json!({"view": "rear", "file": "rear-1600x1000.png"});
        card["exports"] = json!([{"format": "step", "path": "/x/rover-v3.step"}]);
        card["compare"] = json!({"a": 2, "differences": ["Placed wheel_lf"]});
        let html = card_markup("c1", &card).into_string();
        assert!(
            html.contains("/chat/c1/cad/out?doc=rover&amp;v=3&amp;name=rear-1600x1000.png"),
            "{html}"
        );
        assert!(html.contains("name=rover-v3.step") && html.contains("STEP"));
        assert!(html.contains("Placed wheel_lf") && html.contains("Turn it in 3D"));
        assert!(!html.contains("Compare with v2"));
    }

    #[test]
    fn names_carry_only_safe_characters_and_the_lab_is_loopback_only() {
        assert_eq!(clean("rover\"><script>"), "roverscript");
        assert_eq!(clean("../../etc/passwd"), "....etcpasswd");
        assert_eq!(loopback("https://cad.example.com"), None);
        assert!(guidance(&[]).contains("cad_"));
    }
}
