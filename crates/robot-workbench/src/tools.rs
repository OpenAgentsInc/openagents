//! Typed Robot Lab tools and their model-facing declarations.
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "tool", deny_unknown_fields)]
pub enum Tool {
    #[serde(rename = "robot_experiment")]
    #[schemars(
        description = "Save an experiment for this exact scene. Choose the configured policy, or provide frames_json as a list of joint-name/position maps and hold control intervals. This does not start execution."
    )]
    Experiment {
        scene: String,
        version: String,
        document: String,
        seeds: Vec<u64>,
        max_steps: u32,
        policy: bool,
        frames_json: String,
        hold: u32,
    },
    #[serde(rename = "robot_run")]
    #[schemars(
        description = "Explicitly start a saved experiment. batch continues while views close; interactive starts paused. Use a new job name and a declared experiment seed."
    )]
    Run {
        experiment: String,
        version: String,
        seed: u64,
        job: String,
        batch: bool,
    },
    #[serde(rename = "robot_run_status")]
    #[schemars(
        description = "Read run progress and its saved attempt, without starting or resuming execution."
    )]
    RunStatus { job: String },
    #[serde(rename = "robot_run_control")]
    #[schemars(
        description = "Control an explicitly started run. command_json is {\"action\":\"pause|resume|step|stop\"} or {\"action\":\"reset\",\"next_job\":\"NEW_NAME\",\"seed\":N}. Reset saves the previous attempt; stop saves partial work."
    )]
    RunControl { job: String, command_json: String },
    #[serde(rename = "robot_list")]
    #[schemars(description = "List this project's robot scenes and saved attempts.")]
    List,
    #[serde(rename = "robot_import")]
    #[schemars(
        description = "Import the pinned SO-101 description and open a table scene with a red cube."
    )]
    Import {
        document: String,
        source: String,
        revision: String,
    },
    #[serde(rename = "robot_open")]
    #[schemars(
        description = "Open a robot scene or saved attempt at its exact version; omit version for the latest."
    )]
    Open {
        document: String,
        version: Option<String>,
    },
    #[serde(rename = "robot_pose")]
    #[schemars(
        description = "Move one joint in a scene, within its limits, creating a new version. base is the exact saved version hash."
    )]
    Pose {
        document: String,
        base: String,
        joint: String,
        position: f64,
    },
    #[serde(rename = "robot_move_object")]
    #[schemars(
        description = "Move a scene object to x,y,z in meters, creating a new version. base is the exact saved version hash."
    )]
    MoveObject {
        document: String,
        base: String,
        object: String,
        x: f64,
        y: f64,
        z: f64,
    },
    #[serde(rename = "robot_edit")]
    #[schemars(
        description = "Save a checked scene document after reading it. base is the exact saved version hash and scene_json is the scene content as JSON."
    )]
    Edit {
        document: String,
        base: String,
        scene_json: String,
        summary: String,
    },
    #[serde(rename = "robot_compare")]
    #[schemars(
        description = "Compare two saved versions of one scene or attempt with a shared camera."
    )]
    Compare {
        document: String,
        a: String,
        b: String,
        other_document: Option<String>,
    },
    #[serde(rename = "robot_versions")]
    #[schemars(description = "List this document's versions and what changed.")]
    Versions { document: String },
    #[serde(rename = "robot_notes")]
    #[schemars(description = "Read notes attached to an exact saved scene or attempt.")]
    Notes { document: String, version: String },
    #[serde(rename = "robot_add_note")]
    #[schemars(
        description = "Save a note on an exact version and optional frame of a recorded attempt."
    )]
    AddNote {
        document: String,
        version: String,
        frame: Option<u32>,
        text: String,
    },
    #[serde(rename = "robot_inspect")]
    #[schemars(
        description = "Read the observed pose, actions, contacts and cameras at one saved attempt frame."
    )]
    Inspect {
        document: String,
        version: String,
        frame: u32,
    },
    #[serde(rename = "robot_select")]
    #[schemars(description = "Keep this exact scene or attempt selected when the chat reopens.")]
    Select {
        chat: String,
        document: String,
        version: String,
        frame: Option<u32>,
        camera: String,
        selection_base: Option<String>,
        comparison: Option<robot_lab::Reference>,
    },
    #[serde(rename = "robot_restore")]
    #[schemars(description = "Reopen this chat's last selected Robot Lab version.")]
    Restore { chat: String },
}

pub fn definitions() -> Value {
    let schema = serde_json::to_value(schema_for!(Tool)).expect("serializable tool schema");
    let tools = schema["oneOf"]
        .as_array()
        .or_else(|| schema["anyOf"].as_array())
        .expect("tagged tool variants")
        .iter()
        .map(|variant| {
            let name = variant["properties"]["tool"]["const"]
                .as_str()
                .expect("one tool name");
            let mut parameters = variant.clone();
            parameters.as_object_mut().unwrap().remove("description");
            parameters["properties"]
                .as_object_mut()
                .unwrap()
                .remove("tool");
            if let Some(required) = parameters["required"].as_array_mut() {
                required.retain(|v| v != "tool");
            }
            if let Some(definitions) = schema.get("$defs") {
                parameters["$defs"] = definitions.clone();
            }
            json!({"name":name,"description":variant["description"],"parameters":parameters})
        })
        .collect::<Vec<_>>();
    json!({"tools":tools})
}
