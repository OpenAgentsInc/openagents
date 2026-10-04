//! Shared authored chamber solids for authority and presentation.
use glam::Vec3;
pub struct RoomBox {
    pub center: Vec3,
    pub half: Vec3,
    pub color: [f32; 3],
    pub emissive: bool,
}
pub fn room_boxes() -> Vec<RoomBox> {
    let mut boxes = vec![];
    let mut add = |center, half, color, emissive| {
        boxes.push(RoomBox {
            center,
            half,
            color,
            emissive,
        })
    };
    add(
        Vec3::new(0., -0.3, -7.),
        Vec3::new(22., 0.3, 30.),
        [0.19, 0.21, 0.24],
        false,
    );
    for x in [-22., 22.] {
        add(
            Vec3::new(x, 6., -7.),
            Vec3::new(0.5, 6., 30.),
            [0.12, 0.14, 0.18],
            false,
        );
    }
    for z in [-37., 17.] {
        add(
            Vec3::new(0., 6., z),
            Vec3::new(22., 6., 0.5),
            [0.12, 0.14, 0.18],
            false,
        );
    }
    for x in [-15., 15.] {
        for z in [-25., -13., 0., 12.] {
            add(
                Vec3::new(x, 5., z),
                Vec3::new(0.7, 5., 0.7),
                [0.23, 0.24, 0.27],
                false,
            );
            add(
                Vec3::new(x, 2.8, z),
                Vec3::new(0.9, 0.2, 0.9),
                [0.2, 0.24, 0.2],
                false,
            );
            add(
                Vec3::new(x, 3.15, z),
                Vec3::new(0.3, 0.3, 0.3),
                [0.07, 0.8, 0.18],
                true,
            );
        }
    }
    boxes
}
/// Compiles collision boxes from the same authored solids as the room mesh.
pub fn colliders() -> Vec<physics::kinematic::Aabb> {
    room_boxes()
        .into_iter()
        .filter(|b| !b.emissive)
        .map(|b| physics::kinematic::Aabb {
            min: (b.center - b.half).as_dvec3(),
            max: (b.center + b.half).as_dvec3(),
        })
        .collect()
}
