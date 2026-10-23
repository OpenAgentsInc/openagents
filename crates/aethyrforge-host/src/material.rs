//! Editable, instance-scoped color layers and the local stylized-scar generator.
use crate::{Error, ProjectResource, protocol::id};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MATERIAL_SCHEMA: &str = "openagents.verse.material.v1";
pub const SCAR_BACKEND: &str = "local-stylized-scars-v1";
pub const MATERIAL_SIZE: usize = 256;
pub const MAX_MATERIAL_PNG: usize = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialTarget {
    pub entity: String,
    pub asset: String,
    pub surface: String,
    pub material: String,
    pub uv_set: u8,
    pub size: [u16; 2],
    pub uv_bounds: [f64; 4],
}
impl MaterialTarget {
    pub fn werewolf_face(entity: &str) -> Self {
        Self {
            entity: entity.into(),
            asset: "custom:haunt:werewolf".into(),
            surface: "werewolf.face".into(),
            material: "custom:haunt:werewolf:face".into(),
            uv_set: 0,
            size: [256, 256],
            uv_bounds: [0., 0., 1., 1.],
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if !id(&self.entity) || self != &Self::werewolf_face(&self.entity) {
            return Err(Error::Invalid("editable material target"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ColorLayer {
    pub image: Option<ProjectResource>,
    pub visible: bool,
    pub strength: f64,
}
impl Default for ColorLayer {
    fn default() -> Self {
        Self {
            image: None,
            visible: true,
            strength: 1.,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScarArea {
    LeftCheek,
    RightCheek,
    Forehead,
}
impl ScarArea {
    /// The custom face has a dedicated cylindrical UV island; these regions
    /// refer to the character's own left and right, facing positive Z.
    pub fn bounds(self) -> [f64; 4] {
        match self {
            Self::LeftCheek => [0.07, 0.54, 0.23, 0.78],
            Self::RightCheek => [0.27, 0.54, 0.43, 0.78],
            Self::Forehead => [0.16, 0.34, 0.34, 0.49],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScarRecipe {
    pub backend: String,
    pub request: String,
    pub seed: u32,
    pub area: ScarArea,
    pub count: u8,
    pub width: f64,
    pub angle: f64,
    pub color: [u8; 4],
    pub healed: bool,
}
impl ScarRecipe {
    /// This is a local procedural generator, not an inference provider. Its
    /// supported vocabulary changes surface marks, never geometry or animation.
    pub fn from_request(request: &str, area: ScarArea, seed: u32) -> Result<Self, &'static str> {
        if request.is_empty() || request.len() > 1200 || request.chars().any(char::is_control) {
            return Err("Describe scars in one short sentence.");
        }
        let lower = request.to_ascii_lowercase();
        let controls = [
            "; keep",
            ", keep",
            " keep the",
            " keep its",
            " while keeping",
        ]
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min()
        .map_or(lower.as_str(), |end| &lower[..end]);
        let words: Vec<_> = controls
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect();
        if words.iter().any(|s| {
            matches!(
                *s,
                "remove" | "erase" | "delete" | "clear" | "no" | "not" | "without"
            )
        }) || controls.contains("don't")
            || controls.contains("don’t")
        {
            return Err("Describe scars to add. Use Edit by hand to erase existing marks.");
        }
        if !words.iter().any(|s| matches!(*s, "scar" | "scars")) {
            return Err(
                "The local generator makes stylized scars. Use Edit by hand for other color changes.",
            );
        }
        if words.iter().any(|s| {
            matches!(
                *s,
                "animate"
                    | "animation"
                    | "rig"
                    | "sculpt"
                    | "geometry"
                    | "hat"
                    | "tail"
                    | "muzzle"
                    | "nose"
                    | "arm"
                    | "torso"
                    | "ears"
                    | "ear"
                    | "eyes"
                    | "eye"
                    | "legs"
                    | "leg"
            )
        }) {
            return Err(
                "Choose cheek or forehead scars. This tool changes the face's color layer.",
            );
        }
        if words.iter().any(|s| {
            matches!(
                *s,
                "rainbow" | "metallic" | "glow" | "glowing" | "sparkling" | "iridescent"
            )
        }) {
            return Err(
                "Choose a solid scar color. Light effects and color patterns aren't supported here.",
            );
        }
        for (a, b, message) in [
            ("thin", "thick", "Choose thin or thick scars."),
            (
                "vertical",
                "horizontal",
                "Choose vertical or horizontal scars.",
            ),
            ("fresh", "healed", "Choose healed or fresh scars."),
        ] {
            if words.contains(&a) && words.contains(&b) {
                return Err(message);
            }
        }
        let mut requested_count = None;
        for word in &words {
            let number = match *word {
                "one" => Some(1),
                "two" => Some(2),
                "three" | "few" => Some(3),
                "four" => Some(4),
                "five" => Some(5),
                "six" => Some(6),
                "seven" => Some(7),
                "eight" => Some(8),
                s => {
                    if s.bytes().all(|b| b.is_ascii_digit()) {
                        let n = s
                            .parse::<u8>()
                            .map_err(|_| "Choose between one and eight scars.")?;
                        Some(n)
                    } else {
                        None
                    }
                }
            };
            if let Some(n) = number {
                if !(1..=8).contains(&n) {
                    return Err("Choose between one and eight scars.");
                }
                if requested_count.is_some_and(|old| old != n) {
                    return Err("Choose one scar count between one and eight.");
                }
                requested_count = Some(n);
            }
        }
        let count = requested_count.unwrap_or(3);
        let cheek = words.contains(&"cheek") || words.contains(&"cheeks");
        if ((words.contains(&"left") || words.contains(&"right")) && !cheek)
            || words.contains(&"whole")
            || words.contains(&"entire")
        {
            return Err("Choose one area: left cheek, right cheek, or forehead.");
        }
        let requested_areas = [
            (words.contains(&"left") && cheek, ScarArea::LeftCheek),
            (words.contains(&"right") && cheek, ScarArea::RightCheek),
            (words.contains(&"forehead"), ScarArea::Forehead),
        ]
        .into_iter()
        .filter(|(mentioned, _)| *mentioned)
        .map(|(_, area)| area)
        .collect::<Vec<_>>();
        if requested_areas.len() > 1
            || words.contains(&"both")
            || (words.contains(&"left") && words.contains(&"right"))
        {
            return Err("Choose one area: left cheek, right cheek, or forehead.");
        }
        let area = requested_areas.first().copied().unwrap_or(area);
        let healed = !words.contains(&"fresh");
        let colors = [
            ("pink", [194, 119, 111, 225]),
            ("brown", [98, 57, 44, 230]),
            ("red", [140, 62, 54, 230]),
            ("blue", [72, 104, 160, 225]),
            ("green", [79, 119, 77, 225]),
            ("purple", [132, 87, 149, 225]),
            ("tan", [167, 120, 102, 215]),
            ("black", [28, 26, 25, 225]),
            ("white", [224, 217, 205, 225]),
            ("orange", [190, 112, 53, 225]),
            ("yellow", [200, 169, 78, 225]),
            ("gray", [119, 119, 119, 225]),
            ("grey", [119, 119, 119, 225]),
        ];
        let mut requested_colors = Vec::new();
        for (name, color) in colors {
            if words.contains(&name) && !requested_colors.contains(&color) {
                requested_colors.push(color);
            }
        }
        if requested_colors.len() > 1 {
            return Err("Choose one solid scar color for this layer.");
        }
        let color = requested_colors.first().copied().unwrap_or(if healed {
            [167, 120, 102, 215]
        } else {
            [140, 62, 54, 230]
        });
        Ok(Self {
            backend: SCAR_BACKEND.into(),
            request: request.into(),
            seed,
            area,
            count,
            width: if words.contains(&"thin") {
                0.005
            } else if words.contains(&"thick") {
                0.016
            } else {
                0.009
            },
            angle: if words.contains(&"vertical") {
                std::f64::consts::FRAC_PI_2
            } else if words.contains(&"horizontal") {
                0.
            } else {
                0.8
            },
            color,
            healed,
        })
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.backend != SCAR_BACKEND
            || self.request.len() > 1200
            || self.request.chars().any(char::is_control)
            || !(1..=8).contains(&self.count)
            || !self.width.is_finite()
            || !(0.002..=0.025).contains(&self.width)
            || !self.angle.is_finite()
            || self.angle.abs() > std::f64::consts::PI
        {
            return Err(Error::Invalid("scar recipe"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialDocument {
    pub schema: String,
    pub target: MaterialTarget,
    pub generated: ColorLayer,
    pub handmade: ColorLayer,
    pub recipe: Option<ScarRecipe>,
}
impl MaterialDocument {
    pub fn blank(target: MaterialTarget) -> Self {
        Self {
            schema: MATERIAL_SCHEMA.into(),
            target,
            generated: ColorLayer::default(),
            handmade: ColorLayer::default(),
            recipe: None,
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        self.target.validate()?;
        if self.schema != MATERIAL_SCHEMA {
            return Err(Error::Invalid("material document"));
        }
        for layer in [&self.generated, &self.handmade] {
            if !layer.strength.is_finite()
                || !(0. ..=1.).contains(&layer.strength)
                || layer.image.as_ref().is_some_and(|r| {
                    r.media_type != "image/png"
                        || r.bytes == 0
                        || r.bytes > MAX_MATERIAL_PNG as u64
                        || (r.sha256.len() != 64
                            || !r
                                .sha256
                                .bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
                })
            {
                return Err(Error::Invalid("material layer"));
            }
        }
        if let Some(recipe) = &self.recipe {
            recipe.validate()?;
        }
        Ok(())
    }
    pub fn key(&self) -> String {
        crate::project_hash(
            format!(
                "{}\n{}\n{}",
                self.target.entity, self.target.asset, self.target.surface
            )
            .as_bytes(),
        )
    }
    pub fn keep_in(&self, project: &mut crate::ProjectDocument) -> Result<(), Error> {
        self.validate()?;
        if project
            .entities
            .get(&self.target.entity)
            .is_none_or(|e| e.asset != self.target.asset)
        {
            return Err(Error::Invalid("saved material target"));
        }
        let key = self.key();
        let name = format!("material-v1-{key}.json");
        let surface = format!("material-{key}");
        let text = serde_json::to_string(self).map_err(|_| Error::Invalid("material document"))?;
        let mut candidate = project.clone();
        candidate.originals.retain(|o| o.name != name);
        candidate.originals.push(crate::OriginalImport {
            name,
            sha256: crate::project_hash(text.as_bytes()),
            text,
        });
        candidate.layers.retain(|layer| layer.surface != surface);
        for (kind, layer) in [("generated", &self.generated), ("paint", &self.handmade)] {
            if let Some(texture) = &layer.image {
                candidate.layers.push(crate::MaterialLayer {
                    material: self.target.material.clone(),
                    surface: surface.clone(),
                    texture: texture.clone(),
                    uv_set: 0,
                    kind: kind.into(),
                });
            }
        }
        candidate.validate()?;
        *project = candidate;
        Ok(())
    }
}

pub fn project_materials(project: &crate::ProjectDocument) -> Result<Vec<MaterialDocument>, Error> {
    project.validate()?;
    let documents = validate_project_materials(project)?;
    if project.layers.iter().any(|l| {
        !documents
            .iter()
            .any(|d| l.surface == format!("material-{}", d.key()))
    }) {
        return Err(Error::Invalid("unavailable material layer"));
    }
    Ok(documents)
}

/// Checks saved material sources without recursively validating their project.
pub(crate) fn validate_project_materials(
    project: &crate::ProjectDocument,
) -> Result<Vec<MaterialDocument>, Error> {
    let mut documents = Vec::new();
    for source in project
        .originals
        .iter()
        .filter(|o| o.name.starts_with("material-v1-"))
    {
        let document: MaterialDocument =
            serde_json::from_str(&source.text).map_err(|_| Error::Invalid("saved material"))?;
        document.validate()?;
        if source.name != format!("material-v1-{}.json", document.key())
            || documents
                .iter()
                .any(|d: &MaterialDocument| d.target == document.target)
            || project
                .entities
                .get(&document.target.entity)
                .is_none_or(|e| e.asset != document.target.asset)
        {
            return Err(Error::Invalid("saved material target"));
        }
        let surface = format!("material-{}", document.key());
        let expected: Vec<_> = [
            ("generated", &document.generated),
            ("paint", &document.handmade),
        ]
        .into_iter()
        .filter_map(|(kind, layer)| {
            layer.image.as_ref().map(|texture| crate::MaterialLayer {
                material: document.target.material.clone(),
                surface: surface.clone(),
                texture: texture.clone(),
                uv_set: 0,
                kind: kind.into(),
            })
        })
        .collect();
        if expected.iter().collect::<Vec<_>>()
            != project
                .layers
                .iter()
                .filter(|l| l.surface == surface)
                .collect::<Vec<_>>()
        {
            return Err(Error::Invalid("saved material images"));
        }
        documents.push(document);
    }
    if project.layers.iter().any(|l| {
        l.surface.starts_with("material-")
            && !documents
                .iter()
                .any(|d| l.surface == format!("material-{}", d.key()))
    }) {
        return Err(Error::Invalid("unavailable material layer"));
    }
    Ok(documents)
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialImage {
    pub resource: ProjectResource,
    pub png_base64: String,
}
impl MaterialImage {
    pub fn new(bytes: &[u8]) -> Result<Self, Error> {
        use base64::Engine;
        let image = Self {
            resource: ProjectResource {
                sha256: crate::project_hash(bytes),
                bytes: bytes.len() as u64,
                media_type: "image/png".into(),
            },
            png_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        };
        image.validate()?;
        Ok(image)
    }
    /// Full PNG decoding also runs before GPU use and in the native file store.
    pub fn validate(&self) -> Result<Vec<u8>, Error> {
        use base64::Engine;
        if self.png_base64.len() > MAX_MATERIAL_PNG.div_ceil(3) * 4
            || self.resource.media_type != "image/png"
        {
            return Err(Error::Invalid("material PNG size"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.png_base64)
            .map_err(|_| Error::Invalid("material PNG"))?;
        if bytes.len() < 24
            || bytes.len() > MAX_MATERIAL_PNG
            || bytes.len() as u64 != self.resource.bytes
            || bytes[..8] != *b"\x89PNG\r\n\x1a\n"
            || bytes[12..16] != *b"IHDR"
            || bytes[16..20] != 256u32.to_be_bytes()
            || bytes[20..24] != 256u32.to_be_bytes()
            || crate::project_hash(&bytes) != self.resource.sha256
        {
            return Err(Error::Invalid("material PNG"));
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialBundle {
    pub document: MaterialDocument,
    pub generated: Option<MaterialImage>,
    pub handmade: Option<MaterialImage>,
}
impl MaterialBundle {
    pub fn validate(&self) -> Result<(), Error> {
        self.document.validate()?;
        for (image, layer) in [
            (&self.generated, &self.document.generated),
            (&self.handmade, &self.document.handmade),
        ] {
            if image.as_ref().map(|i| &i.resource) != layer.image.as_ref() {
                return Err(Error::Invalid("material source image"));
            }
            if let Some(image) = image {
                image.validate()?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrushMode {
    Paint,
    Erase,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BrushStroke {
    pub points: Vec<[f64; 2]>,
    pub radius: f64,
    pub opacity: f64,
    pub color: [u8; 3],
    pub mode: BrushMode,
}
impl BrushStroke {
    pub fn validate(&self) -> Result<(), Error> {
        if self.points.is_empty()
            || self.points.len() > 256
            || self
                .points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || !self.radius.is_finite()
            || !(0.001..=0.15).contains(&self.radius)
            || !self.opacity.is_finite()
            || !(0. ..=1.).contains(&self.opacity)
        {
            return Err(Error::Invalid("brush stroke"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialPixels(pub Vec<u8>);
impl Default for MaterialPixels {
    fn default() -> Self {
        Self(vec![0; MATERIAL_SIZE * MATERIAL_SIZE * 4])
    }
}
impl MaterialPixels {
    pub fn stroke(&mut self, stroke: &BrushStroke) -> Result<(), Error> {
        stroke.validate()?;
        if self.0.len() != MATERIAL_SIZE * MATERIAL_SIZE * 4 {
            return Err(Error::Invalid("material pixels"));
        }
        let mut a = stroke.points[0];
        for &b in &stroke.points {
            let length = (b[0] - a[0]).hypot(b[1] - a[1]);
            let steps = (length / (stroke.radius * 0.4)).ceil().max(1.) as usize;
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                self.dab([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], stroke);
            }
            a = b;
        }
        Ok(())
    }
    fn dab(&mut self, p: [f64; 2], stroke: &BrushStroke) {
        let n = MATERIAL_SIZE as f64;
        let x0 = ((p[0] - stroke.radius) * n).floor().max(0.) as usize;
        let x1 = ((p[0] + stroke.radius) * n).ceil().min(n) as usize;
        let y0 = ((p[1] - stroke.radius) * n).floor().max(0.) as usize;
        let y1 = ((p[1] + stroke.radius) * n).ceil().min(n) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let distance = ((x as f64 + 0.5) / n - p[0]).hypot((y as f64 + 0.5) / n - p[1]);
                let alpha = ((stroke.radius - distance) * n).clamp(0., 1.) * stroke.opacity;
                let pixel = &mut self.0[(y * MATERIAL_SIZE + x) * 4..][..4];
                if stroke.mode == BrushMode::Erase {
                    pixel[3] = (f64::from(pixel[3]) * (1. - alpha)).round() as u8;
                } else {
                    over(pixel, stroke.color, alpha);
                }
            }
        }
    }
    pub fn composite(
        &self,
        other: &Self,
        base_strength: f64,
        top_strength: f64,
    ) -> Result<Self, Error> {
        let size = MATERIAL_SIZE * MATERIAL_SIZE * 4;
        if self.0.len() != size
            || other.0.len() != size
            || [base_strength, top_strength]
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err(Error::Invalid("material composition"));
        }
        let mut result = self.clone();
        for (base, top) in result.0.chunks_exact_mut(4).zip(other.0.chunks_exact(4)) {
            base[3] = (f64::from(base[3]) * base_strength).round() as u8;
            over(
                base,
                [top[0], top[1], top[2]],
                f64::from(top[3]) / 255. * top_strength,
            );
        }
        Ok(result)
    }
}
fn over(base: &mut [u8], rgb: [u8; 3], alpha: f64) {
    let old = f64::from(base[3]) / 255.;
    let total = alpha + old * (1. - alpha);
    if total > 0. {
        for i in 0..3 {
            base[i] = ((f64::from(rgb[i]) * alpha + f64::from(base[i]) * old * (1. - alpha))
                / total)
                .round() as u8;
        }
    }
    base[3] = (total * 255.).round() as u8;
}
pub fn generate_scars(recipe: &ScarRecipe) -> Result<MaterialPixels, Error> {
    recipe.validate()?;
    let [u0, v0, u1, v1] = recipe.area.bounds();
    let w = u1 - u0;
    let h = v1 - v0;
    let mut state = recipe.seed.max(1);
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        f64::from(state) / f64::from(u32::MAX)
    };
    let mut image = MaterialPixels::default();
    for _ in 0..recipe.count {
        let c = [
            u0 + w * (0.3 + random() * 0.4),
            v0 + h * (0.3 + random() * 0.4),
        ];
        let length = h * (0.17 + random() * 0.11);
        let angle = recipe.angle + (random() - 0.5) * 0.3;
        let delta = [angle.cos() * length * 0.5, angle.sin() * length * 0.5];
        let points = vec![
            [c[0] - delta[0], c[1] - delta[1]],
            [c[0] + delta[0], c[1] + delta[1]],
        ];
        image.stroke(&BrushStroke {
            points,
            radius: recipe.width / 2.,
            opacity: f64::from(recipe.color[3]) / 255.,
            color: [recipe.color[0], recipe.color[1], recipe.color[2]],
            mode: BrushMode::Paint,
        })?;
        if recipe.healed {
            for t in [-0.5, 0., 0.5] {
                let center = [c[0] + delta[0] * t, c[1] + delta[1] * t];
                let side = [-angle.sin() * w * 0.035, angle.cos() * w * 0.035];
                image.stroke(&BrushStroke {
                    points: vec![
                        [center[0] - side[0], center[1] - side[1]],
                        [center[0] + side[0], center[1] + side[1]],
                    ],
                    radius: 0.002,
                    opacity: 0.55,
                    color: [210, 167, 144],
                    mode: BrushMode::Paint,
                })?;
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn project() -> crate::ProjectDocument {
        serde_json::from_value(serde_json::json!({"schema":"openagents.verse.project.v1","namespace":"haunted-house","template":{"world":"house","engine_release":"fixture-6","asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64)},"source_revision":0,"entities":{"wolf-a":{"asset":"custom:haunt:werewolf","position":[0,0,0],"yaw":0,"scale":1}},"rooms":{},"removed_walls":[],"materials":{},"environment":{},"source_draft":"","active_source":"","layers":[],"originals":[]})).unwrap()
    }
    #[test]
    fn saved_material_sources_and_renderable_bundles_share_the_same_image_bound() {
        let resource = ProjectResource {
            sha256: "a".repeat(64),
            bytes: MAX_MATERIAL_PNG as u64,
            media_type: "image/png".into(),
        };
        let mut material = MaterialDocument::blank(MaterialTarget::werewolf_face("wolf-a"));
        material.generated.image = Some(resource);
        material.validate().unwrap();
        let mut saved = project();
        material.keep_in(&mut saved).unwrap();
        assert_eq!(project_materials(&saved).unwrap(), vec![material.clone()]);
        let before = saved.clone();
        material.generated.image.as_mut().unwrap().bytes += 1;
        assert!(material.validate().is_err());
        assert!(material.keep_in(&mut saved).is_err());
        assert_eq!(saved, before);
        // Simulate an external client bypassing keep_in with matching source and layer references.
        let source = &mut saved.originals[0];
        source.text = serde_json::to_string(&material).unwrap();
        source.sha256 = crate::project_hash(source.text.as_bytes());
        saved.layers[0].texture.bytes += 1;
        assert!(saved.validate().is_err());
        assert!(project_materials(&saved).is_err());
        let image = MaterialImage {
            resource: material.generated.image.clone().unwrap(),
            png_base64: "a".repeat((MAX_MATERIAL_PNG + 1).div_ceil(3) * 4),
        };
        assert!(image.validate().is_err());
        let bundle = MaterialBundle {
            document: material,
            generated: Some(image),
            handmade: None,
        };
        assert!(bundle.validate().is_err());
    }
    #[test]
    fn pretty_material_source_keeps_exact_text_within_the_generic_source_bound() {
        let mut saved = project();
        let material = MaterialDocument::blank(MaterialTarget::werewolf_face("wolf-a"));
        material.keep_in(&mut saved).unwrap();
        let source = &mut saved.originals[0];
        source.text = format!(
            "{}\n{}",
            serde_json::to_string_pretty(&material).unwrap(),
            " ".repeat(5000)
        );
        source.sha256 = crate::project_hash(source.text.as_bytes());
        let text = source.text.clone();
        assert!(text.len() > 4096 && text.len() <= 24 * 1024);
        saved.validate().unwrap();
        assert_eq!(project_materials(&saved).unwrap(), vec![material]);
        assert_eq!(saved.originals[0].text, text);
        saved.originals[0].text.push_str(&" ".repeat(24 * 1024));
        saved.originals[0].sha256 = crate::project_hash(saved.originals[0].text.as_bytes());
        assert!(saved.validate().is_err());
    }
    #[test]
    fn saved_materials_bind_the_exact_instance_surface_and_source_images() {
        let mut saved = project();
        let mut material = MaterialDocument::blank(MaterialTarget::werewolf_face("wolf-a"));
        material.handmade.image = Some(ProjectResource {
            sha256: "a".repeat(64),
            bytes: 300,
            media_type: "image/png".into(),
        });
        material.keep_in(&mut saved).unwrap();
        let accepted = saved.clone();
        saved.layers[0].texture.sha256 = "b".repeat(64);
        assert!(saved.validate().is_err());
        saved = accepted.clone();
        saved.entities.get_mut("wolf-a").unwrap().asset = "kit:chair".into();
        assert!(saved.validate().is_err());
        saved = accepted.clone();
        saved.originals.push(saved.originals[0].clone());
        assert!(saved.validate().is_err());
        saved = accepted;
        saved.originals.clear();
        assert!(saved.validate().is_err());
    }
    #[test]
    fn prompts_honor_blue_and_refuse_competing_areas_removal_and_conflicting_controls() {
        let blue = ScarRecipe::from_request("two blue scars", ScarArea::LeftCheek, 4).unwrap();
        assert_eq!(blue.count, 2);
        assert_eq!(blue.color, [72, 104, 160, 225]);
        let preserved = ScarRecipe::from_request(
            "two pink scars on the left cheek; keep the brown fur and expression",
            ScarArea::Forehead,
            4,
        )
        .unwrap();
        assert_eq!(preserved.color, [194, 119, 111, 225]);
        assert_eq!(preserved.area, ScarArea::LeftCheek);
        let pixels = generate_scars(&blue).unwrap();
        let previous = pixels.clone();
        for text in [
            "scars on the left cheek and forehead",
            "scars on both cheeks",
            "remove scars",
            "no scars",
            "don't add scars",
            "add two blue and pink scars",
            "two thin and thick scars",
            "two vertical horizontal scars",
            "two healed fresh scars",
            "two or four scars",
            "add 999 scars",
            "make scars glow",
        ] {
            assert!(
                ScarRecipe::from_request(text, ScarArea::LeftCheek, 5).is_err(),
                "{text}"
            );
            assert_eq!(pixels, previous);
        }
    }
    #[test]
    fn requests_produce_new_localized_pixels_without_touching_handmade_work() {
        let a = ScarRecipe::from_request(
            "Add two thin healed scars across the left cheek",
            ScarArea::Forehead,
            19,
        )
        .unwrap();
        let b = ScarRecipe::from_request(
            "Add five thick pink scars across the right cheek",
            ScarArea::Forehead,
            20,
        )
        .unwrap();
        let first = generate_scars(&a).unwrap();
        let second = generate_scars(&b).unwrap();
        assert_ne!(first, second);
        assert_eq!(a.count, 2);
        assert_eq!(b.count, 5);
        let [x0, y0, x1, y1] = a.area.bounds();
        let mut painted = 0;
        for (i, _p) in first
            .0
            .chunks_exact(4)
            .enumerate()
            .filter(|(_, p)| p[3] > 0)
        {
            painted += 1;
            let x = (i % 256) as f64 / 256.;
            let y = (i / 256) as f64 / 256.;
            assert!(x >= x0 && x <= x1 && y >= y0 && y <= y1);
        }
        assert!(painted > 10);
        let mut handmade = MaterialPixels::default();
        handmade
            .stroke(&BrushStroke {
                points: vec![[0.7, 0.7], [0.8, 0.8]],
                radius: 0.02,
                opacity: 0.8,
                color: [80, 120, 170],
                mode: BrushMode::Paint,
            })
            .unwrap();
        let saved = handmade.clone();
        assert_ne!(
            first.composite(&handmade, 1., 1.).unwrap(),
            second.composite(&handmade, 1., 1.).unwrap()
        );
        assert_eq!(saved, handmade);
        let mut erased = handmade.clone();
        erased
            .stroke(&BrushStroke {
                points: vec![[0.75, 0.75]],
                radius: 0.1,
                opacity: 1.,
                color: [0; 3],
                mode: BrushMode::Erase,
            })
            .unwrap();
        assert!(
            erased
                .0
                .chunks_exact(4)
                .map(|p| u64::from(p[3]))
                .sum::<u64>()
                < handmade
                    .0
                    .chunks_exact(4)
                    .map(|p| u64::from(p[3]))
                    .sum::<u64>()
        );
    }
    #[test]
    fn unsupported_requests_and_invalid_strokes_do_not_change_pixels_or_target() {
        assert!(ScarRecipe::from_request("Add a hat", ScarArea::LeftCheek, 1).is_err());
        assert!(ScarRecipe::from_request("Sculpt nose scars", ScarArea::LeftCheek, 1).is_err());
        assert!(ScarRecipe::from_request("Add 9 scars", ScarArea::LeftCheek, 1).is_err());
        let mut pixels = MaterialPixels::default();
        let old = pixels.clone();
        assert!(
            pixels
                .stroke(&BrushStroke {
                    points: vec![[f64::NAN, 0.]],
                    radius: 0.02,
                    opacity: 1.,
                    color: [0; 3],
                    mode: BrushMode::Paint
                })
                .is_err()
        );
        assert_eq!(pixels, old);
        let target = MaterialTarget::werewolf_face("wolf-a");
        target.validate().unwrap();
        let mut wrong = target.clone();
        wrong.material = "shared-atlas".into();
        assert!(wrong.validate().is_err());
        assert_ne!(target, MaterialTarget::werewolf_face("wolf-b"));
    }
}
