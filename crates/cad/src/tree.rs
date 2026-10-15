//! Assembly trees in exports: STEP (AP242) and 3MF that keep the
//! assembly's structure instead of every solid flat.
//!
//! Each part is written once, in its own frame, and placed by each of its
//! instances; each placed subassembly (a group) is a product or object of
//! its own holding its instances, so the file opens in other CAD as the
//! same tree: assembly → subassemblies → parts. An instance hanging from a
//! part instance (a wheel on a bogie) is placed in the nearest group above
//! it (or the assembly), at its place relative to that group.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use serde_json::Value;
use solidcraft_engine::geom::Vec3;
use solidcraft_engine::kernel::{ExportBody, ExportProduct, StepHeader, step_export_products};

use crate::build::Built;
use crate::xf::Xf;

/// One placed instance as the tree sees it.
struct Node {
    id: String,
    part: Option<String>,
    group: Option<String>,
    /// The group (or `None`: the assembly) it is placed in.
    container: Option<String>,
    /// Its place relative to that container.
    local: Xf,
}

/// The inverse of a rigid transform.
fn inverse(x: &Xf) -> Xf {
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = x.r[j][i];
        }
    }
    let t = [0, 1, 2].map(|i| -(0..3).map(|k| r[i][k] * x.t[k]).sum::<f64>());
    Xf { r, t }
}

/// The name of the whole in exports: the assembly's title, or its id.
#[must_use]
pub fn title(built: &Built) -> String {
    built.assembly["title"]
        .as_str()
        .or(built.assembly["id"].as_str())
        .unwrap_or("assembly")
        .to_owned()
}

fn instances(built: &Built) -> &[Value] {
    built.assembly["instances"]
        .as_array()
        .map_or(&[], Vec::as_slice)
}

fn nodes(built: &Built) -> Vec<Node> {
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    let mut groups = std::collections::BTreeSet::new();
    for i in instances(built) {
        let id = i["id"].as_str().unwrap_or_default();
        if let Some(p) = i["parent"].as_str() {
            parent.insert(id, p);
        }
        if i["group"].is_string() {
            groups.insert(id);
        }
    }
    instances(built)
        .iter()
        .map(|i| {
            let id = i["id"].as_str().unwrap_or_default();
            let mut at = id;
            let mut container = None;
            for _ in 0..=parent.len() {
                match parent.get(at) {
                    Some(p) if groups.contains(p) => {
                        container = Some((*p).to_owned());
                        break;
                    }
                    Some(p) => at = p,
                    None => break,
                }
            }
            let world = built.placements.get(id).copied().unwrap_or(Xf::IDENTITY);
            let base = container
                .as_deref()
                .and_then(|c| built.placements.get(c))
                .copied()
                .unwrap_or(Xf::IDENTITY);
            Node {
                id: id.to_owned(),
                part: i["part"].as_str().map(str::to_owned),
                group: i["group"].as_str().map(str::to_owned),
                container,
                local: inverse(&base).mul(&world),
            }
        })
        .collect()
}

/// Column-major 4×4, as the kernel's STEP writer takes it.
fn columns(x: &Xf) -> [[f64; 4]; 4] {
    let mut m = [[0.0; 4]; 4];
    for (c, col) in m.iter_mut().enumerate().take(3) {
        *col = [x.r[0][c], x.r[1][c], x.r[2][c], 0.0];
    }
    m[3] = [x.t[0], x.t[1], x.t[2], 1.0];
    m
}

/// The assembly as a STEP AP242 file: a product per part (its solids in
/// its own frame), per placed subassembly and for the whole, with an
/// assembly occurrence per instance. The parts are built afresh for it
/// ([`crate::build::fresh_parts`]), so their solids are written exactly as
/// the kernel made them.
///
/// # Errors
/// A part doesn't build, or the kernel couldn't write a solid.
pub fn step(built: &Built) -> Result<String, String> {
    let name = title(built);
    let fresh = crate::build::fresh_parts(built).map_err(|p| p.join(" "))?;
    let states: BTreeMap<&str, _> = fresh
        .iter()
        .map(|(key, s)| (key.as_str(), (s.world_state(), s)))
        .collect();
    let all = nodes(built);
    let mut products: Vec<ExportProduct> = vec![ExportProduct {
        name: name.to_owned(),
        bodies: Vec::new(),
        children: Vec::new(),
    }];
    let mut part_index: BTreeMap<String, usize> = BTreeMap::new();
    for n in &all {
        if let Some(part) = &n.part
            && !part_index.contains_key(part)
        {
            part_index.insert(part.clone(), products.len());
            products.push(ExportProduct {
                name: part.clone(),
                bodies: Vec::new(),
                children: Vec::new(),
            });
        }
    }
    for (part, (state, session)) in &states {
        let Some(i) = part_index.get(*part) else {
            continue;
        };
        let part_color = built
            .solids
            .iter()
            .find(|s| s.part == *part)
            .map(|s| s.color.map(|c| f32::from(c) / 255.0));
        for b in state
            .bodies
            .iter()
            .filter(|b| session.body_visible(&b.name))
        {
            products[*i].bodies.push(ExportBody {
                name: b.name.clone(),
                body: &b.body,
                color: b.body.color().or(part_color),
            });
        }
    }
    let mut group_index: BTreeMap<String, usize> = BTreeMap::new();
    for n in &all {
        if let Some(template) = &n.group {
            group_index.insert(n.id.clone(), products.len());
            products.push(ExportProduct {
                name: format!("{template} ({})", n.id),
                bodies: Vec::new(),
                children: Vec::new(),
            });
        }
    }
    for n in &all {
        let child = match (&n.part, &n.group) {
            (Some(part), _) => part_index[part],
            (None, Some(_)) => group_index[&n.id],
            _ => continue,
        };
        let parent = n
            .container
            .as_ref()
            .and_then(|c| group_index.get(c))
            .copied()
            .unwrap_or(0);
        products[parent]
            .children
            .push((child, columns(&n.local), n.id.clone()));
    }
    let header = StepHeader {
        file_name: format!("{name}.step"),
        ..StepHeader::default()
    };
    step_export_products(&products, 0, &header).map_err(|e| e.to_string())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 3MF's row-major 3×4: `m00 m01 m02 m10 ... m32`, points as rows.
fn matrix(x: &Xf) -> String {
    let mut out = Vec::new();
    for i in 0..3 {
        for j in 0..3 {
            out.push(x.r[j][i]);
        }
    }
    out.extend(x.t);
    out.iter()
        .map(|v| format!("{}", (v * 1e9).round() / 1e9))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The assembly as a 3MF package: a mesh object per solid in its part's
/// frame, an object per part, per placed subassembly and for the whole,
/// joined by components with their transforms; one build item, the whole.
///
/// # Errors
/// Nothing to write, or the package couldn't be made.
pub fn threemf(built: &Built) -> Result<Vec<u8>, String> {
    let name = title(built);
    let name = name.as_str();
    let doc = &built.session.doc;
    let local = built.session.model.state();
    let component_part: BTreeMap<u64, &str> = doc
        .components
        .iter()
        .map(|c| (c.id, c.name.as_str()))
        .collect();
    let all = nodes(built);
    let mut colors: Vec<String> = Vec::new();
    let mut meshes = String::new();
    let mut next = 2u32;
    // Part → the mesh objects of its solids.
    let mut part_meshes: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let color_of: BTreeMap<&str, [u8; 3]> = built
        .solids
        .iter()
        .map(|s| (s.part.as_str(), s.color))
        .collect();
    for b in &local.bodies {
        let component = doc.body_component(&b.name, b.feature);
        let Some(part) = component_part.get(&component) else {
            continue;
        };
        let m = b.mesh();
        if m.triangles.is_empty() {
            continue;
        }
        let (positions, triangles) = solidcraft_engine::io::weld(&m.positions, &m.triangles, 1e-6);
        let c = b
            .body
            .color()
            .map(|c| c.map(|x| (x * 255.0).round() as u8))
            .or_else(|| color_of.get(part).copied())
            .unwrap_or([168, 174, 184]);
        colors.push(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        let id = next;
        next += 1;
        let _ = write!(
            meshes,
            "  <object id=\"{id}\" type=\"model\" name=\"{}\" pid=\"1\" pindex=\"{}\">\n   <mesh>\n    <vertices>\n",
            xml_escape(&b.name),
            colors.len() - 1
        );
        for p in &positions {
            let Vec3 { x, y, z } = *p;
            let _ = writeln!(meshes, "     <vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>");
        }
        meshes.push_str("    </vertices>\n    <triangles>\n");
        for t in &triangles {
            let _ = writeln!(
                meshes,
                "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
                t[0], t[1], t[2]
            );
        }
        meshes.push_str("    </triangles>\n   </mesh>\n  </object>\n");
        part_meshes.entry((*part).to_owned()).or_default().push(id);
    }
    if part_meshes.is_empty() {
        return Err("There's nothing to write: no solids.".into());
    }
    let mut assemblies = String::new();
    let mut object = |name: &str, children: &[(u32, String)], out: &mut String| -> u32 {
        let id = next;
        next += 1;
        let _ = writeln!(
            out,
            "  <object id=\"{id}\" type=\"model\" name=\"{}\">\n   <components>",
            xml_escape(name)
        );
        for (child, transform) in children {
            let _ = writeln!(
                out,
                "    <component objectid=\"{child}\" transform=\"{transform}\"/>"
            );
        }
        out.push_str("   </components>\n  </object>\n");
        id
    };
    let identity = matrix(&Xf::IDENTITY);
    let mut part_object: BTreeMap<String, u32> = BTreeMap::new();
    for (part, ids) in &part_meshes {
        let children: Vec<(u32, String)> = ids.iter().map(|i| (*i, identity.clone())).collect();
        part_object.insert(part.clone(), object(part, &children, &mut assemblies));
    }
    // Groups innermost first, so each refers only to objects already written.
    let depth = |n: &Node| {
        let mut d = 0;
        let mut at = n.container.clone();
        while let Some(c) = at {
            d += 1;
            at = all
                .iter()
                .find(|m| m.id == c)
                .and_then(|m| m.container.clone());
            if d > all.len() {
                break;
            }
        }
        d
    };
    let mut groups: Vec<&Node> = all.iter().filter(|n| n.group.is_some()).collect();
    groups.sort_by_key(|n| std::cmp::Reverse(depth(n)));
    let children_of =
        |container: Option<&str>, done: &BTreeMap<String, u32>| -> Vec<(u32, String)> {
            all.iter()
                .filter(|n| n.container.as_deref() == container)
                .filter_map(|n| {
                    let id = match (&n.part, &n.group) {
                        (Some(part), _) => part_object.get(part).copied(),
                        (None, Some(_)) => done.get(&n.id).copied(),
                        _ => None,
                    }?;
                    Some((id, matrix(&n.local)))
                })
                .collect()
        };
    let mut group_object: BTreeMap<String, u32> = BTreeMap::new();
    for g in groups {
        let children = children_of(Some(&g.id), &group_object);
        let label = format!("{} ({})", g.group.as_deref().unwrap_or_default(), g.id);
        let id = object(&label, &children, &mut assemblies);
        group_object.insert(g.id.clone(), id);
    }
    let top = children_of(None, &group_object);
    let root = object(name, &top, &mut assemblies);
    let mut model = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n <metadata name=\"Application\">OpenAgents CAD Lab</metadata>\n <resources>\n  <basematerials id=\"1\">\n",
    );
    for (i, c) in colors.iter().enumerate() {
        let _ = writeln!(model, "   <base name=\"colour {i}\" displaycolor=\"{c}\"/>");
    }
    model.push_str("  </basematerials>\n");
    model.push_str(&meshes);
    model.push_str(&assemblies);
    let _ = write!(
        model,
        " </resources>\n <build>\n  <item objectid=\"{root}\"/>\n </build>\n</model>\n"
    );
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n <Default Extension=\"model\" ContentType=\"application/vnd.ms-package.3dmanufacturing-3dmodel+xml\"/>\n</Types>\n";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n <Relationship Target=\"/3D/3dmodel.model\" Id=\"rel0\" Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/>\n</Relationships>\n";
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (file, data) in [
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("3D/3dmodel.model", model.as_bytes()),
    ] {
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default());
        zip.start_file(file, options)
            .map_err(|e| format!("3MF: {e}"))?;
        zip.write_all(data).map_err(|e| format!("3MF: {e}"))?;
    }
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|e| format!("3MF: {e}"))
}
