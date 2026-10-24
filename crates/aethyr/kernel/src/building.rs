use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Opening {
    pub room_id: String,
    pub side: String,
    pub center: f32,
    pub width: f32,
    pub bottom: f32,
    pub height: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Support {
    pub id: String,
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub floor_y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ramp {
    pub id: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub width: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildingPlan {
    pub rooms: Vec<Room>,
    #[serde(default)]
    pub openings: Vec<Opening>,
    #[serde(default)]
    pub supports: Vec<Support>,
    #[serde(default)]
    pub ramps: Vec<Ramp>,
}

impl BuildingPlan {
    pub fn normalize(&mut self) {
        for room in &mut self.rooms {
            if room.floor_y.is_nan() {
                room.floor_y = 0.0;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default = "one")]
    pub scale: f32,
}

impl Transform {
    pub fn local(&self, world: [f32; 3]) -> [f32; 3] {
        let dx = (world[0] - self.position[0]) / self.scale;
        let dz = (world[2] - self.position[2]) / self.scale;
        let (s, c) = self.yaw.sin_cos();
        [
            c * dx - s * dz,
            (world[1] - self.position[1]) / self.scale,
            s * dx + c * dz,
        ]
    }
    fn world(&self, local: [f32; 3]) -> [f32; 3] {
        let (s, c) = self.yaw.sin_cos();
        [
            self.position[0] + self.scale * (c * local[0] + s * local[2]),
            self.position[1] + self.scale * local[1],
            self.position[2] + self.scale * (-s * local[0] + c * local[2]),
        ]
    }
    fn bounds(&self, min: [f32; 3], max: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for x in [min[0], max[0]] {
            for y in [min[1], max[1]] {
                for z in [min[2], max[2]] {
                    let p = self.world([x, y, z]);
                    for i in 0..3 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                }
            }
        }
        (lo, hi)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Building {
    pub id: String,
    pub position: [f32; 3],
    #[serde(default)]
    pub yaw: f32,
    #[serde(default = "one")]
    pub scale: f32,
    pub plan: BuildingPlan,
}

#[derive(Clone)]
struct Instance {
    transform: Transform,
    solids: Vec<Collider>,
    supports: Vec<Support>,
    ramps: Vec<Ramp>,
}

#[derive(Clone, Default)]
pub struct Geometry {
    instances: Vec<Instance>,
    pub walls: Vec<WallCollider>,
    pub supports: Vec<Value>,
    pub ramps: Vec<Value>,
}

fn rect_cut(rects: Vec<[f32; 4]>, cut: [f32; 4]) -> Vec<[f32; 4]> {
    let mut out = vec![];
    for [a, b, c, d] in rects {
        let x0 = a.max(cut[0]);
        let x1 = b.min(cut[1]);
        let y0 = c.max(cut[2]);
        let y1 = d.min(cut[3]);
        if x1 <= x0 + EPS || y1 <= y0 + EPS {
            out.push([a, b, c, d]);
            continue;
        }
        for r in [
            [a, x0, c, d],
            [x1, b, c, d],
            [x0, x1, c, y0],
            [x0, x1, y1, d],
        ] {
            if r[1] - r[0] > EPS && r[3] - r[2] > EPS {
                out.push(r);
            }
        }
    }
    out
}

fn ramp_height(r: &Ramp, p: [f32; 3], margin: f32) -> Option<f32> {
    let dx = r.to[0] - r.from[0];
    let dz = r.to[2] - r.from[2];
    let len2 = dx * dx + dz * dz;
    let len = len2.sqrt();
    let x = p[0] - r.from[0];
    let z = p[2] - r.from[2];
    let t = (x * dx + z * dz) / len2;
    let across = (x * dz - z * dx).abs() / len;
    if across > r.width * 0.5 + margin || t < -margin / len || t > 1.0 + margin / len {
        return None;
    }
    Some(r.from[1] + (r.to[1] - r.from[1]) * t.clamp(0.0, 1.0))
}

pub fn compile(world: &World) -> Result<Geometry, String> {
    let mut buildings = world.buildings.clone();
    for e in &world.entities {
        if let Some(plan) = &e.building_plan {
            buildings.push(Building {
                id: e.id.clone(),
                position: e.position,
                yaw: e.yaw,
                scale: e.scale,
                plan: plan.clone(),
            });
        }
    }
    if buildings.len() > 128 {
        return Err("This preview supports up to 128 editable buildings.".into());
    }
    let mut ids = HashSet::new();
    let mut geometry = Geometry::default();
    for b in buildings {
        if !valid_id(&b.id)
            || !ids.insert(b.id.clone())
            || !finite(&b.position)
            || !b.yaw.is_finite()
            || !(0.05..=20.0).contains(&b.scale)
        {
            return Err("An editable building has an invalid transform or name.".into());
        }
        let p = &b.plan;
        if p.rooms.is_empty()
            || p.rooms.len() > 128
            || p.openings.len() > 512
            || p.supports.len() > 512
            || p.ramps.len() > 64
        {
            return Err("That building plan exceeds the preview limits.".into());
        }
        let transform = Transform {
            position: b.position,
            yaw: b.yaw,
            scale: b.scale,
        };
        let mut room_ids = HashSet::new();
        for (i, r) in p.rooms.iter().enumerate() {
            if !valid_id(&r.id)
                || !room_ids.insert(r.id.clone())
                || !finite(&r.center)
                || !finite(&r.size)
                || !finite(&[r.floor_y, r.height])
                || r.size.iter().any(|v| !(2.0..=32.0).contains(v))
                || !(2.0..=12.0).contains(&r.height)
            {
                return Err("An editable building has an invalid room.".into());
            }
            for o in &p.rooms[..i] {
                if (r.floor_y - o.floor_y).abs() < EPS
                    && (r.center[0] - o.center[0]).abs() < (r.size[0] + o.size[0]) / 2.0 - EPS
                    && (r.center[1] - o.center[1]).abs() < (r.size[1] + o.size[1]) / 2.0 - EPS
                {
                    return Err("Building rooms cannot overlap on the same floor.".into());
                }
            }
        }
        for o in &p.openings {
            let Some(r) = p.rooms.iter().find(|r| r.id == o.room_id) else {
                return Err("An opening refers to a missing room.".into());
            };
            let Some(side) = SIDES.iter().position(|s| *s == o.side) else {
                return Err("An opening needs a valid wall side.".into());
            };
            let (_, lo, hi, _) = bounds(r, side);
            if !finite(&[o.center, o.width, o.bottom, o.height])
                || o.width <= 0.0
                || o.height <= 0.0
                || o.bottom < 0.0
                || o.center - o.width / 2.0 < lo - EPS
                || o.center + o.width / 2.0 > hi + EPS
                || o.bottom + o.height > r.height + EPS
            {
                return Err("An opening must fit inside its wall.".into());
            }
        }
        let mut instance = Instance {
            transform: transform.clone(),
            solids: vec![],
            supports: p.supports.clone(),
            ramps: p.ramps.clone(),
        };
        for (i, r) in p.rooms.iter().enumerate() {
            for side in 0..4 {
                let id = format!("{}:{}:{}", b.id, r.id, SIDES[side]);
                if world.removed.contains(&id) {
                    continue;
                }
                let (coord, lo, hi, horizontal) = bounds(r, side);
                let mut rects = vec![[lo, hi, r.floor_y, r.floor_y + r.height]];
                for (j, other) in p.rooms.iter().enumerate() {
                    if j >= i || (other.floor_y - r.floor_y).abs() > EPS {
                        continue;
                    }
                    let (oc, a, z, _) = bounds(other, (side + 2) % 4);
                    if (coord - oc).abs() < EPS {
                        rects = rect_cut(rects, [a, z, r.floor_y, r.floor_y + r.height]);
                    }
                }
                for opening in &p.openings {
                    let other = p
                        .rooms
                        .iter()
                        .find(|o| o.id == opening.room_id)
                        .expect("validated opening");
                    let os = SIDES
                        .iter()
                        .position(|s| *s == opening.side)
                        .expect("validated side");
                    let (oc, _, _, _) = bounds(other, os);
                    let matching = other.id == r.id && os == side
                        || other.id != r.id
                            && os == (side + 2) % 4
                            && (oc - coord).abs() < EPS
                            && (other.floor_y - r.floor_y).abs() < EPS;
                    if matching {
                        rects = rect_cut(
                            rects,
                            [
                                opening.center - opening.width / 2.0,
                                opening.center + opening.width / 2.0,
                                other.floor_y + opening.bottom,
                                other.floor_y + opening.bottom + opening.height,
                            ],
                        );
                    }
                }
                for [a, z, y0, y1] in rects {
                    let (min, max) = if horizontal {
                        ([a, y0, coord - 0.09], [z, y1, coord + 0.09])
                    } else {
                        ([coord - 0.09, y0, a], [coord + 0.09, y1, z])
                    };
                    instance.solids.push(Collider {
                        id: id.clone(),
                        min,
                        max,
                    });
                    let (world_min, world_max) = transform.bounds(min, max);
                    geometry.walls.push(WallCollider {
                        id: id.clone(),
                        room_id: format!("{}:{}", b.id, r.id),
                        side: SIDES[side].into(),
                        min: world_min,
                        max: world_max,
                        local_min: Some(min),
                        local_max: Some(max),
                        transform: Some(transform.clone()),
                    });
                }
            }
        }
        let mut support_ids = HashSet::new();
        for s in &p.supports {
            if !valid_id(&s.id)
                || !support_ids.insert(s.id.clone())
                || !finite(&s.center)
                || !finite(&s.size)
                || !s.floor_y.is_finite()
                || s.size.iter().any(|v| *v <= 0.0 || *v > 128.0)
            {
                return Err("A building floor has invalid dimensions.".into());
            }
            instance.solids.push(Collider {
                id: format!("{}:{}", b.id, s.id),
                min: [
                    s.center[0] - s.size[0] / 2.0,
                    s.floor_y - 0.12,
                    s.center[1] - s.size[1] / 2.0,
                ],
                max: [
                    s.center[0] + s.size[0] / 2.0,
                    s.floor_y,
                    s.center[1] + s.size[1] / 2.0,
                ],
            });
            geometry.supports.push(json!({"id":format!("{}:{}",b.id,s.id),"center":s.center,"size":s.size,"floorY":s.floor_y,"transform":transform}));
        }
        let mut ramp_ids = HashSet::new();
        for r in &p.ramps {
            let length = ((r.to[0] - r.from[0]).powi(2) + (r.to[2] - r.from[2]).powi(2)).sqrt();
            if !valid_id(&r.id)
                || !ramp_ids.insert(r.id.clone())
                || !finite(&r.from)
                || !finite(&r.to)
                || !r.width.is_finite()
                || r.width <= 0.0
                || r.width > 32.0
                || length < EPS
                || (r.to[1] - r.from[1]).abs() > length
            {
                return Err("A stair ramp has invalid dimensions or is too steep.".into());
            }
            geometry.ramps.push(json!({"id":format!("{}:{}",b.id,r.id),"from":r.from,"to":r.to,"width":r.width,"transform":transform}));
        }
        geometry.instances.push(instance);
    }
    Ok(geometry)
}

impl Geometry {
    pub fn camera_sweep(&self, from: [f32; 3], to: [f32; 3], radius: f32) -> f32 {
        let mut fraction = 1.0f32;
        for instance in &self.instances {
            let a = instance.transform.local(from);
            let b = instance.transform.local(to);
            let r = radius / instance.transform.scale;
            for collider in &instance.solids {
                fraction = fraction.min(super::sweep_box(a, b, r, collider.min, collider.max));
            }
        }
        fraction
    }
    pub fn can_occupy(&self, position: [f32; 3]) -> bool {
        self.can_occupy_height(position, HEIGHT)
    }
    pub fn can_occupy_height(&self, position: [f32; 3], capsule_height: f32) -> bool {
        for instance in &self.instances {
            let p = instance.transform.local(position);
            let radius = RADIUS / instance.transform.scale;
            let height = capsule_height / instance.transform.scale;
            for c in &instance.solids {
                if p[1] + height <= c.min[1] + EPS || p[1] >= c.max[1] - EPS {
                    continue;
                }
                let dx = p[0] - p[0].clamp(c.min[0], c.max[0]);
                let dz = p[2] - p[2].clamp(c.min[2], c.max[2]);
                if dx * dx + dz * dz < radius * radius - EPS * EPS {
                    return false;
                }
            }
            for r in &instance.ramps {
                if let Some(y) = ramp_height(r, p, radius) {
                    if p[1] < y - 0.02 && p[1] + height > y - 0.12 {
                        return false;
                    }
                }
            }
        }
        true
    }
    pub fn support_height(&self, position: [f32; 3], max_y: f32) -> Option<f32> {
        let mut result: Option<f32> = None;
        for instance in &self.instances {
            let p = instance.transform.local(position);
            for s in &instance.supports {
                let dx = ((p[0] - s.center[0]).abs() - s.size[0] / 2.0).max(0.0);
                let dz = ((p[2] - s.center[1]).abs() - s.size[1] / 2.0).max(0.0);
                let radius = RADIUS / instance.transform.scale;
                if dx * dx + dz * dz <= radius * radius + EPS {
                    let y = instance.transform.position[1] + s.floor_y * instance.transform.scale;
                    if y <= max_y + EPS {
                        result = Some(result.map_or(y, |old| old.max(y)));
                    }
                }
            }
            for r in &instance.ramps {
                if let Some(y) = ramp_height(r, p, 0.0) {
                    let y = instance.transform.position[1] + y * instance.transform.scale;
                    if y <= max_y + EPS {
                        result = Some(result.map_or(y, |old| old.max(y)));
                    }
                }
            }
        }
        result
    }
}
