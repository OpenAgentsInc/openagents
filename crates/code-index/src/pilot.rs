//! Source-reviewed pilot tasks. Compilation binds every anchor to a Git tree.

use crate::recipes::*;

struct Seed {
    id: &'static str,
    capability: &'static str,
    operation: &'static str,
    description: &'static str,
    anchors: &'static [(&'static str, &'static str, &'static str)],
}

const SEEDS: &[Seed] = &[
    Seed {
        id: "model-pack",
        capability: "3d-assets",
        operation: "add-model-data",
        description: "Add a named 3D model to a validated asset pack.",
        anchors: &[
            (
                "crates/verse-engine/src/assets.rs",
                "verse_engine::assets::Pack",
                "Add model data and keep texture and placement references valid.",
            ),
            (
                "crates/verse-engine/src/assets.rs",
                "verse_engine::assets::Pack::validate",
                "Check pack limits, geometry, and referenced models.",
            ),
            (
                "crates/verse-engine/src/residency.rs",
                "verse_engine::residency::Catalog::new",
                "Check how a validated model becomes a runtime handle.",
            ),
        ],
    },
    Seed {
        id: "authored-box",
        capability: "3d-assets",
        operation: "author-primitive",
        description: "Add an authored box model through the existing primitive geometry path.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/geometry.rs",
                "verse_content::authoring::geometry::compile",
                "Compile bounds, texture, and tint into model geometry.",
            ),
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::Document",
                "Store primitive definitions in the authoring document.",
            ),
            (
                "crates/verse-content/src/authoring/mod.rs",
                "verse_content::authoring::admit",
                "Apply primitive, inventory, and placement checks together.",
            ),
        ],
    },
    Seed {
        id: "model-placement",
        capability: "3d-assets",
        operation: "place-model",
        description: "Place an existing 3D model using a named model and a finite transform.",
        anchors: &[
            (
                "crates/verse-engine/src/assets.rs",
                "verse_engine::assets::Placement",
                "Set the model name and transform.",
            ),
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::Document",
                "Keep authored placement IDs stable.",
            ),
            (
                "crates/verse-content/src/authoring/mod.rs",
                "verse_content::authoring::admit",
                "Validate the model name and transform before use.",
            ),
        ],
    },
    Seed {
        id: "model-material",
        capability: "3d-assets",
        operation: "override-material",
        description: "Override an authored model's materials while preserving texture references.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::ModelEdit",
                "Represent per-surface material overrides.",
            ),
            (
                "crates/verse-content/src/authoring/mod.rs",
                "verse_content::authoring::admit",
                "Apply overrides and check inventory texture dependencies.",
            ),
        ],
    },
    Seed {
        id: "animation-binding",
        capability: "3d-animation",
        operation: "bind-animation-state",
        description: "Bind a model animation state and preserve catalog validation.",
        anchors: &[
            (
                "crates/verse-engine/src/motion.rs",
                "verse_engine::motion::Binding",
                "Describe an animation selection and playback behavior.",
            ),
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::ModelEdit",
                "Store authored state bindings.",
            ),
            (
                "crates/verse-engine/src/residency.rs",
                "verse_engine::residency::Catalog::check_animation",
                "Check that a model declares the selected animation state.",
            ),
        ],
    },
    Seed {
        id: "runtime-model-handle",
        capability: "3d-assets",
        operation: "resolve-runtime-model",
        description: "Resolve an existing model name into a handle belonging to the active runtime catalog.",
        anchors: &[
            (
                "crates/verse-engine/src/residency.rs",
                "verse_engine::residency::Catalog::model",
                "Resolve the model name and retain its catalog identity.",
            ),
            (
                "crates/verse-engine/src/residency.rs",
                "verse_engine::residency::Catalog::check",
                "Reject a handle from another catalog.",
            ),
        ],
    },
    Seed {
        id: "persistent-model-id",
        capability: "3d-assets",
        operation: "bind-persistent-asset",
        description: "Bind a persistent asset identity to a model key or texture slot.",
        anchors: &[
            (
                "crates/verse-engine/src/inventory.rs",
                "verse_engine::inventory::Binding",
                "Choose a model, texture, or source binding.",
            ),
            (
                "crates/verse-engine/src/inventory.rs",
                "verse_engine::inventory::Inventory::validate",
                "Check identities, bindings, and dependency references.",
            ),
            (
                "crates/verse-engine/src/residency.rs",
                "verse_engine::residency::Catalog::model_asset",
                "Resolve an asset ID into a runtime model handle.",
            ),
        ],
    },
    Seed {
        id: "authored-timeline",
        capability: "3d-authoring",
        operation: "add-timeline-cue",
        description: "Add a stable-ID authored timeline cue and convert it to runtime scene order.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::TimelineCue",
                "Keep the authoring ID separate from cue time.",
            ),
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::Document::runtime_scene",
                "Sort runtime cues by time and stable ID.",
            ),
            (
                "crates/verse-content/src/authoring/mod.rs",
                "verse_content::authoring::admit",
                "Check cue identities, actors, and actions.",
            ),
        ],
    },
    Seed {
        id: "authoring-edit",
        capability: "3d-authoring",
        operation: "add-edit-operation",
        description: "Add a data-only authoring edit while preserving transaction validation and history.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::Edit",
                "Describe the new serializable edit operation.",
            ),
            (
                "crates/verse-content/src/authoring/document.rs",
                "verse_content::authoring::document::Edit::apply",
                "Apply the edit to a candidate document before workspace validation.",
            ),
            (
                "crates/verse-content/src/authoring/workspace.rs",
                "verse_content::authoring::workspace::Workspace::transact",
                "Validate before changing document and history.",
            ),
        ],
    },
    Seed {
        id: "authoring-workspace",
        capability: "3d-authoring",
        operation: "validate-workspace",
        description: "Validate a saved authoring workspace before using its content.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/workspace.rs",
                "verse_content::authoring::workspace::Workspace::validate_current",
                "Use the workspace's current validation path.",
            ),
            (
                "crates/verse-content/src/authoring/mod.rs",
                "verse_content::authoring::admit",
                "Reuse runtime content validation.",
            ),
        ],
    },
    Seed {
        id: "release-manifest",
        capability: "3d-publication",
        operation: "extend-release-manifest",
        description: "Extend a public content release while keeping file bounds and verification in agreement.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/release.rs",
                "verse_content::authoring::release::Manifest",
                "Describe the release's files and compatibility profile.",
            ),
            (
                "crates/verse-content/src/authoring/release.rs",
                "verse_content::authoring::release::build",
                "Include the new file in release construction.",
            ),
            (
                "crates/verse-content/src/authoring/release.rs",
                "verse_content::authoring::release::verify",
                "Verify the same file set and digests when reading.",
            ),
        ],
    },
    Seed {
        id: "publication-review",
        capability: "3d-publication",
        operation: "extend-publication-review",
        description: "Change authored-content publication review with submission and resolution checks.",
        anchors: &[
            (
                "crates/verse-content/src/authoring/publication.rs",
                "verse_content::authoring::publication::Book::review",
                "Keep publisher authority and expected revision checks.",
            ),
            (
                "crates/verse-content/src/authoring/publication.rs",
                "verse_content::authoring::publication::Book::submit",
                "Keep submission admission consistent with review.",
            ),
            (
                "crates/verse-content/src/authoring/publication.rs",
                "verse_content::authoring::publication::Book::resolve",
                "Keep served content consistent with publication status.",
            ),
        ],
    },
    Seed {
        id: "coder-plugin",
        capability: "coder-plugins",
        operation: "register-plugin",
        description: "Register a built-in Coder plugin and connect its saved enablement and supported bindings.",
        anchors: &[
            (
                "crates/coder-new/src/plugin_definition.rs",
                "coder_new::plugin_definition::DEFINITIONS",
                "Add the plugin's supported bindings and presentation metadata.",
            ),
            (
                "crates/coder-new/src/bundled_settings.rs",
                "coder_new::bundled_settings::BundledSettings::toggle",
                "Connect persisted enablement where the plugin needs it.",
            ),
            (
                "crates/coder-new/src/plugin_tools.rs",
                "coder_new::plugin_tools::Family::of",
                "Check how the plugin's tools join the available tool families.",
            ),
        ],
    },
    Seed {
        id: "graph-tool",
        capability: "code-tools",
        operation: "add-graph-query",
        description: "Add a typed structural graph query with matching schema and execution dispatch.",
        anchors: &[
            (
                "crates/code-graph/src/tools.rs",
                "code_graph::tools::definitions",
                "Declare the query's arguments and meaning.",
            ),
            (
                "crates/code-graph/src/tools.rs",
                "code_graph::tools::execute",
                "Connect the query to execution and graph loading.",
            ),
            (
                "crates/code-graph/src/query.rs",
                "code_graph::query::Index",
                "Implement the query over graph lookups.",
            ),
        ],
    },
    Seed {
        id: "cli-command",
        capability: "cli",
        operation: "add-command-group",
        description: "Expose an existing Rust capability as an openagents command group with help and JSON output.",
        anchors: &[
            (
                "crates/openagents-cli/src/main.rs",
                "openagents_cli::USAGE",
                "Add discoverable command help and inspect dispatch in this file.",
            ),
            (
                "crates/openagents-cli/src/argv.rs",
                "openagents_cli::argv",
                "Follow the shared argument handling module.",
            ),
            (
                "crates/openagents-cli/src/out.rs",
                "openagents_cli::out",
                "Use the shared command output module.",
            ),
        ],
    },
    Seed {
        id: "graph-cli-flag",
        capability: "cli",
        operation: "add-code-graph-flag",
        description: "Add a code-graph CLI flag and pass its typed value into the owning library operation.",
        anchors: &[
            (
                "crates/code-graph/src/main.rs",
                "code_graph::run",
                "Update parsing, usage, and execution together.",
            ),
            (
                "crates/code-graph/src/tools.rs",
                "code_graph::tools::execute",
                "Check whether the equivalent agent-tool argument also changes.",
            ),
        ],
    },
    Seed {
        id: "world-wire-field",
        capability: "world-network",
        operation: "extend-input-field",
        description: "Extend a world input field with both command conversions and client request handling.",
        anchors: &[
            (
                "crates/verse-world/src/service/wire.rs",
                "verse_world::service::wire::Input",
                "Update the serialized shape and inspect both conversion implementations.",
            ),
            (
                "crates/verse-world/src/service/client.rs",
                "verse_world::service::client::Client::request",
                "Check the sending client's message construction and response path.",
            ),
            (
                "crates/verse-world/src/service/net.rs",
                "verse_world::service::net",
                "Inspect server transport decoding and dispatch.",
            ),
        ],
    },
    Seed {
        id: "world-request",
        capability: "world-network",
        operation: "extend-request-body",
        description: "Add a world request operation across wire types, client calls, and server dispatch.",
        anchors: &[
            (
                "crates/verse-world/src/service/wire.rs",
                "verse_world::service::wire::Body",
                "Define the request variant and inspect version/bound checks.",
            ),
            (
                "crates/verse-world/src/service/client.rs",
                "verse_world::service::client::Client::request",
                "Expose the request through the client's typed operation.",
            ),
            (
                "crates/verse-world/src/service/worker.rs",
                "verse_world::service::worker",
                "Connect the operation to the server worker.",
            ),
        ],
    },
    Seed {
        id: "trace-step",
        capability: "agent-history",
        operation: "extend-trace-step",
        description: "Extend an ATIF trace step and keep exported documents and readable history compatible.",
        anchors: &[
            (
                "crates/atif/src/document.rs",
                "atif::document::Step",
                "Use the shared serializable step contract.",
            ),
            (
                "crates/atif/src/document.rs",
                "atif::document::document_at",
                "Keep exported trajectory fields consistent.",
            ),
            (
                "crates/coder-history/src/project.rs",
                "coder_history::project::readable_record_full",
                "Update the readable projection without replacing raw history.",
            ),
        ],
    },
    Seed {
        id: "history-provider",
        capability: "agent-history",
        operation: "extend-history-provider",
        description: "Extend saved history support through explicit roots, catalog pages, and complete record projection.",
        anchors: &[
            (
                "crates/coder-history/src/lib.rs",
                "coder_history::Harness",
                "Describe the provider in portable history data.",
            ),
            (
                "crates/coder-history/src/host.rs",
                "coder_history::host::Config",
                "Add only explicitly selected provider roots.",
            ),
            (
                "crates/coder-history/src/project.rs",
                "coder_history::project::readable_record_full",
                "Keep unknown records and all raw source bytes accessible.",
            ),
        ],
    },
];

pub fn compile(context: &Context) -> Result<Artifact, String> {
    let mut recipes = Vec::new();
    for seed in SEEDS {
        let mut roles = Vec::new();
        let mut packages = std::collections::BTreeSet::new();
        for (i, (path, symbol, purpose)) in seed.anchors.iter().enumerate() {
            let reference = context
                .reference(path, Some(symbol), 0, 0)
                .map_err(|e| format!("{}: {e}", seed.id))?;
            if let Some(package) = path
                .strip_prefix("crates/")
                .and_then(|s| s.split('/').next())
            {
                packages.insert(package);
            }
            roles.push(Role {
                id: format!("role-{i}"),
                purpose: (*purpose).into(),
                required: true,
                when: None,
                references: vec![reference],
            });
        }
        let mut aliases = vec![seed.description.into(), seed.operation.replace('-', " ")];
        if seed.id == "model-pack" {
            aliases.extend([
                "register 3d model".into(),
                "import meshes".into(),
                "add model to asset pack".into(),
            ]);
        }
        let mut exclusions = Vec::new();
        if seed.capability.starts_with("3d") {
            exclusions.push("inference model registration".into());
        }
        recipes.push(Recipe {dependency_scopes:if seed.capability.starts_with("3d") {vec!["docs/verse".into()]} else {vec![]},id:seed.id.into(),capability:seed.capability.into(),operation:seed.operation.into(),description:seed.description.into(),aliases,exclusions,conditions:vec![],examples:vec![roles[0].references[0].clone()],roles,new_files:vec![],
            checks:packages.into_iter().map(|p|Check{package:p.into(),manifest:format!("crates/{p}/Cargo.toml"),filter:None,establishes:"Run the affected crate's tests; add a behavior test for the requested change.".into()}).collect(),evidence:EvidenceClass::SourceReviewed,
            limitations:vec!["Source-reviewed starting points; required behavior and additional consumers must be checked for the specific change. No cheaper-agent edit result has been measured for this recipe.".into()]});
    }
    context.verify(&Artifact {
        schema: "openagents.task-index.v1".into(),
        snapshot: context.snapshot.clone(),
        recipes,
    })
}
