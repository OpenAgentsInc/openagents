//! The textured zone pack format, `VTP3`.
//!
//! A pack holds base-color textures as PNG, materials with their alpha mode
//! and face culling, static models whose primitives index one material each,
//! at most one skinned character: a joint hierarchy, skinned primitives,
//! and named clips of joint keys, and optionally the forms a character can
//! take, each another skinned character, such as the Grove's Wild Shape
//! beasts. It holds no scripts, URLs, or shaders.
//! Every count, name, coordinate, and decoded allocation is bounded before it
//! is allocated.
//!
//! The textures come first, already compressed. Everything after them, the
//! body, is one raw deflate stream, and its vertices are quantized: each
//! primitive stores its positions and texture coordinates as 16-bit steps
//! from an origin, in planes (every x, then every y, and so on), which
//! deflate compresses far better than interleaved floats.
//!
//! All integers and floats are little-endian:
//!
//! ```text
//! magic           8 bytes  "VTP3\r\n\x1a\n"
//! texture count   u32
//!   name          u8 length, UTF-8 bytes
//!   width, height u32, u32
//!   png length    u32, then the PNG bytes (8-bit gray, RGB, or RGBA)
//! body length     u32, the inflated body's bytes
//! stored length   u32, then that many bytes of raw deflate; the body:
//! material count  u32
//!   name          u8 length, UTF-8 bytes
//!   texture       u16 index, or 0xFFFF for none
//!   base color    4 x f32
//!   alpha mode    u8: 0 opaque, 1 mask, 2 blend
//!   alpha cutoff  f32, zero unless the mode is mask
//!   double sided  u8: 0 or 1
//! model count     u32
//!   name          u8 length, UTF-8 bytes; names strictly increase
//!   primitives    u8 count
//!     material    u16 index
//!     colored     u8: 1 when the vertices carry colors, 0 when all are white
//!     vertices    u32 count, then the vertex planes:
//!                 position origin 3 x f32 and step 3 x f32,
//!                 texture coordinate origin 2 x f32 and step 2 x f32,
//!                 then count x u16 for each of x, y, z, u, and v (a value
//!                 is origin + step x n), count x 3 i8 normals, and, when
//!                 colored, count x 4 u8 colors
//!     indices     u32 count, then u16 each
//! character       u8: 0 for none, 1 for one
//!   name          u8 length, UTF-8 bytes
//!   joints        u16 count, parents first, each:
//!                 parent i16 (-1 for a root), rest translation 3 x f32,
//!                 rest rotation 4 x f32, rest scale 3 x f32,
//!                 inverse bind 16 x f32 (column-major)
//!   primitives    u8 count
//!     material    u16 index
//!     vertices    u32 count, then the vertex planes of an uncolored static
//!                 primitive, then count x 4 u8 joints and count x 4 u8
//!                 weights (255ths)
//!     indices     u32 count, then u16 each
//!   clips         u8 count
//!     name        u8 length, UTF-8 bytes
//!     duration    f32 seconds
//!     distance    f32 meters travelled per loop, zero for a clip on the clock
//!     tracks      u16 count
//!       joint     u16 index
//!       keys      translation, rotation, then scale: u16 count, then each
//!                 key's time f32 and 3, 4, or 3 x f32
//! forms           optional; absent when the body ends after the character
//!   count         u8, 1 to MAX_FORMS
//!   form          each a character section as above, without the flag;
//!                 names strictly increase
//! ```
//!
//! Quantizing loses at most half a step: a fraction of a millimeter for a
//! building 18 m tall, and well under a texel. Encoding a decoded pack again
//! gives the same bytes.
//!
//! Character data is in its source's space, meters with Y up and the
//! character facing +Z: a vertex is skinned by its joints' posed transforms
//! times their inverse binds.

use std::collections::BTreeSet;

/// Identifies a `VTP3` pack.
pub const MAGIC: &[u8; 8] = b"VTP3\r\n\x1a\n";
const NO_TEXTURE: u16 = u16::MAX;
/// Bytes of one uncolored vertex in the planes: five u16 and three i8.
const PLANE_VERTEX_BYTES: usize = 13;
/// The largest quantized value; positions and coordinates span 0 to this.
const STEPS: f32 = u16::MAX as f32;
const MAX_NAME_BYTES: usize = 96;
const MAX_TEXTURES: usize = 64;
const MAX_MATERIALS: usize = 256;
const MAX_MODELS: usize = 256;
const MAX_PRIMITIVES: usize = 16;
/// Most joints in a character; joint indices are bytes.
pub const MAX_JOINTS: usize = 256;
/// Most clips in a character.
pub const MAX_CLIPS: usize = 8;
/// Most forms in a pack.
pub const MAX_FORMS: usize = 8;
/// Most keys on one channel of one track.
pub const MAX_KEYS: usize = 4096;
/// The longest clip, in seconds.
pub const MAX_CLIP_SECONDS: f32 = 60.0;
/// Every admitted model fits within this distance of its origin, in meters.
pub const MAX_COORDINATE: f32 = 64.0;
/// Texture coordinates may tile; this bounds them.
pub const MAX_TEXTURE_COORDINATE: f32 = 256.0;

/// The budgets a pack is compiled and decoded under.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The largest pack transfer.
    pub pack_bytes: u64,
    /// The sum of every texture's width x height x 4.
    pub decoded_texture_bytes: u64,
    /// The longest texture edge, in pixels.
    pub texture_edge: u32,
    /// The sum of every model's triangles.
    pub triangles: u64,
    /// The triangles in any one model.
    pub model_triangles: u64,
    /// The sources and the pack together, as committed to the repository.
    pub committed_bytes: u64,
    /// The character's triangles; they also count toward `triangles`.
    pub character_triangles: u64,
    /// The inflated body: materials, models, the character, and forms.
    pub body_bytes: u64,
}

impl Limits {
    /// The Everglade budgets from `docs/verse/everglade.md`.
    pub const EVERGLADE: Self = Self {
        pack_bytes: 12 * 1024 * 1024,
        decoded_texture_bytes: 48 * 1024 * 1024,
        texture_edge: 1024,
        triangles: 480_000,
        model_triangles: 20_000,
        committed_bytes: 36_000_000,
        character_triangles: 40_000,
        body_bytes: 48 * 1024 * 1024,
    };
}

/// How a material's base-color alpha is used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlphaMode {
    /// Alpha is ignored.
    Opaque,
    /// Fragments with alpha below `cutoff` are discarded.
    Mask { cutoff: f32 },
    /// Alpha blends over what is behind, in one sorted pass.
    Blend,
}

/// A base-color material. Normal, occlusion, roughness, and emissive maps are
/// not admitted.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// `set/source-name`, unique within the pack.
    pub name: String,
    /// The base-color texture, if any.
    pub texture: Option<u16>,
    /// Linear base-color factor, multiplied with the texture and vertex color.
    pub base_color: [f32; 4],
    /// How alpha is used.
    pub alpha: AlphaMode,
    /// Whether back faces are drawn.
    pub double_sided: bool,
}

/// One vertex of a static model, in the model's meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    /// Position, Y up.
    pub position: [f32; 3],
    /// Unit normal.
    pub normal: [f32; 3],
    /// Texture coordinate set 0.
    pub uv: [f32; 2],
    /// The glTF vertex color in 8-bit linear units, multiplied with the base
    /// color. White when the source has none.
    pub color: [u8; 4],
}

/// Triangles that share one material.
#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    /// The material index.
    pub material: u16,
    /// At most 65,536 vertices.
    pub vertices: Vec<Vertex>,
    /// Counter-clockwise triangles.
    pub indices: Vec<u32>,
}

/// A static model with its node transforms applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    /// `set/source-name`, such as `nature/CommonTree_1`.
    pub name: String,
    /// One or more primitives.
    pub primitives: Vec<Primitive>,
}

impl Model {
    /// The model's triangle count.
    pub fn triangles(&self) -> u64 {
        self.primitives
            .iter()
            .map(|p| p.indices.len() as u64 / 3)
            .sum()
    }

    /// The model's axis-aligned bounds as (minimum, maximum).
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for vertex in self.primitives.iter().flat_map(|p| &p.vertices) {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.position[axis]);
                max[axis] = max[axis].max(vertex.position[axis]);
            }
        }
        (min, max)
    }
}

/// One joint of a character's skeleton, in its parent's space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint {
    /// The parent joint's index, which is lower than this joint's, or -1.
    pub parent: i16,
    /// Rest translation.
    pub translation: [f32; 3],
    /// Rest rotation as a unit quaternion, x, y, z, w.
    pub rotation: [f32; 4],
    /// Rest scale.
    pub scale: [f32; 3],
    /// Bind space to joint space, column-major.
    pub inverse_bind: [f32; 16],
}

/// One vertex of a character in its bind pose, with up to four influences.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkinnedVertex {
    /// Position, normal, and texture coordinate in the bind pose; the
    /// color is white.
    pub vertex: Vertex,
    /// Joint indices.
    pub joints: [u8; 4],
    /// Weights in 255ths.
    pub weights: [u8; 4],
}

/// Skinned triangles that share one material.
#[derive(Clone, Debug, PartialEq)]
pub struct SkinnedPrimitive {
    /// The material index.
    pub material: u16,
    /// At most 65,536 vertices.
    pub vertices: Vec<SkinnedVertex>,
    /// Counter-clockwise triangles.
    pub indices: Vec<u32>,
}

/// One joint's keys in a clip. A channel without keys keeps the rest value.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// The joint index.
    pub joint: u16,
    /// Time and translation.
    pub translation: Vec<(f32, [f32; 3])>,
    /// Time and rotation.
    pub rotation: Vec<(f32, [f32; 4])>,
    /// Time and scale.
    pub scale: Vec<(f32, [f32; 3])>,
}

/// A named, looping animation of a character's joints.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Such as `idle`, `walk`, `run`, or `jump`.
    pub name: String,
    /// Loop length, seconds.
    pub duration: f32,
    /// Meters the character travels over one loop, so a gait keeps pace
    /// with the ground; zero when the clip plays on the clock.
    pub distance: f32,
    /// Keyed joints, each at most once.
    pub tracks: Vec<Track>,
}

/// A skinned, animated character.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    /// `set/name`, such as `player/male-ranger`.
    pub name: String,
    /// The skeleton, parents before children.
    pub joints: Vec<Joint>,
    /// One or more skinned primitives.
    pub primitives: Vec<SkinnedPrimitive>,
    /// One or more clips with distinct names.
    pub clips: Vec<Clip>,
}

impl Character {
    /// The character's triangle count.
    pub fn triangles(&self) -> u64 {
        self.primitives
            .iter()
            .map(|p| p.indices.len() as u64 / 3)
            .sum()
    }

    /// The clip named `name`.
    pub fn clip(&self, name: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.name == name)
    }
}

/// A texture as it is stored: PNG bytes and their declared size.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedTexture {
    /// `set/source-name`, unique within the pack.
    pub name: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// An 8-bit gray, RGB, or RGBA PNG.
    pub png: Vec<u8>,
}

/// A decoded texture.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    /// `set/source-name`.
    pub name: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// sRGB color with straight alpha, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// The compiler's view of a pack, before encoding.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contents {
    /// Textures in first-use order.
    pub textures: Vec<EncodedTexture>,
    /// Materials in first-use order.
    pub materials: Vec<Material>,
    /// Models sorted by name.
    pub models: Vec<Model>,
    /// The player's character, if the pack carries one.
    pub character: Option<Character>,
    /// Other skinned characters the player can take the form of, sorted by
    /// name.
    pub forms: Vec<Character>,
}

/// A decoded pack, ready for the zone renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct ZonePack {
    /// Decoded textures; a material's `texture` indexes this list.
    pub textures: Vec<Texture>,
    /// Materials; a primitive's `material` indexes this list.
    pub materials: Vec<Material>,
    /// Models sorted by name.
    pub models: Vec<Model>,
    /// The player's character, if the pack carries one.
    pub character: Option<Character>,
    /// Other skinned characters the player can take the form of, sorted by
    /// name.
    pub forms: Vec<Character>,
}

impl ZonePack {
    /// Finds a model by its `set/source-name`.
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.models
            .binary_search_by(|m| m.name.as_str().cmp(name))
            .ok()
            .map(|i| &self.models[i])
    }

    /// The form named `name`, such as `beasts/giant_spider`.
    pub fn form(&self, name: &str) -> Option<&Character> {
        self.forms.iter().find(|f| f.name == name)
    }

    /// The decoded bytes of every texture.
    pub fn decoded_texture_bytes(&self) -> u64 {
        self.textures.iter().map(|t| t.rgba.len() as u64).sum()
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b'~'))
}

fn check_material(material: &Material, textures: usize) -> Result<(), String> {
    if !valid_name(&material.name) {
        return Err("Zone pack material has an invalid name".into());
    }
    if material
        .texture
        .is_some_and(|t| t == NO_TEXTURE || t as usize >= textures)
    {
        return Err("Zone pack material names a missing texture".into());
    }
    if material
        .base_color
        .iter()
        .any(|c| !c.is_finite() || !(0.0..=1.0).contains(c))
    {
        return Err("Zone pack material has an invalid base color".into());
    }
    if let AlphaMode::Mask { cutoff } = material.alpha
        && (!cutoff.is_finite() || !(0.0..=1.0).contains(&cutoff))
    {
        return Err("Zone pack material has an invalid alpha cutoff".into());
    }
    Ok(())
}

fn check_vertex(vertex: &Vertex) -> Result<(), String> {
    if vertex
        .position
        .iter()
        .any(|n| !n.is_finite() || n.abs() > MAX_COORDINATE)
    {
        return Err("Zone pack vertex has an invalid position".into());
    }
    if vertex
        .uv
        .iter()
        .any(|n| !n.is_finite() || n.abs() > MAX_TEXTURE_COORDINATE)
    {
        return Err("Zone pack vertex has an invalid texture coordinate".into());
    }
    if vertex.normal.iter().any(|n| !n.is_finite()) {
        return Err("Zone pack vertex has an invalid normal".into());
    }
    Ok(())
}

fn check_primitive(primitive: &Primitive, materials: usize) -> Result<(), String> {
    if primitive.material as usize >= materials {
        return Err("Zone pack primitive names a missing material".into());
    }
    if primitive.vertices.is_empty() || primitive.vertices.len() > u16::MAX as usize + 1 {
        return Err("Zone pack primitive has an invalid vertex count".into());
    }
    if primitive.indices.is_empty() || !primitive.indices.len().is_multiple_of(3) {
        return Err("Zone pack primitive has an invalid index count".into());
    }
    if primitive
        .indices
        .iter()
        .any(|&i| i as usize >= primitive.vertices.len())
    {
        return Err("Zone pack primitive indexes past its vertices".into());
    }
    primitive.vertices.iter().try_for_each(check_vertex)
}

/// Checks the structure and budgets the decoder enforces, so the compiler
/// cannot write a pack the loader refuses.
pub fn validate(contents: &Contents, limits: &Limits) -> Result<(), String> {
    if contents.textures.len() > MAX_TEXTURES
        || contents.materials.len() > MAX_MATERIALS
        || contents.models.is_empty()
        || contents.models.len() > MAX_MODELS
    {
        return Err("Zone pack has an invalid number of entries".into());
    }
    let mut names = BTreeSet::new();
    let mut decoded = 0u64;
    for texture in &contents.textures {
        if !valid_name(&texture.name) || !names.insert(("texture", texture.name.as_str())) {
            return Err("Zone pack texture has an invalid or repeated name".into());
        }
        if texture.width == 0
            || texture.height == 0
            || texture.width > limits.texture_edge
            || texture.height > limits.texture_edge
        {
            return Err("Zone pack texture exceeds the texture edge limit".into());
        }
        decoded += texture.width as u64 * texture.height as u64 * 4;
    }
    if decoded > limits.decoded_texture_bytes {
        return Err("Zone pack exceeds the decoded texture budget".into());
    }
    let mut used_textures = BTreeSet::new();
    for material in &contents.materials {
        check_material(material, contents.textures.len())?;
        if !names.insert(("material", material.name.as_str())) {
            return Err("Zone pack material has a repeated name".into());
        }
        used_textures.extend(material.texture);
    }
    if used_textures.len() != contents.textures.len() {
        return Err("Zone pack has a texture no material uses".into());
    }
    let mut triangles = 0u64;
    let mut previous: Option<&str> = None;
    for model in &contents.models {
        if !valid_name(&model.name) || previous.is_some_and(|p| p >= model.name.as_str()) {
            return Err("Zone pack model names must be valid and strictly increasing".into());
        }
        previous = Some(&model.name);
        if model.primitives.is_empty() || model.primitives.len() > MAX_PRIMITIVES {
            return Err("Zone pack model has an invalid primitive count".into());
        }
        for primitive in &model.primitives {
            check_primitive(primitive, contents.materials.len())?;
        }
        let count = model.triangles();
        if count > limits.model_triangles {
            return Err(format!(
                "Zone pack model {} exceeds the per-model triangle budget",
                model.name
            ));
        }
        triangles += count;
    }
    if let Some(character) = &contents.character {
        check_character(character, contents.materials.len(), limits)?;
        triangles += character.triangles();
    }
    if contents.forms.len() > MAX_FORMS {
        return Err("Zone pack has too many forms".into());
    }
    let mut previous: Option<&str> = None;
    for form in &contents.forms {
        if previous.is_some_and(|p| p >= form.name.as_str()) {
            return Err("Zone pack form names must be strictly increasing".into());
        }
        previous = Some(&form.name);
        check_character(form, contents.materials.len(), limits)?;
        triangles += form.triangles();
    }
    if triangles > limits.triangles {
        return Err(format!(
            "Zone pack exceeds the triangle budget: {triangles} of {}",
            limits.triangles
        ));
    }
    Ok(())
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|n| n.is_finite())
}

fn check_keys<const N: usize>(keys: &[(f32, [f32; N])], duration: f32) -> Result<(), String> {
    if keys.len() > MAX_KEYS {
        return Err("Zone pack track has too many keys".into());
    }
    let mut previous = 0.0;
    for (time, value) in keys {
        if !time.is_finite() || *time < previous || *time > duration || !finite(value) {
            return Err("Zone pack track has an invalid key".into());
        }
        previous = *time;
    }
    Ok(())
}

/// Checks a character's skeleton, skinned primitives, and clips.
fn check_character(character: &Character, materials: usize, limits: &Limits) -> Result<(), String> {
    if !valid_name(&character.name) {
        return Err("Zone pack character has an invalid name".into());
    }
    let joints = character.joints.len();
    if joints == 0 || joints > MAX_JOINTS {
        return Err("Zone pack character has an invalid joint count".into());
    }
    for (index, joint) in character.joints.iter().enumerate() {
        let quaternion = joint.rotation.iter().map(|n| n * n).sum::<f32>();
        if !(joint.parent == -1 || (0..index as i16).contains(&joint.parent))
            || !finite(&joint.translation)
            || !finite(&joint.scale)
            || !finite(&joint.inverse_bind)
            || !finite(&joint.rotation)
            || !(0.5..=2.0).contains(&quaternion)
        {
            return Err("Zone pack character has an invalid joint".into());
        }
    }
    if character.primitives.is_empty() || character.primitives.len() > MAX_PRIMITIVES {
        return Err("Zone pack character has an invalid primitive count".into());
    }
    for primitive in &character.primitives {
        let plain = Primitive {
            material: primitive.material,
            vertices: primitive.vertices.iter().map(|v| v.vertex).collect(),
            indices: primitive.indices.clone(),
        };
        check_primitive(&plain, materials)?;
        if primitive.vertices.iter().any(|v| {
            v.vertex.color != [255; 4]
                || v.joints.iter().any(|&j| j as usize >= joints)
                || v.weights.iter().map(|&w| w as u32).sum::<u32>() == 0
        }) {
            return Err("Zone pack character vertex has an invalid influence".into());
        }
    }
    if character.triangles() > limits.character_triangles {
        return Err("Zone pack character exceeds its triangle budget".into());
    }
    if character.clips.is_empty() || character.clips.len() > MAX_CLIPS {
        return Err("Zone pack character has an invalid clip count".into());
    }
    let mut names = BTreeSet::new();
    for clip in &character.clips {
        if !valid_name(&clip.name) || !names.insert(clip.name.as_str()) {
            return Err("Zone pack clip has an invalid or repeated name".into());
        }
        if !clip.duration.is_finite()
            || clip.duration <= 0.0
            || clip.duration > MAX_CLIP_SECONDS
            || !clip.distance.is_finite()
            || !(0.0..=MAX_COORDINATE).contains(&clip.distance)
            || clip.tracks.len() > joints
        {
            return Err("Zone pack clip has an invalid length or track count".into());
        }
        let mut keyed = BTreeSet::new();
        for track in &clip.tracks {
            if track.joint as usize >= joints || !keyed.insert(track.joint) {
                return Err("Zone pack clip has an invalid or repeated track".into());
            }
            check_keys(&track.translation, clip.duration)?;
            check_keys(&track.rotation, clip.duration)?;
            check_keys(&track.scale, clip.duration)?;
        }
    }
    Ok(())
}

/// Encodes validated contents. The same contents always give the same bytes.
pub fn encode(contents: &Contents, limits: &Limits) -> Result<Vec<u8>, String> {
    validate(contents, limits)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    put_u32(&mut out, contents.textures.len() as u32);
    for texture in &contents.textures {
        put_name(&mut out, &texture.name);
        put_u32(&mut out, texture.width);
        put_u32(&mut out, texture.height);
        let length = u32::try_from(texture.png.len()).map_err(|_| "Zone pack texture too large")?;
        put_u32(&mut out, length);
        out.extend_from_slice(&texture.png);
    }
    let body = encode_body(contents);
    if body.len() as u64 > limits.body_bytes {
        return Err("Zone pack exceeds the body budget".into());
    }
    let stored = miniz_oxide::deflate::compress_to_vec(&body, DEFLATE_LEVEL);
    put_u32(&mut out, body.len() as u32);
    let length = u32::try_from(stored.len()).map_err(|_| "Zone pack body too large")?;
    put_u32(&mut out, length);
    out.extend_from_slice(&stored);
    if out.len() as u64 > limits.pack_bytes {
        return Err("Zone pack exceeds the transfer budget".into());
    }
    Ok(out)
}

/// Deflate's strongest level; a pack is compiled once and inflated often.
const DEFLATE_LEVEL: u8 = 10;

/// The body: materials, models, the character, and forms, uncompressed.
fn encode_body(contents: &Contents) -> Vec<u8> {
    let mut out = Vec::new();
    put_u32(&mut out, contents.materials.len() as u32);
    for material in &contents.materials {
        put_name(&mut out, &material.name);
        out.extend_from_slice(&material.texture.unwrap_or(NO_TEXTURE).to_le_bytes());
        for channel in material.base_color {
            put_f32(&mut out, channel);
        }
        let (mode, cutoff) = match material.alpha {
            AlphaMode::Opaque => (0u8, 0.0),
            AlphaMode::Mask { cutoff } => (1, cutoff),
            AlphaMode::Blend => (2, 0.0),
        };
        out.push(mode);
        put_f32(&mut out, cutoff);
        out.push(u8::from(material.double_sided));
    }
    put_u32(&mut out, contents.models.len() as u32);
    for model in &contents.models {
        put_name(&mut out, &model.name);
        out.push(model.primitives.len() as u8);
        for primitive in &model.primitives {
            out.extend_from_slice(&primitive.material.to_le_bytes());
            let colored = primitive.vertices.iter().any(|v| v.color != [255; 4]);
            out.push(u8::from(colored));
            put_u32(&mut out, primitive.vertices.len() as u32);
            put_planes(&mut out, &primitive.vertices);
            if colored {
                for vertex in &primitive.vertices {
                    out.extend_from_slice(&vertex.color);
                }
            }
            put_indices(&mut out, &primitive.indices);
        }
    }
    match &contents.character {
        None => out.push(0),
        Some(character) => {
            out.push(1);
            put_character(&mut out, character);
        }
    }
    if !contents.forms.is_empty() {
        out.push(contents.forms.len() as u8);
        for form in &contents.forms {
            put_character(&mut out, form);
        }
    }
    out
}

/// Quantizes a normal component to a signed byte.
pub fn snorm8(n: f32) -> i8 {
    (n.clamp(-1.0, 1.0) * 127.0).round() as i8
}

/// A vertex's value on one quantized plane: x, y, z, u, then v.
fn plane_value(vertex: &Vertex, plane: usize) -> f32 {
    if plane < 3 {
        vertex.position[plane]
    } else {
        vertex.uv[plane - 3]
    }
}

/// The origin and step that cover `values` in [`STEPS`] steps.
fn plane_range(values: impl Iterator<Item = f32>) -> (f32, f32) {
    let (min, max) = values.fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), n| {
        (lo.min(n), hi.max(n))
    });
    if !(min.is_finite() && max.is_finite()) {
        return (0.0, 0.0);
    }
    (min, (max - min) / STEPS)
}

/// The step count nearest `value`.
fn quantize(value: f32, origin: f32, step: f32) -> u16 {
    if step > 0.0 {
        ((value - origin) / step).round().clamp(0.0, STEPS) as u16
    } else {
        0
    }
}

/// The value `n` steps from `origin`.
fn dequantize(n: u16, origin: f32, step: f32) -> f32 {
    origin + f32::from(n) * step
}

/// Writes the position and coordinate ranges, then the x, y, z, u, v, and
/// normal planes of `vertices`.
fn put_planes(out: &mut Vec<u8>, vertices: &[Vertex]) {
    let ranges: Vec<(f32, f32)> = (0..5)
        .map(|plane| plane_range(vertices.iter().map(|v| plane_value(v, plane))))
        .collect();
    for range in [&ranges[..3], &ranges[3..]] {
        for &(origin, _) in range {
            put_f32(out, origin);
        }
        for &(_, step) in range {
            put_f32(out, step);
        }
    }
    for (plane, &(origin, step)) in ranges.iter().enumerate() {
        for vertex in vertices {
            out.extend_from_slice(
                &quantize(plane_value(vertex, plane), origin, step).to_le_bytes(),
            );
        }
    }
    for vertex in vertices {
        for n in vertex.normal {
            out.push(snorm8(n) as u8);
        }
    }
}

fn put_indices(out: &mut Vec<u8>, indices: &[u32]) {
    put_u32(out, indices.len() as u32);
    for &index in indices {
        out.extend_from_slice(&(index as u16).to_le_bytes());
    }
}

fn put_floats(out: &mut Vec<u8>, values: &[f32]) {
    for &n in values {
        put_f32(out, n);
    }
}

fn put_keys<const N: usize>(out: &mut Vec<u8>, keys: &[(f32, [f32; N])]) {
    // `validate` bounds keys to MAX_KEYS, below u16::MAX.
    out.extend_from_slice(&(keys.len() as u16).to_le_bytes());
    for (time, value) in keys {
        put_f32(out, *time);
        put_floats(out, value);
    }
}

fn put_character(out: &mut Vec<u8>, character: &Character) {
    // `validate` bounds every count below its field's width.
    put_name(out, &character.name);
    out.extend_from_slice(&(character.joints.len() as u16).to_le_bytes());
    for joint in &character.joints {
        out.extend_from_slice(&joint.parent.to_le_bytes());
        put_floats(out, &joint.translation);
        put_floats(out, &joint.rotation);
        put_floats(out, &joint.scale);
        put_floats(out, &joint.inverse_bind);
    }
    out.push(character.primitives.len() as u8);
    for primitive in &character.primitives {
        out.extend_from_slice(&primitive.material.to_le_bytes());
        put_u32(out, primitive.vertices.len() as u32);
        // A skinned vertex's color is always white and is not stored.
        let plain: Vec<Vertex> = primitive.vertices.iter().map(|v| v.vertex).collect();
        put_planes(out, &plain);
        for vertex in &primitive.vertices {
            out.extend_from_slice(&vertex.joints);
        }
        for vertex in &primitive.vertices {
            out.extend_from_slice(&vertex.weights);
        }
        put_indices(out, &primitive.indices);
    }
    out.push(character.clips.len() as u8);
    for clip in &character.clips {
        put_name(out, &clip.name);
        put_f32(out, clip.duration);
        put_f32(out, clip.distance);
        out.extend_from_slice(&(clip.tracks.len() as u16).to_le_bytes());
        for track in &clip.tracks {
            out.extend_from_slice(&track.joint.to_le_bytes());
            put_keys(out, &track.translation);
            put_keys(out, &track.rotation);
            put_keys(out, &track.scale);
        }
    }
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_name(out: &mut Vec<u8>, name: &str) {
    // `validate` bounds names to MAX_NAME_BYTES, below u8::MAX.
    out.push(name.len() as u8);
    out.extend_from_slice(name.as_bytes());
}

/// Decodes the pack structure and keeps textures encoded. It does not check
/// the pinned digest; the loader does that first.
pub fn decode_contents(bytes: &[u8], limits: &Limits) -> Result<Contents, String> {
    if bytes.len() as u64 > limits.pack_bytes {
        return Err("Zone pack exceeds the transfer limit".into());
    }
    let mut reader = Reader { bytes, offset: 0 };
    if reader.take(MAGIC.len())? != MAGIC {
        return Err("Zone pack has an unsupported format".into());
    }
    let mut contents = Contents::default();
    let count = reader.count(MAX_TEXTURES)?;
    let mut decoded = 0u64;
    for _ in 0..count {
        let name = reader.name()?;
        let width = reader.u32()?;
        let height = reader.u32()?;
        if width == 0 || height == 0 || width > limits.texture_edge || height > limits.texture_edge
        {
            return Err("Zone pack texture exceeds the texture edge limit".into());
        }
        // Reserve the decoded budget from declared sizes before any decoding.
        decoded += width as u64 * height as u64 * 4;
        if decoded > limits.decoded_texture_bytes {
            return Err("Zone pack exceeds the decoded texture budget".into());
        }
        let length = reader.u32()? as usize;
        let png = reader.take(length)?.to_vec();
        contents.textures.push(EncodedTexture {
            name,
            width,
            height,
            png,
        });
    }
    let body_length = reader.u32()? as u64;
    if body_length > limits.body_bytes {
        return Err("Zone pack exceeds the body budget".into());
    }
    let length = reader.u32()? as usize;
    let stored = reader.take(length)?;
    if reader.offset != bytes.len() {
        return Err("Zone pack has trailing data".into());
    }
    // The limit bounds the allocation by the declared, budgeted length.
    let body = miniz_oxide::inflate::decompress_to_vec_with_limit(stored, body_length as usize)
        .map_err(|_| "Zone pack body could not be inflated")?;
    if body.len() as u64 != body_length {
        return Err("Zone pack body has an unexpected length".into());
    }
    let mut reader = Reader {
        bytes: &body,
        offset: 0,
    };
    let count = reader.count(MAX_MATERIALS)?;
    for _ in 0..count {
        let name = reader.name()?;
        let texture = match reader.u16()? {
            NO_TEXTURE => None,
            index => Some(index),
        };
        let base_color = [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?];
        let mode = reader.u8()?;
        let cutoff = reader.f32()?;
        let alpha = match mode {
            0 if cutoff == 0.0 => AlphaMode::Opaque,
            1 => AlphaMode::Mask { cutoff },
            2 if cutoff == 0.0 => AlphaMode::Blend,
            _ => return Err("Zone pack material has an invalid alpha mode".into()),
        };
        let double_sided = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return Err("Zone pack material has an invalid face flag".into()),
        };
        let material = Material {
            name,
            texture,
            base_color,
            alpha,
            double_sided,
        };
        check_material(&material, contents.textures.len())?;
        contents.materials.push(material);
    }
    let count = reader.count(MAX_MODELS)?;
    let mut triangles = 0u64;
    for _ in 0..count {
        let name = reader.name()?;
        let primitive_count = reader.u8()? as usize;
        if primitive_count == 0 || primitive_count > MAX_PRIMITIVES {
            return Err("Zone pack model has an invalid primitive count".into());
        }
        let mut primitives = Vec::with_capacity(primitive_count);
        let mut model_triangles = 0u64;
        for _ in 0..primitive_count {
            let material = reader.u16()?;
            let colored = match reader.u8()? {
                0 => false,
                1 => true,
                _ => return Err("Zone pack primitive has an invalid color flag".into()),
            };
            let vertex_count = reader.vertex_count()?;
            let mut vertices = reader.planes(vertex_count)?;
            if colored {
                let colors = reader.take(vertex_count * 4)?;
                for (vertex, color) in vertices.iter_mut().zip(colors.chunks_exact(4)) {
                    vertex.color = [color[0], color[1], color[2], color[3]];
                }
            }
            let indices = reader.indices(
                "Zone pack model exceeds the per-model triangle budget",
                |count| {
                    model_triangles += count as u64 / 3;
                    model_triangles <= limits.model_triangles
                },
            )?;
            primitives.push(Primitive {
                material,
                vertices,
                indices,
            });
        }
        triangles += model_triangles;
        if triangles > limits.triangles {
            return Err("Zone pack exceeds the triangle budget".into());
        }
        contents.models.push(Model { name, primitives });
    }
    contents.character = match reader.u8()? {
        0 => None,
        1 => Some(reader.character(limits)?),
        _ => return Err("Zone pack has an invalid character flag".into()),
    };
    if reader.offset < body.len() {
        let count = reader.u8()? as usize;
        if count == 0 || count > MAX_FORMS {
            return Err("Zone pack has an invalid form count".into());
        }
        for _ in 0..count {
            contents.forms.push(reader.character(limits)?);
        }
    }
    if reader.offset != body.len() {
        return Err("Zone pack has trailing data".into());
    }
    validate(&contents, limits)?;
    Ok(contents)
}

/// Decodes the pack structure, then every texture, within `limits`.
pub fn decode(bytes: &[u8], limits: &Limits) -> Result<ZonePack, String> {
    let contents = decode_contents(bytes, limits)?;
    let textures = contents
        .textures
        .iter()
        .map(decode_texture)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ZonePack {
        textures,
        materials: contents.materials,
        models: contents.models,
        character: contents.character,
        forms: contents.forms,
    })
}

/// Decodes one stored texture and checks it against its declared size.
pub fn decode_texture(texture: &EncodedTexture) -> Result<Texture, String> {
    let expected = texture.width as usize * texture.height as usize * 4;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(texture.png.as_slice()));
    // The declared size was already charged to the decoded budget; the
    // workspace limit keeps a lying header from allocating more.
    decoder.set_limits(png::Limits {
        bytes: expected + 64 * 1024,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|_| "Zone pack texture is not a readable PNG")?;
    let info = reader.info();
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err("Zone pack texture must be gray, RGB, or RGBA".into()),
    };
    if info.width != texture.width
        || info.height != texture.height
        || info.bit_depth != png::BitDepth::Eight
        || info.animation_control.is_some()
    {
        return Err("Zone pack texture does not match its declared format".into());
    }
    let size = texture.width as usize * texture.height as usize * channels;
    if reader.output_buffer_size() != Some(size) {
        return Err("Zone pack texture has an invalid decoded size".into());
    }
    let mut pixels = vec![0; size];
    let output = reader
        .next_frame(&mut pixels)
        .map_err(|_| "Zone pack texture could not be decoded")?;
    if output.buffer_size() != size {
        return Err("Zone pack texture is incomplete".into());
    }
    let rgba = if channels == 4 {
        pixels
    } else {
        let mut rgba = Vec::with_capacity(expected);
        for p in pixels.chunks_exact(channels) {
            let [r, g, b] = if channels == 3 {
                [p[0], p[1], p[2]]
            } else {
                [p[0]; 3]
            };
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        rgba
    };
    Ok(Texture {
        name: texture.name.clone(),
        width: texture.width,
        height: texture.height,
        rgba,
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("Zone pack size overflow")?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or("Zone pack is truncated")?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let v = self.take(2)?;
        Ok(u16::from_le_bytes([v[0], v[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let v = self.take(4)?;
        Ok(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }

    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn floats<const N: usize>(&mut self) -> Result<[f32; N], String> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Ok(values)
    }

    /// A u16 count within `max`.
    fn short(&mut self, max: usize) -> Result<usize, String> {
        let count = self.u16()? as usize;
        if count > max {
            return Err("Zone pack has too many entries".into());
        }
        Ok(count)
    }

    fn keys<const N: usize>(&mut self) -> Result<Vec<(f32, [f32; N])>, String> {
        let count = self.short(MAX_KEYS)?;
        // Bounds the allocation by bytes actually present in the pack.
        let present = (self.bytes.len() - self.offset) / ((N + 1) * 4);
        if count > present {
            return Err("Zone pack is truncated".into());
        }
        let mut keys = Vec::with_capacity(count);
        for _ in 0..count {
            keys.push((self.f32()?, self.floats()?));
        }
        Ok(keys)
    }

    /// A primitive's vertex count, 1 to 65,536.
    fn vertex_count(&mut self) -> Result<usize, String> {
        let count = self.u32()? as usize;
        if count == 0 || count > u16::MAX as usize + 1 {
            return Err("Zone pack primitive has an invalid vertex count".into());
        }
        Ok(count)
    }

    /// Reads `count` vertices' ranges and planes, white and uncolored.
    fn planes(&mut self, count: usize) -> Result<Vec<Vertex>, String> {
        let ranges: [f32; 10] = self.floats()?;
        if ranges.iter().any(|n| !n.is_finite()) || [3, 4, 5, 8, 9].iter().any(|&i| ranges[i] < 0.0)
        {
            return Err("Zone pack primitive has an invalid vertex range".into());
        }
        // Origins and steps: x, y, z, then u, v.
        let origin = [ranges[0], ranges[1], ranges[2], ranges[6], ranges[7]];
        let step = [ranges[3], ranges[4], ranges[5], ranges[8], ranges[9]];
        // Bounds the allocation by bytes actually present in the pack.
        let planes = self.take(count * PLANE_VERTEX_BYTES)?;
        let value = |plane: usize, i: usize| {
            let at = (plane * count + i) * 2;
            dequantize(
                u16::from_le_bytes([planes[at], planes[at + 1]]),
                origin[plane],
                step[plane],
            )
        };
        let normals = &planes[count * 10..];
        let normal = |n: u8| (n as i8) as f32 / 127.0;
        Ok((0..count)
            .map(|i| Vertex {
                position: [value(0, i), value(1, i), value(2, i)],
                normal: [
                    normal(normals[i * 3]),
                    normal(normals[i * 3 + 1]),
                    normal(normals[i * 3 + 2]),
                ],
                uv: [value(3, i), value(4, i)],
                color: [255; 4],
            })
            .collect())
    }

    /// Reads a primitive's indices after `admit` accepts their count, or
    /// refuses them with `refusal`.
    fn indices(
        &mut self,
        refusal: &str,
        mut admit: impl FnMut(usize) -> bool,
    ) -> Result<Vec<u32>, String> {
        let count = self.u32()? as usize;
        if count == 0 || !count.is_multiple_of(3) {
            return Err("Zone pack primitive has an invalid index count".into());
        }
        if !admit(count) {
            return Err(refusal.into());
        }
        let packed = self.take(count * 2)?;
        Ok(packed
            .chunks_exact(2)
            .map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])))
            .collect())
    }

    /// Reads the character section; `validate` checks it afterwards.
    fn character(&mut self, limits: &Limits) -> Result<Character, String> {
        let name = self.name()?;
        let count = self.short(MAX_JOINTS)?;
        let mut joints = Vec::with_capacity(count.min(MAX_JOINTS));
        for _ in 0..count {
            let parent = self.u16()? as i16;
            joints.push(Joint {
                parent,
                translation: self.floats()?,
                rotation: self.floats()?,
                scale: self.floats()?,
                inverse_bind: self.floats()?,
            });
        }
        let count = self.u8()? as usize;
        if count == 0 || count > MAX_PRIMITIVES {
            return Err("Zone pack character has an invalid primitive count".into());
        }
        let mut primitives = Vec::with_capacity(count);
        let mut triangles = 0u64;
        for _ in 0..count {
            let material = self.u16()?;
            let vertex_count = self.vertex_count()?;
            let planes = self.planes(vertex_count)?;
            let joints = self.take(vertex_count * 4)?;
            let weights = self.take(vertex_count * 4)?;
            let vertices = planes
                .into_iter()
                .zip(joints.chunks_exact(4).zip(weights.chunks_exact(4)))
                .map(|(vertex, (j, w))| SkinnedVertex {
                    vertex,
                    joints: [j[0], j[1], j[2], j[3]],
                    weights: [w[0], w[1], w[2], w[3]],
                })
                .collect();
            let indices =
                self.indices("Zone pack character exceeds its triangle budget", |count| {
                    triangles += count as u64 / 3;
                    triangles <= limits.character_triangles
                })?;
            primitives.push(SkinnedPrimitive {
                material,
                vertices,
                indices,
            });
        }
        let count = self.u8()? as usize;
        if count == 0 || count > MAX_CLIPS {
            return Err("Zone pack character has an invalid clip count".into());
        }
        let mut clips = Vec::with_capacity(count);
        for _ in 0..count {
            let name = self.name()?;
            let duration = self.f32()?;
            let distance = self.f32()?;
            let tracks_count = self.short(MAX_JOINTS)?;
            let mut tracks = Vec::with_capacity(tracks_count);
            for _ in 0..tracks_count {
                tracks.push(Track {
                    joint: self.u16()?,
                    translation: self.keys()?,
                    rotation: self.keys()?,
                    scale: self.keys()?,
                });
            }
            clips.push(Clip {
                name,
                duration,
                distance,
                tracks,
            });
        }
        Ok(Character {
            name,
            joints,
            primitives,
            clips,
        })
    }

    fn count(&mut self, max: usize) -> Result<usize, String> {
        let count = self.u32()? as usize;
        if count > max {
            return Err("Zone pack has too many entries".into());
        }
        Ok(count)
    }

    fn name(&mut self) -> Result<String, String> {
        let length = self.u8()? as usize;
        let bytes = self.take(length)?;
        let name = std::str::from_utf8(bytes).map_err(|_| "Zone pack name is not UTF-8")?;
        if !valid_name(name) {
            return Err("Zone pack has an invalid name".into());
        }
        Ok(name.to_owned())
    }
}
