//! MuJoCo 3.3.7 through checked, owner-configured native libraries.
//! The model, data, input snapshot, and library have one explicit lifetime.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString, c_char, c_int};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use glam::{DQuat, DVec3};
use libloading::Library;
use robot_lab::document::{Build, Contact, ObjectState, Pose, Scene, Sensor, Shape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::runner::{Backend, Snapshot};
use crate::{Error, Result, require};

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]
#[path = "mujoco_ffi.rs"]
mod ffi;

const VERSION: i32 = 337;
const MAX_INPUT_BYTES: u64 = 64 << 20;
static COMPILER: Mutex<()> = Mutex::new(());

type LoadXml =
    unsafe extern "C" fn(*const c_char, *const ffi::mjVFS, *mut c_char, c_int) -> *mut ffi::mjModel;
type MakeData = unsafe extern "C" fn(*const ffi::mjModel) -> *mut ffi::mjData;
type DeleteModel = unsafe extern "C" fn(*mut ffi::mjModel);
type DeleteData = unsafe extern "C" fn(*mut ffi::mjData);
type StepFn = unsafe extern "C" fn(*const ffi::mjModel, *mut ffi::mjData);
type NameId = unsafe extern "C" fn(*const ffi::mjModel, c_int, *const c_char) -> c_int;
type ContactForce = unsafe extern "C" fn(*const ffi::mjModel, *const ffi::mjData, c_int, *mut f64);
type ObjectVelocity =
    unsafe extern "C" fn(*const ffi::mjModel, *const ffi::mjData, c_int, c_int, *mut f64, c_int);
type SaveXml =
    unsafe extern "C" fn(*const c_char, *const ffi::mjModel, *mut c_char, c_int) -> c_int;
type IdName = unsafe extern "C" fn(*const ffi::mjModel, c_int, c_int) -> *const c_char;

struct Api {
    _library: Library,
    load: LoadXml,
    make: MakeData,
    delete_model: DeleteModel,
    delete_data: DeleteData,
    step: StepFn,
    reset: StepFn,
    forward: StepFn,
    name: NameId,
    contact_force: ContactForce,
    velocity: ObjectVelocity,
    save: SaveXml,
    free_xml: unsafe extern "C" fn(),
    id_name: IdName,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibraryPin {
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Clone)]
pub struct Runtime {
    api: Arc<Api>,
    sha256: String,
    scratch: PathBuf,
}

impl Runtime {
    /// An operator chooses and hashes the library. Chat documents cannot
    /// select a native library. Version validation precedes every struct read.
    pub fn open(pin: &LibraryPin, scratch: &Path) -> Result<Self> {
        require(
            scratch.is_absolute(),
            "Choose an absolute private simulation folder.",
        )?;
        fs::create_dir_all(scratch)?;
        let scratch = fs::canonicalize(scratch)?;
        require(
            !scratch.ancestors().any(|p| p.join(".git").exists()),
            "Choose a simulation folder outside the Git checkout.",
        )?;
        require(
            // macOS arm64: bindings regenerated from the official 3.3.7
            // macOS headers are byte-identical, and the native tests pass.
            (cfg!(target_os = "linux") && cfg!(target_arch = "x86_64"))
                || (cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")),
            "Run MuJoCo on a Linux x86-64 or Apple silicon Mac host with this version of Robot Lab.",
        )?;
        require(
            pin.sha256.len() == 64 && pin.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "Configure an exact MuJoCo library file.",
        )?;
        let path = fs::canonicalize(&pin.path)?;
        let sha256 = file_hash(&path, 64 << 20)?;
        require(
            sha256 == pin.sha256,
            "The MuJoCo library has changed. Update its configured file hash.",
        )?;
        // SAFETY: only the operator's explicitly pinned native library is
        // loaded. Its ABI version is checked before accessing native objects.
        let library = unsafe { Library::new(path) }.map_err(|e| Error::Library(e.to_string()))?;
        macro_rules! symbol {
            ($name:literal, $type:ty) => {{
                // SAFETY: MuJoCo's versioned C API declares this exact signature;
                // the owning Api retains the library until all objects drop.
                unsafe {
                    *library
                        .get::<$type>(concat!($name, "\0").as_bytes())
                        .map_err(|e| Error::Library(e.to_string()))?
                }
            }};
        }
        let version = symbol!("mj_version", unsafe extern "C" fn() -> c_int);
        // SAFETY: a zero-argument version query accesses no foreign structures.
        require(
            unsafe { version() } == VERSION,
            "Use MuJoCo 3.3.7 with this backend.",
        )?;
        let api = Api {
            load: symbol!("mj_loadXML", LoadXml),
            make: symbol!("mj_makeData", MakeData),
            delete_model: symbol!("mj_deleteModel", DeleteModel),
            delete_data: symbol!("mj_deleteData", DeleteData),
            step: symbol!("mj_step", StepFn),
            reset: symbol!("mj_resetData", StepFn),
            forward: symbol!("mj_forward", StepFn),
            name: symbol!("mj_name2id", NameId),
            contact_force: symbol!("mj_contactForce", ContactForce),
            velocity: symbol!("mj_objectVelocity", ObjectVelocity),
            save: symbol!("mj_saveLastXML", SaveXml),
            free_xml: symbol!("mj_freeLastXML", unsafe extern "C" fn()),
            id_name: symbol!("mj_id2name", IdName),
            _library: library,
        };
        Ok(Self {
            api: Arc::new(api),
            sha256,
            scratch,
        })
    }

    pub fn load(&self, root: &Path, entry: &str, timestep: f64) -> Result<Model> {
        require(
            timestep.is_finite() && (1e-6..=1.0).contains(&timestep),
            "Use a finite simulation timestep.",
        )?;
        let prepared = Prepared::new(root, entry, &self.scratch)?;
        let path = c_string(
            prepared
                .directory
                .path()
                .join(entry)
                .to_str()
                .ok_or_else(|| Error::Invalid("Use a UTF-8 model path.".into()))?,
        )?;
        let _compiler = COMPILER
            .lock()
            .map_err(|_| Error::Library("The model loader couldn't continue".into()))?;
        let mut message = [0_i8; 4_096];
        // SAFETY: strings and the error buffer live for the complete load; input
        // dependencies are bounded ordinary files in a private snapshot.
        let pointer = unsafe {
            (self.api.load)(
                path.as_ptr(),
                std::ptr::null(),
                message.as_mut_ptr(),
                message.len() as c_int,
            )
        };
        let Some(mut model) = NonNull::new(pointer) else {
            return Err(Error::Invalid(format!(
                "This robot couldn't be loaded: {}",
                error_text(&message)
            )));
        };
        // SAFETY: version 337 was validated, and mj_loadXML returned a model
        // with the exact generated 3.3.7 layout. Delete on a failed bound check.
        let native = unsafe { model.as_mut() };
        let bounded = (1..=256).contains(&native.nbody)
            && (0..=128).contains(&native.njnt)
            && (0..=512).contains(&native.nq)
            && (0..=512).contains(&native.nv)
            && (0..=128).contains(&native.nu)
            && (0..=1_024).contains(&native.ngeom)
            && (0..=64).contains(&native.nmesh)
            && (0..=32).contains(&native.ntex)
            && native.nbuffer <= 256 << 20
            && native.narena <= 256 << 20
            && native.nplugin == 0;
        if !bounded {
            // SAFETY: this is the unique model returned by this API.
            unsafe { (self.api.delete_model)(model.as_ptr()) };
            return Err(Error::Invalid(
                "This robot exceeds the native model limits or uses a plugin.".into(),
            ));
        }
        native.opt.timestep = timestep;
        // This qualified backend profile uses implicit actuator damping.
        native.opt.integrator = 3;
        // Position servos below own actuation explicitly. Source actuators
        // remain in the imported definition but cannot add a second controller.
        native.opt.disableflags |= 1 << 11;
        // SAFETY: makeData receives the live model and returns data owned here.
        let data = unsafe { (self.api.make)(model.as_ptr()) };
        let Some(data) = NonNull::new(data) else {
            // SAFETY: no data was allocated; the unique model is still live.
            unsafe { (self.api.delete_model)(model.as_ptr()) };
            return Err(Error::Library(
                "The robot's state couldn't be allocated".into(),
            ));
        };
        let mut result = Model {
            model,
            data,
            runtime: self.clone(),
            prepared,
            timestep,
            targets: BTreeMap::new(),
            controlled: BTreeMap::new(),
            objects: BTreeMap::new(),
            initial_qpos: Vec::new(),
            exported: String::new(),
            robot_root: None,
        };
        // SAFETY: initialize all derived fields in the owned model/data pair.
        unsafe { (result.runtime.api.forward)(result.model.as_ptr(), result.data.as_ptr()) };
        result.initial_qpos = result.qpos();
        let output = tempfile::NamedTempFile::new_in(result.prepared.directory.path())?;
        let output_path = c_string(
            output
                .path()
                .to_str()
                .ok_or_else(|| Error::Invalid("Use a UTF-8 model path.".into()))?,
        )?;
        // SAFETY: compiler lock still belongs to this load, so the legacy XML
        // is this exact model. The error buffer and output path remain live.
        let saved = unsafe {
            (self.api.save)(
                output_path.as_ptr(),
                result.model.as_ptr(),
                message.as_mut_ptr(),
                message.len() as c_int,
            )
        };
        // SAFETY: the compiled model owns its arrays; release only compiler XML.
        unsafe { (self.api.free_xml)() };
        require(
            saved != 0,
            &format!("This robot couldn't be saved: {}", error_text(&message)),
        )?;
        result.exported = String::from_utf8(read_file(output.path(), 1 << 20)?)
            .map_err(|_| Error::Invalid("The exported robot description isn't UTF-8.".into()))?;
        Ok(result)
    }

    /// Assemble a checked single-arm scene around a preserved URDF model.
    /// Robot and task objects share this one MuJoCo world and contact solver.
    pub fn arm_scene(
        &self,
        source: &Path,
        entry: &str,
        scene: &Scene,
        timestep: f64,
        servo: Servo,
    ) -> Result<Model> {
        scene.validate()?;
        require(
            scene.robots.len() == 1,
            "This simulation backend runs one arm per scene.",
        )?;
        let imported = self.load(source, entry, timestep)?;
        let description = robot_description::load(&source.join(entry))
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let xml = imported.export_mjcf()?;
        let doc = roxmltree::Document::parse(&xml).map_err(|e| Error::Invalid(e.to_string()))?;
        let world = doc
            .descendants()
            .find(|node| node.has_tag_name("worldbody"))
            .ok_or_else(|| Error::Invalid("This robot has no simulation bodies.".into()))?;
        let range = world.range();
        let old = &xml[range.clone()];
        let first = old
            .find('>')
            .ok_or_else(|| Error::Invalid("This robot's world couldn't be read.".into()))?
            + 1;
        let last = old
            .rfind("</worldbody>")
            .ok_or_else(|| Error::Invalid("This robot's world couldn't be read.".into()))?;
        let robot_pose = source_pose(&scene.robots[0].pose);
        let mut contents = format!(
            "<worldbody><body name=\"robot-mount\" pos=\"{}\" quat=\"{}\">{}</body>",
            numbers(&robot_pose.0),
            numbers(&robot_pose.1),
            &old[first..last]
        );
        for object in &scene.objects {
            let collider = object.collider.as_ref().ok_or_else(|| {
                Error::Invalid("Give every task object a collision shape.".into())
            })?;
            let pose = source_pose(&object.geometry.pose);
            let shape =
                match collider {
                    Shape::Box { size } => format!(
                        "type=\"box\" size=\"{}\"",
                        numbers(&[size[0] * 0.5, size[2] * 0.5, size[1] * 0.5])
                    ),
                    Shape::Sphere { radius } => format!("type=\"sphere\" size=\"{radius}\""),
                    Shape::Cylinder { radius, length } => {
                        format!("type=\"cylinder\" size=\"{radius} {}\"", length * 0.5)
                    }
                    Shape::Mesh { .. } => return Err(Error::Invalid(
                        "Use a checked box, sphere, or cylinder collision shape for task objects."
                            .into(),
                    )),
                };
            require(
                object
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b)),
                "Use short object names in simulation.",
            )?;
            let free = if object.mass > 0.0 {
                format!("<freejoint name=\"{}-free\"/>", object.id)
            } else {
                String::new()
            };
            let mass = if object.mass > 0.0 {
                format!(" mass=\"{}\"", object.mass)
            } else {
                String::new()
            };
            contents.push_str(&format!("<body name=\"{}\" pos=\"{}\" quat=\"{}\">{free}<geom name=\"{}-shape\" {shape}{mass} friction=\"{} 0.01 0.001\"/></body>", object.id, numbers(&pose.0), numbers(&pose.1), object.id, object.friction));
        }
        contents.push_str("</worldbody>");
        let mut converted = format!("{}{}{}", &xml[..range.start], contents, &xml[range.end..]);
        let mut actuators = String::from("<actuator>");
        for joint in description.movable() {
            require(
                joint
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b)),
                "Use short joint names for native simulation.",
            )?;
            actuators.push_str(&format!("<position name=\"robotlab:{}\" joint=\"{}\" kp=\"{}\" kv=\"{}\" forcelimited=\"true\" forcerange=\"-{} {}\"/>", joint.name, joint.name, servo.kp, servo.kd, servo.effort, servo.effort));
        }
        actuators.push_str("</actuator>");
        let mut exclusions = String::from("<contact>");
        for joint in description.joints() {
            let parent = &description.links()[joint.parent].name;
            let child = &description.links()[joint.child].name;
            require(
                [parent, child].iter().all(|name| {
                    name.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
                }),
                "Use short robot link names for native simulation.",
            )?;
            exclusions.push_str(&format!("<exclude body1=\"{parent}\" body2=\"{child}\"/>"));
        }
        exclusions.push_str("</contact>");
        let end = converted
            .rfind("</mujoco>")
            .ok_or_else(|| Error::Invalid("This robot couldn't be converted to a scene.".into()))?;
        converted.insert_str(end, &format!("{actuators}{exclusions}"));
        // Legacy export can put its resolved private mesh directory in the
        // compiler. Strip only this exact owned snapshot prefix.
        let prefix = format!("{}/", imported.prepared.directory.path().display());
        converted = converted.replace(&prefix, "");
        let generated = tempfile::tempdir_in(&self.scratch)?;
        for relative in imported.prepared.hashes.keys() {
            let to = generated.path().join(relative);
            fs::create_dir_all(to.parent().unwrap())?;
            fs::copy(imported.prepared.directory.path().join(relative), &to)?;
        }
        fs::write(generated.path().join("robot-scene.xml"), converted)?;
        let mut model = self.load(generated.path(), "robot-scene.xml", timestep)?;
        model.robot_root = Some(model.id(1, "robot-mount")?);
        for joint in description.movable() {
            model.control_joint(&joint.name, servo.clone())?;
        }
        let initial = &scene.robots[0].joints;
        require(
            initial
                .keys()
                .all(|name| model.controlled.contains_key(name)),
            "Set only this robot's named initial joints.",
        )?;
        for (name, value) in initial {
            let joint = &model.controlled[name];
            require(
                value.is_finite()
                    && model
                        .limits(joint)
                        .is_none_or(|(lower, upper)| *value >= lower && *value <= upper),
                "Keep initial joints inside the robot limits.",
            )?;
            model.initial_qpos[joint.qpos] = *value;
        }
        for object in &scene.objects {
            model.observe_object(&object.id, &object.id)?;
        }
        model.reset(0)?;
        Ok(model)
    }
}

fn source_pose(pose: &Pose) -> ([f64; 3], [f64; 4]) {
    let convert = DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
    let position = convert * DVec3::from_array(pose.position);
    let rotation = convert
        * DQuat::from_xyzw(
            pose.rotation[0],
            pose.rotation[1],
            pose.rotation[2],
            pose.rotation[3],
        )
        * convert.inverse();
    (
        position.to_array(),
        [rotation.w, rotation.x, rotation.y, rotation.z],
    )
}

fn numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Servo {
    pub kp: f64,
    pub kd: f64,
    pub effort: f64,
    pub speed: f64,
}

struct Controlled {
    joint: usize,
    qpos: usize,
    dof: usize,
    actuator: usize,
    servo: Servo,
}

/// Raw pointers deliberately keep this model on the thread that owns it.
/// Other threads exchange portable observations and stop requests.
pub struct Model {
    model: NonNull<ffi::mjModel>,
    data: NonNull<ffi::mjData>,
    runtime: Runtime,
    prepared: Prepared,
    timestep: f64,
    targets: BTreeMap<String, f64>,
    controlled: BTreeMap<String, Controlled>,
    objects: BTreeMap<String, usize>,
    initial_qpos: Vec<f64>,
    exported: String,
    robot_root: Option<usize>,
}

impl Drop for Model {
    fn drop(&mut self) {
        // SAFETY: this owner alone frees data, then its model. The runtime
        // field keeps both destructor symbols and the library alive.
        unsafe {
            (self.runtime.api.delete_data)(self.data.as_ptr());
            (self.runtime.api.delete_model)(self.model.as_ptr());
        }
    }
}

impl Model {
    fn native(&self) -> &ffi::mjModel {
        // SAFETY: the live version-checked model is owned for this borrow.
        unsafe { self.model.as_ref() }
    }
    fn state(&self) -> &ffi::mjData {
        // SAFETY: the live data is owned and never shared during mutation.
        unsafe { self.data.as_ref() }
    }
    fn id(&self, kind: i32, name: &str) -> Result<usize> {
        let name = c_string(name)?;
        // SAFETY: model and zero-terminated name remain live during the query.
        let index = unsafe { (self.runtime.api.name)(self.model.as_ptr(), kind, name.as_ptr()) };
        require(
            index >= 0,
            "This model is missing a requested body or joint.",
        )?;
        Ok(index as usize)
    }
    pub fn dimensions(&self) -> (usize, usize, usize) {
        let m = self.native();
        (m.nq as usize, m.nv as usize, m.nbody as usize)
    }
    pub fn qpos(&self) -> Vec<f64> {
        copy_numbers(self.state().qpos, self.native().nq as usize)
    }
    pub fn qvel(&self) -> Vec<f64> {
        copy_numbers(self.state().qvel, self.native().nv as usize)
    }
    pub fn time(&self) -> f64 {
        self.state().time
    }

    /// Select an explicit checked start pose before pinning the backend build.
    pub fn initial_pose(&mut self, values: &BTreeMap<String, f64>) -> Result<()> {
        self.validate_action(values)?;
        for (name, value) in values {
            self.initial_qpos[self.controlled[name].qpos] = *value;
        }
        self.reset(0)?;
        Ok(())
    }

    pub fn control_joint(&mut self, name: &str, servo: Servo) -> Result<()> {
        require(
            self.controlled.len() < 64
                && !name.is_empty()
                && name.len() <= 96
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
                && [servo.kp, servo.kd, servo.effort, servo.speed]
                    .iter()
                    .all(|n| n.is_finite() && *n > 0.0 && *n <= 1e12),
            "Choose named robot joints and finite servo settings.",
        )?;
        let joint = self.id(3, name)?;
        require(
            joint < self.native().njnt as usize,
            "Choose a joint in this model.",
        )?;
        // SAFETY: joint indexes are in the model's njnt-sized generated arrays.
        let (kind, qpos, dof) = unsafe {
            (
                *self.native().jnt_type.add(joint),
                *self.native().jnt_qposadr.add(joint),
                *self.native().jnt_dofadr.add(joint),
            )
        };
        require(
            matches!(kind, 2 | 3)
                && qpos >= 0
                && dof >= 0
                && qpos < self.native().nq
                && dof < self.native().nv,
            "Control only hinge or slider joints with position targets.",
        )?;
        let actuators: Vec<_> = (0..self.native().nu as usize)
            .filter(|index| {
                // SAFETY: actuator arrays hold nu rows and trnid has two entries each.
                unsafe {
                    *self.native().actuator_trntype.add(*index) == 0
                        && *self.native().actuator_trnid.add(*index * 2) == joint as i32
                        && *self.native().actuator_dyntype.add(*index) == 0
                }
            })
            .collect();
        require(
            actuators.len() == 1,
            "This joint needs one position servo. Open the robot as an arm scene.",
        )?;
        let actuator = actuators[0];
        // SAFETY: all indexes above are checked, and this unique model can
        // configure its fixed-gain affine position servo before stepping.
        unsafe {
            let native = self.model.as_mut();
            native.opt.disableflags &= !(1 << 11);
            *native.actuator_gaintype.add(actuator) = 0;
            *native.actuator_biastype.add(actuator) = 1;
            for i in 0..10 {
                *native.actuator_gainprm.add(actuator * 10 + i) = 0.0;
                *native.actuator_biasprm.add(actuator * 10 + i) = 0.0;
            }
            *native.actuator_gainprm.add(actuator * 10) = servo.kp;
            *native.actuator_biasprm.add(actuator * 10 + 1) = -servo.kp;
            *native.actuator_biasprm.add(actuator * 10 + 2) = -servo.kd;
            *native.actuator_forcelimited.add(actuator) = 1;
            *native.actuator_forcerange.add(actuator * 2) = -servo.effort;
            *native.actuator_forcerange.add(actuator * 2 + 1) = servo.effort;
            for i in 0..6 {
                *native.actuator_gear.add(actuator * 6 + i) = if i == 0 { 1.0 } else { 0.0 };
            }
        }
        self.targets.insert(name.into(), self.qpos()[qpos as usize]);
        self.controlled.insert(
            name.into(),
            Controlled {
                joint,
                qpos: qpos as usize,
                dof: dof as usize,
                actuator,
                servo,
            },
        );
        Ok(())
    }

    pub fn observe_object(&mut self, id: &str, body_name: &str) -> Result<()> {
        require(
            !id.is_empty()
                && id.len() <= 96
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
                && self.objects.len() < 256,
            "Choose a short object name.",
        )?;
        let body = self.id(1, body_name)?;
        require(
            body < self.native().nbody as usize,
            "Choose an object body in this model.",
        )?;
        self.objects.insert(id.into(), body);
        Ok(())
    }

    /// A source-local body frame placed in runtime Y-up coordinates, for
    /// drawing the original link meshes without another axis conversion.
    pub fn body_pose(&self, name: &str) -> Result<Pose> {
        self.body_pose_id(self.id(1, name)?)
    }
    fn body_pose_id(&self, body: usize) -> Result<Pose> {
        require(
            body < self.native().nbody as usize,
            "Choose a body in this model.",
        )?;
        // SAFETY: xpos and xquat hold 3/4 values per checked body index.
        let position = unsafe { copy_numbers(self.state().xpos.add(body * 3), 3) };
        let quaternion = unsafe { copy_numbers(self.state().xquat.add(body * 4), 4) };
        let convert = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
        let p = convert * DVec3::from_slice(&position);
        let q =
            convert * DQuat::from_xyzw(quaternion[1], quaternion[2], quaternion[3], quaternion[0]);
        let pose = Pose {
            position: p.to_array(),
            rotation: q.to_array(),
        };
        pose.validate()?;
        Ok(pose)
    }

    /// Return the conversion captured under the model's original compile lock.
    /// Later loads cannot change which source this conversion describes.
    pub fn export_mjcf(&self) -> Result<String> {
        Ok(self.exported.clone())
    }

    fn contacts(&self) -> Result<Vec<Contact>> {
        require(
            (0..=1_024).contains(&self.state().ncon),
            "This simulation has too many contacts to record.",
        )?;
        let mut contacts = Vec::new();
        let convert = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
        for index in 0..self.state().ncon as usize {
            // SAFETY: ncon is checked and contact is MuJoCo's ncon-entry array.
            let contact = unsafe { &*self.state().contact.add(index) };
            if contact.exclude != 0 {
                continue;
            }
            require(
                contact
                    .geom
                    .iter()
                    .all(|id| *id >= 0 && *id < self.native().ngeom),
                "This backend doesn't record flexible-body contacts.",
            )?;
            let mut force = [0.0; 6];
            // SAFETY: force has the six entries required by a checked contact id.
            unsafe {
                (self.runtime.api.contact_force)(
                    self.model.as_ptr(),
                    self.data.as_ptr(),
                    index as c_int,
                    force.as_mut_ptr(),
                )
            };
            let normal = DVec3::from_slice(&contact.frame[..3]);
            let tangent_a = DVec3::from_slice(&contact.frame[3..6]);
            let tangent_b = DVec3::from_slice(&contact.frame[6..9]);
            let world_force =
                convert * (normal * force[0] + tangent_a * force[1] + tangent_b * force[2]);
            contacts.push(Contact {
                a: self.contact_label(contact.geom[0])?,
                b: self.contact_label(contact.geom[1])?,
                position: (convert * DVec3::from_array(contact.pos)).to_array(),
                force: world_force.to_array(),
            });
        }
        Ok(contacts)
    }

    fn contact_label(&self, geometry: i32) -> Result<String> {
        require(
            geometry >= 0 && geometry < self.native().ngeom,
            "Choose geometry from this model.",
        )?;
        // SAFETY: geometry is checked against ngeom and maps to a model body.
        let body = unsafe { *self.native().geom_bodyid.add(geometry as usize) };
        for (name, id) in &self.objects {
            if *id == body as usize {
                return Ok(name.clone());
            }
        }
        // SAFETY: MuJoCo owns any returned zero-terminated name for this model.
        let name = unsafe { (self.runtime.api.id_name)(self.model.as_ptr(), 1, body) };
        let name = if name.is_null() {
            format!("body-{body}")
        } else {
            // SAFETY: id2name returned a live model-owned string.
            unsafe { CStr::from_ptr(name) }
                .to_string_lossy()
                .into_owned()
        };
        Ok(if self.is_robot_body(body as usize) {
            format!("robot:{name}")
        } else {
            format!("scene:{name}")
        })
    }

    fn is_robot_body(&self, mut body: usize) -> bool {
        for _ in 0..self.native().nbody {
            if self.robot_root == Some(body) {
                return true;
            }
            if body == 0 || body >= self.native().nbody as usize {
                return false;
            }
            // SAFETY: each joint and body index belongs to this model.
            unsafe {
                if self
                    .controlled
                    .values()
                    .any(|joint| *self.native().jnt_bodyid.add(joint.joint) as usize == body)
                {
                    return true;
                }
                body = *self.native().body_parentid.add(body) as usize;
            }
        }
        false
    }

    fn limits(&self, controlled: &Controlled) -> Option<(f64, f64)> {
        // SAFETY: each Controlled joint id was checked against njnt at binding.
        unsafe {
            (*self.native().jnt_limited.add(controlled.joint) != 0).then(|| {
                (
                    *self.native().jnt_range.add(controlled.joint * 2),
                    *self.native().jnt_range.add(controlled.joint * 2 + 1),
                )
            })
        }
    }
}

impl Backend for Model {
    fn build(&self) -> Build {
        let profile: BTreeMap<_, _> = self
            .controlled
            .iter()
            .map(|(name, joint)| (name, &joint.servo))
            .collect();
        let config = serde_json::to_vec(&(
            &self.runtime.sha256,
            &self.prepared.hashes,
            self.timestep,
            profile,
            &self.objects,
            &self.initial_qpos,
        ))
        .expect("finite checked model configuration");
        let mut hasher = Sha256::new();
        hasher.update(config);
        hasher.update(include_str!("mujoco.rs"));
        hasher.update(include_str!("mujoco_ffi.rs"));
        Build {
            name: "robot-lab-mujoco-3.3.7".into(),
            sha256: format!("{:x}", hasher.finalize()),
        }
    }
    fn timestep(&self) -> f64 {
        self.timestep
    }
    fn reset(&mut self, _seed: u64) -> Result<Snapshot> {
        // SAFETY: reset and forward receive this owner's matching model/data.
        unsafe {
            (self.runtime.api.reset)(self.model.as_ptr(), self.data.as_ptr());
        }
        let initial = self.initial_qpos.clone();
        // SAFETY: qpos holds nq writable entries and initial was copied at load.
        unsafe {
            std::ptr::copy_nonoverlapping(initial.as_ptr(), self.state().qpos, initial.len());
        }
        self.targets = self
            .controlled
            .iter()
            .map(|(name, joint)| (name.clone(), initial[joint.qpos]))
            .collect();
        for joint in self.controlled.values() {
            // SAFETY: each actuator index was checked against nu during binding.
            unsafe {
                *self.state().ctrl.add(joint.actuator) = initial[joint.qpos];
            }
        }
        // SAFETY: refresh the matching owned data's derived transforms.
        unsafe {
            (self.runtime.api.forward)(self.model.as_ptr(), self.data.as_ptr());
        }
        self.snapshot()
    }
    fn snapshot(&self) -> Result<Snapshot> {
        let qpos = self.qpos();
        let qvel = self.qvel();
        require(
            qpos.iter().chain(&qvel).all(|n| n.is_finite()),
            "The simulation produced an invalid robot state.",
        )?;
        let mut result = Snapshot::default();
        for (name, joint) in &self.controlled {
            let position = qpos[joint.qpos];
            result.joints.insert(name.clone(), position);
            result.velocities.insert(name.clone(), qvel[joint.dof]);
            if self
                .limits(joint)
                .is_some_and(|(lower, upper)| position < lower || position > upper)
            {
                result.limit_violations.push(name.clone());
            }
        }
        let convert = DQuat::from_rotation_x(-std::f64::consts::FRAC_PI_2);
        for (id, body) in &self.objects {
            let mut velocity = [0.0; 6];
            // SAFETY: this body id is checked and velocity holds six entries.
            unsafe {
                (self.runtime.api.velocity)(
                    self.model.as_ptr(),
                    self.data.as_ptr(),
                    1,
                    *body as c_int,
                    velocity.as_mut_ptr(),
                    0,
                )
            };
            let mut pose = self.body_pose_id(*body)?;
            let q = DQuat::from_xyzw(
                pose.rotation[0],
                pose.rotation[1],
                pose.rotation[2],
                pose.rotation[3],
            ) * convert.inverse();
            pose.rotation = q.to_array();
            result.objects.insert(
                id.clone(),
                ObjectState {
                    pose,
                    angular_velocity: (convert * DVec3::from_slice(&velocity[..3])).to_array(),
                    linear_velocity: (convert * DVec3::from_slice(&velocity[3..])).to_array(),
                    held: false,
                },
            );
        }
        result.contacts = self.contacts()?;
        for (name, object) in &mut result.objects {
            object.held = result.contacts.iter().any(|c| {
                (c.a == *name && c.b.starts_with("robot:"))
                    || (c.b == *name && c.a.starts_with("robot:"))
            });
        }
        Ok(result)
    }
    fn validate_action(&self, values: &BTreeMap<String, f64>) -> Result<()> {
        require(
            values.len() == self.controlled.len(),
            "Set every controlled joint in this action.",
        )?;
        for (name, value) in values {
            let joint = self.controlled.get(name).ok_or_else(|| {
                Error::Invalid("The action names a joint outside this robot.".into())
            })?;
            require(
                value.is_finite()
                    && self
                        .limits(joint)
                        .is_none_or(|(lower, upper)| *value >= lower && *value <= upper),
                "Keep robot actions inside the joint limits.",
            )?;
        }
        Ok(())
    }
    fn apply(&mut self, values: &BTreeMap<String, f64>) -> Result<()> {
        self.validate_action(values)?;
        self.targets = values.clone();
        Ok(())
    }
    fn step(&mut self) -> Result<Snapshot> {
        let previous_time = self.time();
        let qpos = self.qpos();
        let mut controls = vec![0.0; self.native().nu as usize];
        for (name, joint) in &self.controlled {
            let servo = &joint.servo;
            let velocity = (servo.kp * (self.targets[name] - qpos[joint.qpos]) / servo.kd)
                .clamp(-servo.speed, servo.speed);
            controls[joint.actuator] = qpos[joint.qpos] + velocity * servo.kd / servo.kp;
        }
        // SAFETY: ctrl is an nu-entry array in the matching data;
        // advancing and refreshing it is exclusive to this owner.
        unsafe {
            if !controls.is_empty() {
                std::ptr::copy_nonoverlapping(controls.as_ptr(), self.state().ctrl, controls.len());
            }
            (self.runtime.api.step)(self.model.as_ptr(), self.data.as_ptr());
            (self.runtime.api.forward)(self.model.as_ptr(), self.data.as_ptr());
        }
        require(
            (self.time() - previous_time - self.timestep).abs() <= 1e-9,
            "This robot became unstable. Try a smaller simulation step.",
        )?;
        self.snapshot()
    }
    fn sensors(&mut self, _: u32) -> Result<BTreeMap<String, Sensor>> {
        Ok(BTreeMap::new())
    }
}

fn copy_numbers(pointer: *const f64, length: usize) -> Vec<f64> {
    if length == 0 {
        return Vec::new();
    }
    // SAFETY: callers supply only model-sized live arrays owned by Model.
    unsafe { std::slice::from_raw_parts(pointer, length).to_vec() }
}

fn c_string(value: &str) -> Result<CString> {
    CString::new(value)
        .map_err(|_| Error::Invalid("Use a name or path without null characters.".into()))
}
fn error_text(bytes: &[i8]) -> String {
    let bytes: Vec<_> = bytes
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as u8)
        .collect();
    String::from_utf8_lossy(&bytes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    require(
        fs::symlink_metadata(path)?.file_type().is_file() && fs::metadata(path)?.len() <= limit,
        "Use ordinary robot files within the import size limit.",
    )?;
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 <= limit,
        "This robot file exceeds the import size limit.",
    )?;
    Ok(bytes)
}
fn file_hash(path: &Path, limit: u64) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(read_file(path, limit)?)))
}

struct Prepared {
    directory: tempfile::TempDir,
    hashes: BTreeMap<String, String>,
}

impl Prepared {
    fn new(root: &Path, entry: &str, scratch: &Path) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        let mut files = BTreeMap::new();
        let mut pending = vec![entry.to_owned()];
        let mut visited = BTreeSet::new();
        let mut converted_urdf = BTreeMap::new();
        while let Some(relative) = pending.pop() {
            if !visited.insert(relative.clone()) {
                continue;
            }
            require(visited.len() <= 256, "Use at most 256 robot files.")?;
            let path = asset_path(&root, &relative)?;
            let bytes = read_file(
                &path,
                if relative.ends_with(".xml") || relative.ends_with(".urdf") {
                    1 << 20
                } else {
                    16 << 20
                },
            )?;
            require(
                files
                    .values()
                    .map(|b: &Vec<u8>| b.len() as u64)
                    .sum::<u64>()
                    + bytes.len() as u64
                    <= MAX_INPUT_BYTES,
                "This robot's files exceed the 64 MB import limit.",
            )?;
            if relative.ends_with(".xml") || relative.ends_with(".urdf") {
                let xml = std::str::from_utf8(&bytes)
                    .map_err(|_| Error::Invalid("Use a UTF-8 robot description.".into()))?;
                let doc = roxmltree::Document::parse_with_options(
                    xml,
                    roxmltree::ParsingOptions {
                        allow_dtd: false,
                        nodes_limit: 50_000,
                        ..Default::default()
                    },
                )
                .map_err(|e| {
                    Error::Invalid(format!("This robot description couldn't be read: {e}"))
                })?;
                let root_name = doc.root_element().tag_name().name();
                require(
                    matches!(root_name, "mujoco" | "robot" | "mujocoinclude"),
                    "Choose an MJCF or URDF robot description.",
                )?;
                if root_name == "robot" {
                    robot_description::load(&path)
                        .map_err(|error| Error::Invalid(error.to_string()))?;
                    require(
                        !doc.descendants().any(|node| node.has_tag_name("mujoco")),
                        "Use MJCF for a robot with existing MuJoCo options.",
                    )?;
                    let closing = xml.rfind("</robot>").ok_or_else(|| {
                        Error::Invalid("Include links and joints in the robot description.".into())
                    })?;
                    let mut converted = xml.to_owned();
                    converted.insert_str(closing, "<mujoco><compiler strippath=\"false\" fusestatic=\"false\" discardvisual=\"false\"/></mujoco>");
                    converted_urdf.insert(relative.clone(), converted.into_bytes());
                }
                let parent = Path::new(&relative).parent().unwrap_or(Path::new(""));
                let compiler = doc.descendants().find(|n| n.has_tag_name("compiler"));
                let meshdir = compiler.and_then(|n| n.attribute("meshdir")).unwrap_or("");
                let texturedir = compiler
                    .and_then(|n| n.attribute("texturedir"))
                    .unwrap_or("");
                for node in doc.descendants().filter(|n| n.is_element()) {
                    require(
                        node.ancestors().count() <= 128,
                        "Use fewer nested robot elements.",
                    )?;
                    require(
                        !matches!(
                            node.tag_name().name(),
                            "plugin"
                                | "extension"
                                | "flex"
                                | "flexcomp"
                                | "composite"
                                | "replicate"
                                | "attach"
                        ),
                        "This backend doesn't load plugins or flexible robots.",
                    )?;
                    if matches!(node.tag_name().name(), "texture" | "hfield") {
                        let keys = if node.has_tag_name("texture") {
                            ["width", "height"]
                        } else {
                            ["nrow", "ncol"]
                        };
                        for key in keys {
                            if let Some(value) = node.attribute(key) {
                                require(
                                    value.parse::<u32>().is_ok_and(|n| (1..=2048).contains(&n)),
                                    "Use textures and height maps no larger than 2048 pixels per side.",
                                )?;
                            }
                        }
                        // External image dimensions need a decoder preflight;
                        // the qualified catalog uses generated textures only.
                        require(
                            !node.has_tag_name("texture") || node.attribute("file").is_none(),
                            "Use generated textures for this simulation backend.",
                        )?;
                    }
                    if node.has_tag_name("size") {
                        for attribute in node.attributes() {
                            let value = attribute.value();
                            require(
                                matches!(
                                    attribute.name(),
                                    "memory" | "njmax" | "nconmax" | "nstack"
                                ),
                                "This backend doesn't support that model size setting.",
                            )?;
                            if attribute.name() == "memory" {
                                let (digits, multiplier) = match value.as_bytes().last() {
                                    Some(b'K' | b'k') => (&value[..value.len() - 1], 1_u64 << 10),
                                    Some(b'M' | b'm') => (&value[..value.len() - 1], 1_u64 << 20),
                                    Some(b'G' | b'g') => (&value[..value.len() - 1], 1_u64 << 30),
                                    _ => (value, 1),
                                };
                                let size = digits
                                    .parse::<u64>()
                                    .ok()
                                    .and_then(|n| n.checked_mul(multiplier));
                                require(
                                    size.is_some_and(|n| n > 0 && n <= 256 << 20),
                                    "Use at most 256 MB of simulation memory.",
                                )?;
                            } else {
                                require(
                                    value
                                        .parse::<i64>()
                                        .is_ok_and(|n| (-1..=100_000).contains(&n)),
                                    "Use bounded contact and stack sizes.",
                                )?;
                            }
                        }
                    }
                    let dependency = node.attribute("file").or_else(|| {
                        node.has_tag_name("mesh")
                            .then(|| node.attribute("filename"))
                            .flatten()
                    });
                    if let Some(file) = dependency {
                        let prefix = if node.has_tag_name("mesh") {
                            meshdir
                        } else if node.has_tag_name("texture") {
                            texturedir
                        } else {
                            ""
                        };
                        let dependency = parent
                            .join(prefix)
                            .join(file)
                            .to_str()
                            .ok_or_else(|| Error::Invalid("Use UTF-8 robot file paths.".into()))?
                            .replace('\\', "/");
                        asset_path(&root, &dependency)?;
                        pending.push(dependency);
                    }
                }
            }
            files.insert(relative, bytes);
        }
        let directory = tempfile::tempdir_in(scratch)?;
        let mut hashes = BTreeMap::new();
        for (relative, bytes) in files {
            let output = directory.path().join(&relative);
            fs::create_dir_all(output.parent().unwrap())?;
            let mut file = File::create(output)?;
            file.write_all(converted_urdf.get(&relative).unwrap_or(&bytes))?;
            hashes.insert(relative, format!("{:x}", Sha256::digest(&bytes)));
        }
        Ok(Self { directory, hashes })
    }
}

fn asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.is_empty()
            && relative.len() <= 512
            && !relative.contains(['\\', ':', '\0'])
            && relative
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."),
        "Keep robot assets inside their source folder.",
    )?;
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(part) = component else {
            return Err(Error::Invalid(
                "Keep robot assets inside their source folder.".into(),
            ));
        };
        path.push(part);
        require(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "Use ordinary robot assets without links.",
        )?;
    }
    Ok(path)
}
