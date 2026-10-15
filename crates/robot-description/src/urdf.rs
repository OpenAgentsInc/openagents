//! The URDF reader and its checks.

use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use glam::{DMat3, DMat4, DVec3};
use roxmltree::{Document, Node, ParsingOptions};

use crate::{
    AXIS_TOLERANCE, Error, Geometry, Inertial, Joint, JointKind, Limit, Link,
    MAX_DESCRIPTION_BYTES, MAX_JOINTS, MAX_LINKS, MAX_MESH_BYTES, MAX_MESH_EXTENT_METERS,
    MAX_MESHES, MAX_OFFSET_METERS, MAX_TOTAL_MESH_BYTES, MAX_XML_NODES, Robot, Shape, TriangleMesh,
    origin,
};

/// Reads the URDF file at `path`; its meshes resolve under its folder.
pub fn load(path: &Path) -> Result<Robot, Error> {
    let shown = path.display().to_string();
    let read_error = |e: std::io::Error| Error::Read {
        path: shown.clone(),
        reason: e.to_string(),
    };
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("urdf") => {}
        Some("xacro") => {
            return Err(Error::Unsupported {
                feature: "Xacro (expand it offline into a plain .urdf first)".into(),
                at: shown,
            });
        }
        _ => return Err(Error::NotUrdf(format!("{shown} is not a .urdf file"))),
    }
    let size = std::fs::metadata(path).map_err(read_error)?.len();
    if size > MAX_DESCRIPTION_BYTES {
        return Err(Error::TooLarge {
            what: format!("{shown} in bytes"),
            limit: MAX_DESCRIPTION_BYTES,
            actual: size,
        });
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(read_error)?
        .take(MAX_DESCRIPTION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_DESCRIPTION_BYTES {
        return Err(Error::TooLarge {
            what: format!("{shown} in bytes"),
            limit: MAX_DESCRIPTION_BYTES,
            actual: bytes.len() as u64,
        });
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::NotUrdf(format!("{shown} is not UTF-8 text")))?;
    let folder = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    parse(&text, folder)
}

/// Reads URDF `text` whose meshes resolve under the folder `assets`.
pub fn parse(text: &str, assets: &Path) -> Result<Robot, Error> {
    parse_impl(text, Meshes::new(assets))
}

/// Read checked description data with an injected, bounded mesh reader. This
/// supports browser/private-asset adapters without filesystem access.
pub fn parse_with_reader(
    text: &str,
    reader: &dyn Fn(&str) -> Result<Vec<u8>, Error>,
) -> Result<Robot, Error> {
    parse_impl(
        text,
        Meshes {
            folder: Path::new(""),
            canonical: None,
            loaded: HashMap::new(),
            bytes: 0,
            reader: Some(reader),
        },
    )
}

fn parse_impl(text: &str, mut meshes: Meshes<'_>) -> Result<Robot, Error> {
    if text.len() as u64 > MAX_DESCRIPTION_BYTES {
        return Err(Error::TooLarge {
            what: "the description in bytes".into(),
            limit: MAX_DESCRIPTION_BYTES,
            actual: text.len() as u64,
        });
    }
    if text.contains("xacro:") {
        return Err(Error::Unsupported {
            feature: "Xacro (expand it offline into a plain .urdf first)".into(),
            at: "robot".into(),
        });
    }
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
            ..ParsingOptions::default()
        },
    )
    .map_err(|e| Error::Xml(e.to_string()))?;
    let robot = doc.root_element();
    if robot.tag_name().name() != "robot" || robot.tag_name().namespace().is_some() {
        return Err(Error::NotUrdf(format!(
            "the top element is <{}>, not <robot>",
            robot.tag_name().name()
        )));
    }
    if let Some(node) = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().namespace().is_some())
    {
        return Err(Error::Unsupported {
            feature: format!(
                "the extension element <{}> from {}",
                node.tag_name().name(),
                node.tag_name().namespace().unwrap_or_default()
            ),
            at: "robot".into(),
        });
    }
    let name = required(robot, "name", "robot")?.to_string();

    let mut materials = HashMap::new();
    for node in elements(robot).filter(|n| n.has_tag_name("material")) {
        let at = "robot material";
        let key = required(node, "name", at)?;
        materials.insert(key.to_string(), color(node, at)?);
    }

    let mut links = Vec::new();
    let mut joints = Vec::new();
    for node in elements(robot) {
        match node.tag_name().name() {
            "link" => {
                if links.len() == MAX_LINKS {
                    return Err(too_many("links", MAX_LINKS, robot, "link"));
                }
                links.push(link(node, &materials, &mut meshes)?);
            }
            "joint" => {
                if joints.len() == MAX_JOINTS {
                    return Err(too_many("joints", MAX_JOINTS, robot, "joint"));
                }
                joints.push(joint(node)?);
            }
            "material" => {}
            "transmission" => {
                // Transmissions describe motors, not link geometry or
                // kinematics. This importer only inspects poses; it does not
                // claim to import actuator behavior.
            }
            "gazebo" => {
                return Err(Error::Unsupported {
                    feature: "<gazebo> (simulator settings)".into(),
                    at: format!("robot {name}"),
                });
            }
            other => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}>"),
                    at: format!("robot {name}"),
                });
            }
        }
    }
    tree(name, links, joints)
}

fn too_many(what: &str, limit: usize, robot: Node, tag: &str) -> Error {
    Error::TooLarge {
        what: format!("the number of {what}"),
        limit: limit as u64,
        actual: elements(robot).filter(|n| n.has_tag_name(tag)).count() as u64,
    }
}

/// A joint as written, before its links are looked up.
struct RawJoint {
    name: String,
    kind: JointKind,
    parent: String,
    child: String,
    origin: DMat4,
    axis: DVec3,
    limit: Option<Limit>,
}

fn tree(name: String, links: Vec<Link>, raw: Vec<RawJoint>) -> Result<Robot, Error> {
    if links.is_empty() {
        return Err(Error::Structure("the robot has no links".into()));
    }
    let mut index = HashMap::new();
    for (i, link) in links.iter().enumerate() {
        if index.insert(link.name.as_str(), i).is_some() {
            return Err(Error::Structure(format!(
                "two links are named {}",
                link.name
            )));
        }
    }
    let mut names = HashSet::new();
    let mut parent_of: Vec<Option<usize>> = vec![None; links.len()];
    let mut joints = Vec::with_capacity(raw.len());
    for (j, joint) in raw.into_iter().enumerate() {
        if !names.insert(joint.name.clone()) {
            return Err(Error::Structure(format!(
                "two joints are named {}",
                joint.name
            )));
        }
        let find = |role: &str, link: &str| {
            index.get(link).copied().ok_or_else(|| {
                Error::Structure(format!(
                    "joint {} names {role} link {link}, which the description does not define",
                    joint.name
                ))
            })
        };
        let parent = find("parent", &joint.parent)?;
        let child = find("child", &joint.child)?;
        if parent == child {
            return Err(Error::Structure(format!(
                "joint {} joins link {} to itself",
                joint.name, joint.parent
            )));
        }
        if let Some(other) = parent_of[child] {
            let other: &Joint = &joints[other];
            return Err(Error::Unsupported {
                feature: format!(
                    "a closed kinematic chain (link {} is the child of joints {} and {})",
                    joint.child, other.name, joint.name
                ),
                at: format!("joint {}", joint.name),
            });
        }
        parent_of[child] = Some(j);
        joints.push(Joint {
            name: joint.name,
            kind: joint.kind,
            parent,
            child,
            origin: joint.origin,
            axis: joint.axis,
            limit: joint.limit,
        });
    }
    let roots: Vec<usize> = (0..links.len())
        .filter(|&i| parent_of[i].is_none())
        .collect();
    let root = match roots.as_slice() {
        [] => {
            return Err(Error::Structure(
                "every link is the child of a joint, so the joints form a loop".into(),
            ));
        }
        [root] => *root,
        many => {
            let names: Vec<&str> = many.iter().map(|&i| links[i].name.as_str()).collect();
            return Err(Error::Structure(format!(
                "links {} have no parent joint; a robot is one tree with one base link",
                names.join(", ")
            )));
        }
    };
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); links.len()];
    for (j, joint) in joints.iter().enumerate() {
        children[joint.parent].push(j);
    }
    let mut order = Vec::with_capacity(joints.len());
    let mut reached = vec![false; links.len()];
    reached[root] = true;
    let mut queue = VecDeque::from([root]);
    while let Some(link) = queue.pop_front() {
        for &j in &children[link] {
            let child = joints[j].child;
            if !reached[child] {
                reached[child] = true;
                order.push(j);
                queue.push_back(child);
            }
        }
    }
    let unreached: Vec<&str> = (0..links.len())
        .filter(|&i| !reached[i])
        .map(|i| links[i].name.as_str())
        .collect();
    if !unreached.is_empty() {
        return Err(Error::Structure(format!(
            "links {} are not connected to base link {}; their joints form a loop",
            unreached.join(", "),
            links[root].name
        )));
    }
    let movable = (0..joints.len())
        .filter(|&j| joints[j].kind != JointKind::Fixed)
        .collect();
    Ok(Robot {
        name,
        links,
        joints,
        root,
        order,
        movable,
    })
}

fn link(
    node: Node,
    materials: &HashMap<String, Option<[f32; 4]>>,
    meshes: &mut Meshes,
) -> Result<Link, Error> {
    let name = required(node, "name", "link")?.to_string();
    let at = format!("link {name}");
    let mut visuals = Vec::new();
    let mut collisions = Vec::new();
    let mut inertial_of = None;
    for child in elements(node) {
        match child.tag_name().name() {
            "origin" => {
                // Some generated URDFs include this nonstandard identity
                // origin on a dummy link. It has no effect on the link frame.
                let origin = pose_origin(child, &at)?;
                if !origin
                    .to_cols_array()
                    .iter()
                    .zip(DMat4::IDENTITY.to_cols_array())
                    .all(|(actual, expected)| (actual - expected).abs() <= 1e-12)
                {
                    return Err(Error::Unsupported {
                        feature: "a non-identity <origin> directly in a link".into(),
                        at,
                    });
                }
            }
            "inertial" => inertial_of = Some(inertial(child, &at)?),
            "visual" => {
                let at = format!("{at} visual {}", visuals.len() + 1);
                visuals.push(shape(child, &at, Some(materials), meshes)?);
            }
            "collision" => {
                let at = format!("{at} collision {}", collisions.len() + 1);
                collisions.push(shape(child, &at, None, meshes)?);
            }
            other => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}> in a link"),
                    at,
                });
            }
        }
    }
    Ok(Link {
        name,
        inertial: inertial_of,
        visuals,
        collisions,
    })
}

fn inertial(node: Node, at: &str) -> Result<Inertial, Error> {
    let mut out = Inertial {
        mass: 0.0,
        origin: DMat4::IDENTITY,
        inertia: DMat3::ZERO,
    };
    for child in elements(node) {
        match child.tag_name().name() {
            "origin" => {
                out.origin = pose_origin(child, at)?;
            }
            "mass" => {
                let [mass] = numbers(required(child, "value", at)?, at, "mass")?;
                if mass < 0.0 {
                    return Err(invalid(at, format!("has a negative mass {mass}")));
                }
                out.mass = mass;
            }
            "inertia" => {
                let mut value = [0.0; 6];
                for (slot, key) in value
                    .iter_mut()
                    .zip(["ixx", "ixy", "ixz", "iyy", "iyz", "izz"])
                {
                    [*slot] = numbers::<1>(required(child, key, at)?, at, key)?;
                }
                let [xx, xy, xz, yy, yz, zz] = value;
                out.inertia = DMat3::from_cols(
                    DVec3::new(xx, xy, xz),
                    DVec3::new(xy, yy, yz),
                    DVec3::new(xz, yz, zz),
                );
            }
            other => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}> in <inertial>"),
                    at: at.into(),
                });
            }
        }
    }
    Ok(out)
}

fn shape(
    node: Node,
    at: &str,
    materials: Option<&HashMap<String, Option<[f32; 4]>>>,
    meshes: &mut Meshes,
) -> Result<Shape, Error> {
    let mut placed = DMat4::IDENTITY;
    let mut geometry = None;
    let mut rgba = None;
    for child in elements(node) {
        match (child.tag_name().name(), materials) {
            ("origin", _) => placed = pose_origin(child, at)?,
            ("geometry", _) => geometry = Some(geometry_of(child, at, meshes)?),
            ("material", Some(materials)) => {
                rgba = match color(child, at)? {
                    Some(own) => Some(own),
                    None => {
                        let key = required(child, "name", at)?;
                        *materials.get(key).ok_or_else(|| {
                            invalid(at, format!("uses material {key}, which is not defined"))
                        })?
                    }
                };
            }
            (other, _) => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}> here"),
                    at: at.into(),
                });
            }
        }
    }
    Ok(Shape {
        origin: placed,
        geometry: geometry.ok_or_else(|| invalid(at, "has no <geometry>".into()))?,
        rgba,
    })
}

/// A material's own color, if it gives one.
fn color(node: Node, at: &str) -> Result<Option<[f32; 4]>, Error> {
    let mut rgba = None;
    for child in elements(node) {
        match child.tag_name().name() {
            "color" => {
                let values: [f64; 4] = numbers(required(child, "rgba", at)?, at, "rgba")?;
                if values.iter().any(|v| !(0.0..=1.0).contains(v)) {
                    return Err(invalid(at, "has a color outside 0 to 1".into()));
                }
                rgba = Some(values.map(|v| v as f32));
            }
            "texture" => {
                return Err(Error::Unsupported {
                    feature: "a material texture".into(),
                    at: at.into(),
                });
            }
            other => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}> in <material>"),
                    at: at.into(),
                });
            }
        }
    }
    Ok(rgba)
}

fn geometry_of(node: Node, at: &str, meshes: &mut Meshes) -> Result<Geometry, Error> {
    let shapes: Vec<Node> = elements(node).collect();
    let [shape] = shapes.as_slice() else {
        return Err(invalid(at, "needs exactly one shape in <geometry>".into()));
    };
    let positive = |key: &str| -> Result<f64, Error> {
        let [value] = numbers(required(*shape, key, at)?, at, key)?;
        length(value, at, key)
    };
    Ok(match shape.tag_name().name() {
        "box" => {
            let size: [f64; 3] = numbers(required(*shape, "size", at)?, at, "box size")?;
            for value in size {
                length(value, at, "box size")?;
            }
            Geometry::Box {
                size: DVec3::from(size),
            }
        }
        "cylinder" => Geometry::Cylinder {
            radius: positive("radius")?,
            length: positive("length")?,
        },
        "sphere" => Geometry::Sphere {
            radius: positive("radius")?,
        },
        "mesh" => {
            let file = required(*shape, "filename", at)?;
            let scale: [f64; 3] = match shape.attribute("scale") {
                Some(text) => numbers(text, at, "mesh scale")?,
                None => [1.0; 3],
            };
            if scale.iter().any(|s| *s <= 0.0) {
                return Err(invalid(
                    at,
                    "has a mesh scale that is not three positive numbers (mirroring is not supported)"
                        .into(),
                ));
            }
            let mesh = meshes.get(file)?;
            let extent = (0..3)
                .map(|axis| f64::from(mesh.max[axis] - mesh.min[axis]) * scale[axis])
                .fold(0.0, f64::max);
            if extent > MAX_MESH_EXTENT_METERS {
                return Err(Error::Units {
                    at: at.into(),
                    reason: format!(
                        "mesh {file} spans {extent:.1} m; URDF lengths are meters, so a millimeter mesh needs scale=\"0.001 0.001 0.001\""
                    ),
                });
            }
            Geometry::Mesh {
                file: file.into(),
                scale: DVec3::from(scale),
                mesh,
            }
        }
        other => {
            return Err(Error::Unsupported {
                feature: format!("<{other}> geometry"),
                at: at.into(),
            });
        }
    })
}

/// A positive length no larger than [`MAX_OFFSET_METERS`].
fn length(value: f64, at: &str, what: &str) -> Result<f64, Error> {
    if value <= 0.0 {
        return Err(invalid(
            at,
            format!("has a {what} of {value}, not positive"),
        ));
    }
    if value > MAX_OFFSET_METERS {
        return Err(Error::Units {
            at: at.into(),
            reason: format!("has a {what} of {value} m; URDF lengths are meters"),
        });
    }
    Ok(value)
}

fn joint(node: Node) -> Result<RawJoint, Error> {
    let name = required(node, "name", "joint")?.to_string();
    let at = format!("joint {name}");
    let kind = match required(node, "type", &at)? {
        "fixed" => JointKind::Fixed,
        "revolute" => JointKind::Revolute,
        "continuous" => JointKind::Continuous,
        "prismatic" => JointKind::Prismatic,
        kind @ ("floating" | "planar") => {
            return Err(Error::Unsupported {
                feature: format!("a {kind} joint"),
                at,
            });
        }
        other => return Err(invalid(&at, format!("has unknown joint type {other:?}"))),
    };
    let mut placed = DMat4::IDENTITY;
    let mut parent = None;
    let mut child_link = None;
    let mut axis = DVec3::X;
    let mut limit = None;
    for child in elements(node) {
        match child.tag_name().name() {
            "origin" => placed = pose_origin(child, &at)?,
            "parent" => parent = Some(required(child, "link", &at)?.to_string()),
            "child" => child_link = Some(required(child, "link", &at)?.to_string()),
            "axis" => axis = DVec3::from(numbers::<3>(required(child, "xyz", &at)?, &at, "axis")?),
            "limit" => limit = Some(limit_of(child, &at)?),
            // Damping, friction, encoder calibration, and soft limits do
            // not move a link; they are checked as numbers and kept out of
            // inspection.
            "dynamics" | "calibration" | "safety_controller" => {
                for attribute in child.attributes() {
                    numbers::<1>(attribute.value(), &at, attribute.name())?;
                }
            }
            "mimic" => {
                return Err(Error::Unsupported {
                    feature: format!(
                        "a mimic joint (following joint {})",
                        child.attribute("joint").unwrap_or("?")
                    ),
                    at,
                });
            }
            other => {
                return Err(Error::Unsupported {
                    feature: format!("<{other}> in a joint"),
                    at,
                });
            }
        }
    }
    let parent = parent.ok_or_else(|| invalid(&at, "has no <parent link=\"...\"/>".into()))?;
    let child = child_link.ok_or_else(|| invalid(&at, "has no <child link=\"...\"/>".into()))?;
    if kind != JointKind::Fixed {
        if (axis.length() - 1.0).abs() > AXIS_TOLERANCE {
            return Err(invalid(
                &at,
                format!(
                    "has axis {} {} {}, which is not a unit vector",
                    axis.x, axis.y, axis.z
                ),
            ));
        }
        axis = axis.normalize();
    }
    let limit = match kind {
        JointKind::Fixed | JointKind::Continuous => None,
        JointKind::Revolute | JointKind::Prismatic => {
            let limit = limit.ok_or_else(|| {
                invalid(&at, "is a moving joint with no <limit lower upper>".into())
            })?;
            let (bound, unit) = if kind == JointKind::Revolute {
                (std::f64::consts::TAU + 1e-6, "radians, not degrees")
            } else {
                (MAX_OFFSET_METERS, "meters")
            };
            for value in [limit.lower, limit.upper] {
                if value.abs() > bound {
                    return Err(Error::Units {
                        at,
                        reason: format!("has a limit of {value}; URDF joint limits are {unit}"),
                    });
                }
            }
            Some(limit)
        }
    };
    Ok(RawJoint {
        name,
        kind,
        parent,
        child,
        origin: placed,
        axis,
        limit,
    })
}

fn limit_of(node: Node, at: &str) -> Result<Limit, Error> {
    let value = |key: &str| -> Result<f64, Error> {
        node.attribute(key)
            .map_or(Ok(0.0), |text| numbers::<1>(text, at, key).map(|[v]| v))
    };
    let limit = Limit {
        lower: value("lower")?,
        upper: value("upper")?,
        effort: value("effort")?,
        velocity: value("velocity")?,
    };
    if limit.lower >= limit.upper {
        return Err(invalid(
            at,
            format!(
                "has limits [{}, {}], which leave no range of motion",
                limit.lower, limit.upper
            ),
        ));
    }
    if limit.effort < 0.0 || limit.velocity < 0.0 {
        return Err(invalid(
            at,
            "has a negative effort or velocity limit".into(),
        ));
    }
    Ok(limit)
}

/// An `<origin xyz rpy>`, both optional.
fn pose_origin(node: Node, at: &str) -> Result<DMat4, Error> {
    let read = |key: &str| -> Result<DVec3, Error> {
        node.attribute(key).map_or(Ok(DVec3::ZERO), |text| {
            numbers::<3>(text, at, key).map(DVec3::from)
        })
    };
    let xyz = read("xyz")?;
    let rpy = read("rpy")?;
    if xyz.length() > MAX_OFFSET_METERS {
        return Err(Error::Units {
            at: at.into(),
            reason: format!(
                "has an origin {:.1} m from its parent; URDF lengths are meters",
                xyz.length()
            ),
        });
    }
    Ok(origin(xyz, rpy))
}

fn numbers<const N: usize>(text: &str, at: &str, what: &str) -> Result<[f64; N], Error> {
    let values: Vec<f64> = text
        .split_ascii_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| invalid(at, format!("has {what} {text:?}, which is not numbers")))?;
    let values: [f64; N] = values
        .try_into()
        .map_err(|_| invalid(at, format!("has {what} {text:?}; it needs {N} numbers")))?;
    if values.iter().any(|v| !v.is_finite()) {
        return Err(invalid(
            at,
            format!("has {what} {text:?}, which is not finite"),
        ));
    }
    Ok(values)
}

fn required<'a>(node: Node<'a, '_>, key: &str, at: &str) -> Result<&'a str, Error> {
    match node.attribute(key) {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(invalid(
            at,
            format!("<{}> has no {key}", node.tag_name().name()),
        )),
    }
}

fn invalid(at: &str, reason: String) -> Error {
    Error::Invalid {
        at: at.into(),
        reason,
    }
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(Node::is_element)
}

/// The meshes one description reads, resolved under its folder and bounded.
struct Meshes<'a> {
    folder: &'a Path,
    canonical: Option<PathBuf>,
    loaded: HashMap<PathBuf, Arc<TriangleMesh>>,
    bytes: u64,
    reader: Option<&'a dyn Fn(&str) -> Result<Vec<u8>, Error>>,
}

impl<'a> Meshes<'a> {
    fn new(folder: &'a Path) -> Self {
        Self {
            folder,
            canonical: None,
            loaded: HashMap::new(),
            bytes: 0,
            reader: None,
        }
    }

    fn get(&mut self, file: &str) -> Result<Arc<TriangleMesh>, Error> {
        let refuse = |reason: &str| Error::AssetPath {
            path: file.into(),
            reason: reason.into(),
        };
        if file.contains("://") {
            return Err(refuse(
                "is a URL (such as package://); meshes must be paths relative to the description file",
            ));
        }
        if file.contains('\\') {
            return Err(refuse("uses backslashes; write mesh paths with /"));
        }
        let relative = Path::new(file);
        let drive = file.len() >= 2 && file.as_bytes()[1] == b':';
        if relative.has_root() || relative.is_absolute() || drive {
            return Err(refuse(
                "is absolute; meshes must be paths relative to the description file",
            ));
        }
        for part in relative.components() {
            match part {
                Component::Normal(_) | Component::CurDir => {}
                Component::ParentDir => {
                    return Err(refuse(
                        "climbs out with ..; meshes must stay under the description's folder",
                    ));
                }
                Component::RootDir | Component::Prefix(_) => {
                    return Err(refuse(
                        "is absolute; meshes must be paths relative to the description file",
                    ));
                }
            }
        }
        let extension = relative
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if extension != "stl" {
            return Err(Error::Unsupported {
                feature: format!("mesh format {extension:?} (only STL is read)"),
                at: file.into(),
            });
        }
        if let Some(reader) = self.reader {
            let path = relative.to_path_buf();
            if let Some(mesh) = self.loaded.get(&path) {
                return Ok(mesh.clone());
            }
            if self.loaded.len() >= MAX_MESHES {
                return Err(Error::TooLarge {
                    what: "the number of robot meshes".into(),
                    limit: MAX_MESHES as u64,
                    actual: self.loaded.len() as u64 + 1,
                });
            }
            let bytes = reader(file)?;
            if bytes.len() as u64 > MAX_MESH_BYTES {
                return Err(Error::TooLarge {
                    what: format!("mesh {file} in bytes"),
                    limit: MAX_MESH_BYTES,
                    actual: bytes.len() as u64,
                });
            }
            self.bytes += bytes.len() as u64;
            if self.bytes > MAX_TOTAL_MESH_BYTES {
                return Err(Error::TooLarge {
                    what: "all robot meshes in bytes".into(),
                    limit: MAX_TOTAL_MESH_BYTES,
                    actual: self.bytes,
                });
            }
            let mesh = Arc::new(crate::stl::parse(&bytes).map_err(|reason| Error::Mesh {
                path: file.into(),
                reason,
            })?);
            self.loaded.insert(path, mesh.clone());
            return Ok(mesh);
        }
        let folder = match &self.canonical {
            Some(folder) => folder.clone(),
            None => {
                let folder = self.folder.canonicalize().map_err(|e| Error::Read {
                    path: self.folder.display().to_string(),
                    reason: e.to_string(),
                })?;
                self.canonical = Some(folder.clone());
                folder
            }
        };
        let path = folder
            .join(relative)
            .canonicalize()
            .map_err(|_| refuse("does not exist under the description's folder"))?;
        if !path.starts_with(&folder) {
            return Err(refuse(
                "leads outside the description's folder through a link",
            ));
        }
        if let Some(mesh) = self.loaded.get(&path) {
            return Ok(mesh.clone());
        }
        if self.loaded.len() == MAX_MESHES {
            return Err(Error::TooLarge {
                what: "the number of distinct mesh files".into(),
                limit: MAX_MESHES as u64,
                actual: MAX_MESHES as u64 + 1,
            });
        }
        let metadata = std::fs::metadata(&path).map_err(|_| refuse("cannot be read"))?;
        if !metadata.is_file() {
            return Err(refuse("is not a file"));
        }
        if metadata.len() > MAX_MESH_BYTES {
            return Err(Error::TooLarge {
                what: format!("mesh {file} in bytes"),
                limit: MAX_MESH_BYTES,
                actual: metadata.len(),
            });
        }
        self.bytes += metadata.len();
        if self.bytes > MAX_TOTAL_MESH_BYTES {
            return Err(Error::TooLarge {
                what: "all meshes together in bytes".into(),
                limit: MAX_TOTAL_MESH_BYTES,
                actual: self.bytes,
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| Error::Read {
            path: file.into(),
            reason: e.to_string(),
        })?;
        let mesh = Arc::new(crate::stl::parse(&bytes).map_err(|reason| Error::Mesh {
            path: file.into(),
            reason,
        })?);
        self.loaded.insert(path, mesh.clone());
        Ok(mesh)
    }
}
