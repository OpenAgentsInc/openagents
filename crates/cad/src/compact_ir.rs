use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;

use crate::mcp_tools::{CadMcpDocument, CadMcpMaterial, CadMcpNode, CadMcpNodeOp, CadMcpRoot};
use crate::{CadError, CadResult};

pub const CAD_COMPACT_IR_VERSION: &str = "0.2";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompactParseError {
    line: usize,
    message: String,
}

impl fmt::Display for CompactParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for CompactParseError {}

pub fn to_compact(document: &CadMcpDocument) -> CadResult<String> {
    let nodes = index_nodes(document)?;

    let mut out = format!("# vcad {}\n", CAD_COMPACT_IR_VERSION);

    let mut material_names = BTreeSet::new();
    for name in document.materials.keys() {
        material_names.insert(name.clone());
    }
    for root in &document.roots {
        let material = root.material.trim();
        if !material.is_empty() {
            material_names.insert(material.to_string());
        }
    }
    if material_names.is_empty() && !nodes.is_empty() {
        material_names.insert("default".to_string());
    }

    if !material_names.is_empty() {
        out.push_str("\n# Materials\n");
        for name in material_names {
            let density = document
                .materials
                .get(&name)
                .and_then(|material| material.density)
                .filter(|value| value.is_finite());

            out.push_str("M ");
            out.push_str(&quote_if_needed(&name));
            out.push_str(" 0.8 0.8 0.8 0 0.5");
            if let Some(value) = density {
                out.push(' ');
                out.push_str(&format_number(value));
            }
            out.push('\n');
        }
    }

    if !nodes.is_empty() {
        let mut root_ids = Vec::new();
        if document.roots.is_empty() {
            if let Some(last) = nodes.keys().next_back().copied() {
                root_ids.push(last);
            }
        } else {
            root_ids.extend(document.roots.iter().map(|root| root.root));
        }

        let ordered = topological_order(&nodes, &root_ids)?;
        let id_map = ordered
            .iter()
            .enumerate()
            .map(|(index, id)| (*id, index as u64))
            .collect::<HashMap<_, _>>();

        out.push_str("\n# Geometry\n");
        for node_id in &ordered {
            let node = nodes
                .get(node_id)
                .ok_or_else(|| CadError::InvalidFeatureGraph {
                    reason: format!("compact ir serialization missing node {node_id}"),
                })?;
            let line = format_geometry(node, &id_map)?;
            out.push_str(&line);
            out.push('\n');
        }

        let roots = if document.roots.is_empty() {
            let material = document
                .materials
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            vec![CadMcpRoot {
                root: *ordered.last().ok_or_else(|| CadError::ParseFailed {
                    reason: "compact ir serialization expected at least one node".to_string(),
                })?,
                material,
            }]
        } else {
            document.roots.clone()
        };

        out.push_str("\n# Scene\n");
        for root in roots {
            let mapped = id_map
                .get(&root.root)
                .copied()
                .ok_or_else(|| CadError::ParseFailed {
                    reason: format!(
                        "compact ir serialization root {} does not exist in node map",
                        root.root
                    ),
                })?;
            out.push_str(&format!(
                "ROOT {} {}\n",
                mapped,
                quote_if_needed(root.material.trim())
            ));
        }
    }

    while out.ends_with('\n') {
        out.pop();
    }

    Ok(out)
}

pub fn from_compact(input: &str) -> CadResult<CadMcpDocument> {
    let mut nodes = BTreeMap::<String, CadMcpNode>::new();
    let mut materials = BTreeMap::<String, CadMcpMaterial>::new();
    let mut pending_roots = Vec::<(usize, u64, String)>::new();
    let mut next_node_id = 0u64;

    for (line_index, raw_line) in input.lines().enumerate() {
        let line = strip_comment_preserving_quotes(raw_line).trim().to_string();
        if line.is_empty() {
            continue;
        }

        let tokens = tokenize(&line, line_index).map_err(map_parse_error)?;
        if tokens.is_empty() {
            continue;
        }

        match tokens[0].as_str() {
            "M" => parse_material(&tokens, line_index, &mut materials).map_err(map_parse_error)?,
            "ROOT" => {
                if tokens.len() < 3 {
                    return Err(map_parse_error(CompactParseError {
                        line: line_index,
                        message: format!("ROOT requires at least 2 args, got {}", tokens.len() - 1),
                    }));
                }
                let root = parse_u64(&tokens[1], line_index).map_err(map_parse_error)?;
                let material = tokens[2].clone();
                pending_roots.push((line_index, root, material));
            }
            "C" | "Y" | "S" | "K" | "U" | "D" | "I" | "T" | "R" | "X" => {
                let (op, name) = parse_geometry_opcode(&tokens, line_index, next_node_id)
                    .map_err(map_parse_error)?;
                let node = CadMcpNode {
                    id: next_node_id,
                    name,
                    op,
                };
                let _ = nodes.insert(next_node_id.to_string(), node);
                next_node_id = next_node_id.saturating_add(1);
            }
            opcode => {
                return Err(map_parse_error(CompactParseError {
                    line: line_index,
                    message: format!("unknown opcode: {opcode}"),
                }));
            }
        }
    }

    let mut roots = Vec::<CadMcpRoot>::new();
    for (line, root, material) in pending_roots {
        if root >= next_node_id {
            return Err(map_parse_error(CompactParseError {
                line,
                message: format!("node {root} referenced but only {next_node_id} nodes defined"),
            }));
        }
        if !materials.contains_key(&material) {
            let _ = materials.insert(material.clone(), CadMcpMaterial { density: None });
        }
        roots.push(CadMcpRoot { root, material });
    }

    if !nodes.is_empty() && roots.is_empty() {
        if materials.is_empty() {
            let _ = materials.insert("default".to_string(), CadMcpMaterial { density: None });
        }
        let material = materials
            .keys()
            .next()
            .cloned()
            .unwrap_or_else(|| "default".to_string());
        roots.push(CadMcpRoot {
            root: next_node_id.saturating_sub(1),
            material,
        });
    }

    let mut part_materials = BTreeMap::<String, String>::new();
    for root in &roots {
        let base_name = nodes
            .get(&root.root.to_string())
            .and_then(|node| node.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| format!("part_{}", root.root));

        let mut candidate = base_name.clone();
        let mut suffix = 2u64;
        while part_materials.contains_key(&candidate) {
            candidate = format!("{base_name}_{suffix}");
            suffix = suffix.saturating_add(1);
        }

        let _ = part_materials.insert(candidate, root.material.clone());
    }

    Ok(CadMcpDocument {
        version: "0.1".to_string(),
        nodes,
        materials,
        roots,
        part_materials,
    })
}

pub fn looks_like_compact_ir(payload: &str) -> bool {
    let trimmed = payload.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return false;
    }

    for raw_line in payload.lines() {
        let stripped = strip_comment_preserving_quotes(raw_line);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }
        let opcode = line.split_whitespace().next().unwrap_or_default();
        return matches!(
            opcode,
            "M" | "ROOT" | "C" | "Y" | "S" | "K" | "U" | "D" | "I" | "T" | "R" | "X"
        );
    }

    false
}

fn map_parse_error(error: CompactParseError) -> CadError {
    CadError::ParseFailed {
        reason: format!("compact ir {}", error),
    }
}

fn parse_material(
    tokens: &[String],
    line: usize,
    materials: &mut BTreeMap<String, CadMcpMaterial>,
) -> Result<(), CompactParseError> {
    if tokens.len() == 3 {
        let name = tokens[1].clone();
        let density = parse_f64(&tokens[2], line)?;
        let _ = materials.insert(
            name,
            CadMcpMaterial {
                density: Some(density),
            },
        );
        return Ok(());
    }

    if tokens.len() < 7 {
        return Err(CompactParseError {
            line,
            message: format!(
                "M requires at least 6 args (name rgb metallic roughness), got {}",
                tokens.len() - 1
            ),
        });
    }

    let name = tokens[1].clone();
    let _ = parse_f64(&tokens[2], line)?;
    let _ = parse_f64(&tokens[3], line)?;
    let _ = parse_f64(&tokens[4], line)?;
    let _ = parse_f64(&tokens[5], line)?;
    let _ = parse_f64(&tokens[6], line)?;

    let density = if tokens.len() >= 8 {
        Some(parse_f64(&tokens[7], line)?)
    } else {
        None
    };

    let _ = materials.insert(name, CadMcpMaterial { density });
    Ok(())
}

fn parse_geometry_opcode(
    tokens: &[String],
    line: usize,
    next_node_id: u64,
) -> Result<(CadMcpNodeOp, Option<String>), CompactParseError> {
    match tokens[0].as_str() {
        "C" => {
            let name = parse_optional_name(tokens, 3, line)?;
            Ok((
                CadMcpNodeOp::Cube {
                    size: crate::mcp_tools::CadMcpVec3 {
                        x: parse_f64(&tokens[1], line)?,
                        y: parse_f64(&tokens[2], line)?,
                        z: parse_f64(&tokens[3], line)?,
                    },
                },
                name,
            ))
        }
        "Y" => {
            let name = parse_optional_name(tokens, 2, line)?;
            Ok((
                CadMcpNodeOp::Cylinder {
                    radius: parse_f64(&tokens[1], line)?,
                    height: parse_f64(&tokens[2], line)?,
                    segments: 32,
                },
                name,
            ))
        }
        "S" => {
            let name = parse_optional_name(tokens, 1, line)?;
            Ok((
                CadMcpNodeOp::Sphere {
                    radius: parse_f64(&tokens[1], line)?,
                    segments: 32,
                },
                name,
            ))
        }
        "K" => {
            let name = parse_optional_name(tokens, 3, line)?;
            Ok((
                CadMcpNodeOp::Cone {
                    radius_bottom: parse_f64(&tokens[1], line)?,
                    radius_top: parse_f64(&tokens[2], line)?,
                    height: parse_f64(&tokens[3], line)?,
                    segments: 32,
                },
                name,
            ))
        }
        "U" => {
            let name = parse_optional_name(tokens, 2, line)?;
            Ok((
                CadMcpNodeOp::Union {
                    left: parse_node_ref(&tokens[1], line, next_node_id)?,
                    right: parse_node_ref(&tokens[2], line, next_node_id)?,
                },
                name,
            ))
        }
        "D" => {
            let name = parse_optional_name(tokens, 2, line)?;
            Ok((
                CadMcpNodeOp::Difference {
                    left: parse_node_ref(&tokens[1], line, next_node_id)?,
                    right: parse_node_ref(&tokens[2], line, next_node_id)?,
                },
                name,
            ))
        }
        "I" => {
            let name = parse_optional_name(tokens, 2, line)?;
            Ok((
                CadMcpNodeOp::Intersection {
                    left: parse_node_ref(&tokens[1], line, next_node_id)?,
                    right: parse_node_ref(&tokens[2], line, next_node_id)?,
                },
                name,
            ))
        }
        "T" => {
            let name = parse_optional_name(tokens, 4, line)?;
            Ok((
                CadMcpNodeOp::Translate {
                    child: parse_node_ref(&tokens[1], line, next_node_id)?,
                    offset: crate::mcp_tools::CadMcpVec3 {
                        x: parse_f64(&tokens[2], line)?,
                        y: parse_f64(&tokens[3], line)?,
                        z: parse_f64(&tokens[4], line)?,
                    },
                },
                name,
            ))
        }
        "R" => {
            let name = parse_optional_name(tokens, 4, line)?;
            Ok((
                CadMcpNodeOp::Rotate {
                    child: parse_node_ref(&tokens[1], line, next_node_id)?,
                    angles: crate::mcp_tools::CadMcpVec3 {
                        x: parse_f64(&tokens[2], line)?,
                        y: parse_f64(&tokens[3], line)?,
                        z: parse_f64(&tokens[4], line)?,
                    },
                },
                name,
            ))
        }
        "X" => {
            let name = parse_optional_name(tokens, 4, line)?;
            Ok((
                CadMcpNodeOp::Scale {
                    child: parse_node_ref(&tokens[1], line, next_node_id)?,
                    factor: crate::mcp_tools::CadMcpVec3 {
                        x: parse_f64(&tokens[2], line)?,
                        y: parse_f64(&tokens[3], line)?,
                        z: parse_f64(&tokens[4], line)?,
                    },
                },
                name,
            ))
        }
        opcode => Err(CompactParseError {
            line,
            message: format!("unknown opcode: {opcode}"),
        }),
    }
}

fn parse_optional_name(
    tokens: &[String],
    arg_count: usize,
    line: usize,
) -> Result<Option<String>, CompactParseError> {
    let required = arg_count + 1;
    if tokens.len() == required {
        return Ok(None);
    }
    if tokens.len() == required + 1 {
        return Ok(Some(tokens[required].clone()));
    }
    Err(CompactParseError {
        line,
        message: format!(
            "{} expects {} args, got {}",
            tokens[0],
            arg_count,
            tokens.len().saturating_sub(1)
        ),
    })
}

fn parse_node_ref(token: &str, line: usize, next_node_id: u64) -> Result<u64, CompactParseError> {
    let node = parse_u64(token, line)?;
    if node >= next_node_id {
        return Err(CompactParseError {
            line,
            message: format!("node {node} referenced but only {next_node_id} nodes defined"),
        });
    }
    Ok(node)
}

fn parse_f64(token: &str, line: usize) -> Result<f64, CompactParseError> {
    let value = token.parse::<f64>().map_err(|_| CompactParseError {
        line,
        message: format!("invalid number: {token}"),
    })?;
    if !value.is_finite() {
        return Err(CompactParseError {
            line,
            message: format!("invalid number: {token}"),
        });
    }
    Ok(value)
}

fn parse_u64(token: &str, line: usize) -> Result<u64, CompactParseError> {
    token.parse::<u64>().map_err(|_| CompactParseError {
        line,
        message: format!("invalid node id: {token}"),
    })
}

fn tokenize(line: &str, line_number: usize) -> Result<Vec<String>, CompactParseError> {
    let mut tokens = Vec::<String>::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in line.chars() {
        if in_quotes {
            if escape {
                current.push(ch);
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_quotes = false;
                continue;
            }
            current.push(ch);
            continue;
        }

        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        if ch == '"' {
            in_quotes = true;
            continue;
        }

        current.push(ch);
    }

    if in_quotes {
        return Err(CompactParseError {
            line: line_number,
            message: "unterminated quoted string".to_string(),
        });
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn strip_comment_preserving_quotes(line: &str) -> String {
    let mut out = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in line.chars() {
        if in_quotes {
            out.push(ch);
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_quotes = false;
            }
            continue;
        }

        if ch == '"' {
            in_quotes = true;
            out.push(ch);
            continue;
        }

        if ch == '#' {
            break;
        }

        out.push(ch);
    }

    out
}

fn index_nodes(document: &CadMcpDocument) -> CadResult<BTreeMap<u64, &CadMcpNode>> {
    let mut indexed = BTreeMap::<u64, &CadMcpNode>::new();
    for (node_id, node) in &document.nodes {
        let parsed = node_id
            .parse::<u64>()
            .map_err(|error| CadError::ParseFailed {
                reason: format!(
                    "compact ir serialization found non-numeric node id {node_id}: {error}"
                ),
            })?;

        if parsed != node.id {
            return Err(CadError::ParseFailed {
                reason: format!(
                    "compact ir serialization node id mismatch key={node_id} value={}",
                    node.id
                ),
            });
        }

        if indexed.insert(parsed, node).is_some() {
            return Err(CadError::ParseFailed {
                reason: format!("compact ir serialization duplicated node id {parsed}"),
            });
        }
    }

    Ok(indexed)
}

fn topological_order(nodes: &BTreeMap<u64, &CadMcpNode>, roots: &[u64]) -> CadResult<Vec<u64>> {
    fn visit(
        node_id: u64,
        nodes: &BTreeMap<u64, &CadMcpNode>,
        visited: &mut HashSet<u64>,
        visiting: &mut HashSet<u64>,
        ordered: &mut Vec<u64>,
    ) -> CadResult<()> {
        if visited.contains(&node_id) {
            return Ok(());
        }
        if !visiting.insert(node_id) {
            return Err(CadError::InvalidFeatureGraph {
                reason: format!("compact ir serialization detected cycle at node {node_id}"),
            });
        }

        let node = nodes.get(&node_id).ok_or_else(|| CadError::ParseFailed {
            reason: format!("compact ir serialization missing node {node_id}"),
        })?;

        for child in child_ids(&node.op) {
            visit(child, nodes, visited, visiting, ordered)?;
        }

        let _ = visiting.remove(&node_id);
        let _ = visited.insert(node_id);
        ordered.push(node_id);
        Ok(())
    }

    let mut ordered = Vec::<u64>::new();
    let mut visited = HashSet::<u64>::new();
    let mut visiting = HashSet::<u64>::new();

    if roots.is_empty() {
        for node_id in nodes.keys().copied() {
            visit(node_id, nodes, &mut visited, &mut visiting, &mut ordered)?;
        }
        return Ok(ordered);
    }

    for root in roots {
        visit(*root, nodes, &mut visited, &mut visiting, &mut ordered)?;
    }

    for node_id in nodes.keys().copied() {
        if !visited.contains(&node_id) {
            visit(node_id, nodes, &mut visited, &mut visiting, &mut ordered)?;
        }
    }

    Ok(ordered)
}

fn child_ids(op: &CadMcpNodeOp) -> Vec<u64> {
    match op {
        CadMcpNodeOp::Union { left, right }
        | CadMcpNodeOp::Difference { left, right }
        | CadMcpNodeOp::Intersection { left, right } => vec![*left, *right],
        CadMcpNodeOp::Translate { child, .. }
        | CadMcpNodeOp::Rotate { child, .. }
        | CadMcpNodeOp::Scale { child, .. } => vec![*child],
        CadMcpNodeOp::Cube { .. }
        | CadMcpNodeOp::Cylinder { .. }
        | CadMcpNodeOp::Sphere { .. }
        | CadMcpNodeOp::Cone { .. } => Vec::new(),
    }
}

fn format_geometry(node: &CadMcpNode, id_map: &HashMap<u64, u64>) -> CadResult<String> {
    let name = node
        .name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!(" {}", quote_always(value)))
        .unwrap_or_default();

    let line = match &node.op {
        CadMcpNodeOp::Cube { size } => format!(
            "C {} {} {}{}",
            format_number(size.x),
            format_number(size.y),
            format_number(size.z),
            name
        ),
        CadMcpNodeOp::Cylinder { radius, height, .. } => format!(
            "Y {} {}{}",
            format_number(*radius),
            format_number(*height),
            name
        ),
        CadMcpNodeOp::Sphere { radius, .. } => {
            format!("S {}{}", format_number(*radius), name)
        }
        CadMcpNodeOp::Cone {
            radius_bottom,
            radius_top,
            height,
            ..
        } => format!(
            "K {} {} {}{}",
            format_number(*radius_bottom),
            format_number(*radius_top),
            format_number(*height),
            name
        ),
        CadMcpNodeOp::Union { left, right } => format!(
            "U {} {}{}",
            lookup_mapped(*left, id_map)?,
            lookup_mapped(*right, id_map)?,
            name
        ),
        CadMcpNodeOp::Difference { left, right } => format!(
            "D {} {}{}",
            lookup_mapped(*left, id_map)?,
            lookup_mapped(*right, id_map)?,
            name
        ),
        CadMcpNodeOp::Intersection { left, right } => format!(
            "I {} {}{}",
            lookup_mapped(*left, id_map)?,
            lookup_mapped(*right, id_map)?,
            name
        ),
        CadMcpNodeOp::Translate { child, offset } => format!(
            "T {} {} {} {}{}",
            lookup_mapped(*child, id_map)?,
            format_number(offset.x),
            format_number(offset.y),
            format_number(offset.z),
            name
        ),
        CadMcpNodeOp::Rotate { child, angles } => format!(
            "R {} {} {} {}{}",
            lookup_mapped(*child, id_map)?,
            format_number(angles.x),
            format_number(angles.y),
            format_number(angles.z),
            name
        ),
        CadMcpNodeOp::Scale { child, factor } => format!(
            "X {} {} {} {}{}",
            lookup_mapped(*child, id_map)?,
            format_number(factor.x),
            format_number(factor.y),
            format_number(factor.z),
            name
        ),
    };

    Ok(line)
}

fn lookup_mapped(node_id: u64, id_map: &HashMap<u64, u64>) -> CadResult<u64> {
    id_map
        .get(&node_id)
        .copied()
        .ok_or_else(|| CadError::ParseFailed {
            reason: format!("compact ir serialization missing mapped node {node_id}"),
        })
}

fn quote_if_needed(value: &str) -> String {
    if value.is_empty()
        || value.contains(char::is_whitespace)
        || value.contains('"')
        || value.contains('#')
    {
        return quote_always(value);
    }
    value.to_string()
}

fn quote_always(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn format_number(value: f64) -> String {
    let mut out = format!("{value}");
    if out.contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    if out == "-0" {
        return "0".to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{CAD_COMPACT_IR_VERSION, from_compact, looks_like_compact_ir, to_compact};
    use crate::mcp_tools::{
        CadMcpDocument, CadMcpMaterial, CadMcpNode, CadMcpNodeOp, CadMcpRoot, CadMcpVec3,
    };
    use std::collections::BTreeMap;

    fn sample_document() -> CadMcpDocument {
        let nodes = BTreeMap::from([
            (
                "10".to_string(),
                CadMcpNode {
                    id: 10,
                    name: Some("Base Plate".to_string()),
                    op: CadMcpNodeOp::Cube {
                        size: CadMcpVec3 {
                            x: 50.0,
                            y: 30.0,
                            z: 5.0,
                        },
                    },
                },
            ),
            (
                "11".to_string(),
                CadMcpNode {
                    id: 11,
                    name: Some("Hole".to_string()),
                    op: CadMcpNodeOp::Cylinder {
                        radius: 5.0,
                        height: 10.0,
                        segments: 32,
                    },
                },
            ),
            (
                "12".to_string(),
                CadMcpNode {
                    id: 12,
                    name: None,
                    op: CadMcpNodeOp::Translate {
                        child: 11,
                        offset: CadMcpVec3 {
                            x: 25.0,
                            y: 15.0,
                            z: 0.0,
                        },
                    },
                },
            ),
            (
                "13".to_string(),
                CadMcpNode {
                    id: 13,
                    name: Some("Plate With Hole".to_string()),
                    op: CadMcpNodeOp::Difference {
                        left: 10,
                        right: 12,
                    },
                },
            ),
        ]);

        CadMcpDocument {
            version: "0.1".to_string(),
            nodes,
            materials: BTreeMap::from([
                ("default".to_string(), CadMcpMaterial { density: None }),
                (
                    "aluminum".to_string(),
                    CadMcpMaterial {
                        density: Some(2700.0),
                    },
                ),
            ]),
            roots: vec![CadMcpRoot {
                root: 13,
                material: "aluminum".to_string(),
            }],
            part_materials: BTreeMap::from([("plate".to_string(), "aluminum".to_string())]),
        }
    }

    #[test]
    fn to_compact_writes_vcad_header_and_geometry() {
        let compact = to_compact(&sample_document()).expect("serialize compact ir");
        assert!(compact.contains(&format!("# vcad {CAD_COMPACT_IR_VERSION}")));
        assert!(compact.contains("# Materials"));
        assert!(compact.contains("M aluminum 0.8 0.8 0.8 0 0.5 2700"));
        assert!(compact.contains("C 50 30 5 \"Base Plate\""));
        assert!(compact.contains("Y 5 10 \"Hole\""));
        assert!(compact.contains("T 1 25 15 0"));
        assert!(compact.contains("D 0 2 \"Plate With Hole\""));
        assert!(compact.contains("ROOT 3 aluminum"));
    }

    #[test]
    fn from_compact_parses_geometry_and_defaults() {
        let compact = "C 50 30 5\nY 5 10\nT 1 25 15 0\nD 0 2";
        let document = from_compact(compact).expect("parse compact");
        assert_eq!(document.nodes.len(), 4);
        assert_eq!(document.roots.len(), 1);
        assert_eq!(document.roots[0].root, 3);
        assert!(document.materials.contains_key("default"));

        match &document.nodes["2"].op {
            CadMcpNodeOp::Translate { child, offset } => {
                assert_eq!(*child, 1);
                assert_eq!(offset.x, 25.0);
                assert_eq!(offset.y, 15.0);
                assert_eq!(offset.z, 0.0);
            }
            other => panic!("expected translate, got {other:?}"),
        }
    }

    #[test]
    fn from_compact_rejects_invalid_reference() {
        let compact = "C 10 10 10\nD 0 2";
        let error = from_compact(compact).expect_err("invalid reference should fail");
        assert!(error.to_string().contains("nodes defined"));
    }

    #[test]
    fn from_compact_roundtrip_is_deterministic() {
        let first = to_compact(&sample_document()).expect("serialize first");
        let first_doc = from_compact(&first).expect("parse first");
        let second = to_compact(&first_doc).expect("serialize second");
        let second_doc = from_compact(&second).expect("parse second");
        assert_eq!(first_doc, second_doc);
    }

    #[test]
    fn looks_like_compact_ir_detects_expected_inputs() {
        assert!(looks_like_compact_ir("C 10 10 10"));
        assert!(looks_like_compact_ir("M default 0.8 0.8 0.8 0 0.5"));
        assert!(!looks_like_compact_ir("{\"version\":\"0.1\"}"));
        assert!(!looks_like_compact_ir("ISO-10303-21;"));
    }
}
