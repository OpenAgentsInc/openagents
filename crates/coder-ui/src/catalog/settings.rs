//! Inert Coder settings, selection, and disclosure presentation fixtures.

use rust_native::style::{Color, TextAlign, TextWeight, Viewport};
use rust_native::{Axis, Element, Node, RichRun, TextRole};
use unicode_segmentation::UnicodeSegmentation;

use super::{CatalogEntry, CatalogIntent, CatalogVariant, FixtureState, SourceRef};

use crate::source_theme::{
    ACCENT_MODEL as CYAN, ACCENT_SUCCESS as GREEN, BG_BASE as BASE, BG_DARK as DARK,
    BG_LIGHT as LIGHT, COMMAND as AMBER, DIFF_DELETE_FG as RED, GRAY, GRAY_BRIGHT as BRIGHT,
    PROMPT_BORDER_ACTIVE as BORDER, TEXT_PRIMARY as PRIMARY, TEXT_SECONDARY as SECONDARY,
};
const ROUTER_ENDPOINT: &str = "https://openrouter.ai/api/v1";
const TYPESAFE_ENDPOINT: &str = "https://api.typesafe.ai";
const VERCEL_ENDPOINT: &str = "https://ai-gateway.vercel.sh/typesafe";
const BRAINSTORM_ORIGIN: &str = "https://api.brainstorm.world";
const CLOUD_ERROR: &str =
    "Invalid cloud execution mode, size, template, credential names, or workspace paths.";

#[derive(Clone, Copy)]
struct Plugin {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    enabled: bool,
}

const PLUGINS: [Plugin; 8] = [
    Plugin {
        id: "openrouter-byok",
        name: "OpenRouter BYOK",
        description: "Use OpenRouter models with your own API key.",
        enabled: false,
    },
    Plugin {
        id: "microcoder",
        name: "Coder loop",
        description: "Coder's own coding loop, with local model logins and bounded commands.",
        enabled: true,
    },
    Plugin {
        id: "jev",
        name: "Jev",
        description: "Typed decisions through the Jev SDK. Connect TypeSafe or a compatible gateway.",
        enabled: true,
    },
    Plugin {
        id: "openagents-cli",
        name: "OpenAgents CLI",
        description: "Discover and call the bundled openagents command in this working directory.",
        enabled: true,
    },
    Plugin {
        id: "acp-subagents",
        name: "ACP Subagents",
        description: "Define named ACP agents and delegate tasks to their registered executables.",
        enabled: true,
    },
    Plugin {
        id: "brainstorm",
        name: "Brainstorm",
        description: "Explicit public profile and reputation lookups through the Brainstorm house perspective.",
        enabled: false,
    },
    Plugin {
        id: "boat-cloud",
        name: "Boat Cloud",
        description: "Delegate through Boat integrated agents or headless Coder, with saved workspaces and usage.",
        enabled: false,
    },
    Plugin {
        id: "gce-cloud",
        name: "GCE Cloud",
        description: "Run headless Coder in the granted GCE pool, with reconnectable jobs and estimated usage.",
        enabled: false,
    },
];

/// The settings catalog pins every source branch without invoking its runtime.
pub(super) fn entries() -> Vec<CatalogEntry> {
    vec![
        entry(
            "plugins.row",
            "Plugins",
            "Plugin row",
            "One selectable plugin registration, enable state, and colored status.",
            "ui/plugins.rs",
            "manager",
            &[
                "openrouter-byok",
                "microcoder",
                "jev",
                "openagents-cli",
                "acp-subagents",
                "brainstorm",
                "boat-cloud",
                "gce-cloud",
                "disabled",
                "enabled",
                "checking",
                "verified",
                "unavailable",
                "unselected",
                "narrow",
            ],
        ),
        entry(
            "plugin.status",
            "Plugins",
            "Plugin status",
            "The exact neutral, amber, red, and green status roles, including storage failure.",
            "ui/plugins.rs",
            "status_color",
            &[
                "disabled",
                "setup-required",
                "configured",
                "checking",
                "verified",
                "unavailable",
                "demo",
                "expired",
                "all-agents-off",
                "no-agents",
                "storage-error",
            ],
        ),
        entry(
            "settings.fields",
            "Settings",
            "Framed settings fields",
            "Native editable fields with source focus colors, masked secret drafts, and bounded long values.",
            "ui/plugins.rs",
            "field",
            &[
                "focused",
                "unfocused",
                "secret",
                "masking",
                "long-secret",
                "long-value",
                "invalid",
                "disabled",
                "narrow",
            ],
        ),
        entry(
            "settings.actions",
            "Settings",
            "Settings actions",
            "Unframed source actions with selection, action role, and disabled state.",
            "ui/plugins.rs",
            "action",
            &[
                "selected",
                "unselected",
                "ordinary",
                "test",
                "save",
                "remove",
                "destructive",
                "cancel",
                "disabled",
            ],
        ),
        entry(
            "models.choice",
            "Pickers",
            "Model choice row",
            "A selected or unselected model, reasoning, or output row with its source description.",
            "ui/models.rs",
            "render_choice / choices",
            &[
                "current",
                "unselected",
                "reasoning",
                "reasoning-active",
                "output",
                "output-active",
                "long-label",
                "narrow",
            ],
        ),
        entry(
            "models.details",
            "Pickers",
            "Model capability details",
            "Provider and exact model identity with known limits or the immutable catalog description.",
            "ui/models.rs",
            "details",
            &["unknown", "metadata", "output-limited", "narrow"],
        ),
        entry(
            "models.search",
            "Pickers",
            "Model search field",
            "A single-line native search field; text and caret remain local to the owning fixture.",
            "ui/models.rs",
            "search",
            &["empty", "search", "long-search", "narrow"],
        ),
        entry(
            "sessions.row",
            "Sessions",
            "Saved conversation row",
            "A numbered selected row with bounded age, entry count, ID, and working folder.",
            "ui/resume.rs",
            "render",
            &[
                "selected",
                "unselected",
                "just-now",
                "minutes",
                "hours",
                "days",
                "long-title",
                "narrow",
            ],
        ),
        entry(
            "plugins.manager",
            "Plugins",
            "Plugin manager",
            "Eight host-supported plugins, their enable switches, connection states, and selected details.",
            "ui/plugins.rs",
            "manager",
            &[
                "default",
                "openrouter-byok",
                "microcoder",
                "jev",
                "openagents-cli",
                "acp-subagents",
                "brainstorm",
                "boat-cloud",
                "gce-cloud",
                "configured",
                "checking",
                "verified",
                "unavailable",
                "demo",
                "expired",
                "all-agents-off",
                "storage-error",
                "narrow",
                "short",
            ],
        ),
        entry(
            "plugins.details",
            "Plugins",
            "Plugin details and information",
            "Read-only details for all eight plugins, including the two information-only screens.",
            "ui/plugins.rs",
            "plugin_details / plugin_info",
            &[
                "openrouter-byok",
                "microcoder",
                "jev",
                "openagents-cli",
                "acp-subagents",
                "brainstorm",
                "boat-cloud",
                "gce-cloud",
                "key-added",
                "demo-key-added",
                "reasoning",
                "storage-error",
                "narrow",
            ],
        ),
        entry(
            "settings.openrouter",
            "Settings",
            "OpenRouter connection settings",
            "Masked key editing, model ID, explicit connection test, staged key removal, and save or cancel.",
            "ui/plugins.rs",
            "router_settings",
            &[
                "default",
                "key-added",
                "key-draft",
                "key-remove",
                "checking",
                "verified",
                "failed",
                "demo",
                "memory-key",
                "storage-error",
                "invalid-key",
                "long-model",
                "focus-key",
                "focus-model",
                "focus-test",
                "focus-save",
                "focus-remove",
                "focus-cancel",
                "narrow",
                "short",
            ],
        ),
        entry(
            "settings.jev",
            "Settings",
            "Jev gateway settings",
            "TypeSafe, Vercel, and custom gateways with origin-bound masked credentials and explicit validation.",
            "ui/plugins.rs",
            "jev_settings",
            &[
                "default",
                "typesafe",
                "vercel",
                "custom",
                "key-added",
                "key-draft",
                "key-remove",
                "changed-origin",
                "checking",
                "verified",
                "failed",
                "demo",
                "storage-error",
                "invalid-endpoint",
                "invalid-model",
                "invalid-key",
                "focus-gateway",
                "focus-endpoint",
                "focus-key",
                "focus-model",
                "focus-test",
                "focus-save",
                "focus-remove",
                "focus-cancel",
                "narrow",
                "short",
            ],
        ),
        entry(
            "settings.acp",
            "Settings",
            "ACP agent picker",
            "Detected agent checkboxes, bounded selection, refresh, and empty or storage failure states.",
            "ui/plugins.rs",
            "acp_settings",
            &[
                "detected",
                "empty",
                "all-off",
                "selected-off",
                "many",
                "last-selected",
                "storage-error",
                "narrow",
                "short",
            ],
        ),
        entry(
            "settings.brainstorm",
            "Settings",
            "Brainstorm house settings",
            "Explicit public discovery for one saved HTTPS recipient, with unsigned house observations and expiry.",
            "ui/plugins.rs",
            "brainstorm_settings",
            &[
                "default",
                "demo",
                "configured",
                "changed-recipient",
                "checking",
                "verified",
                "discovery",
                "expired",
                "failed",
                "invalid-origin",
                "storage-error",
                "focus-origin",
                "focus-test",
                "focus-save",
                "focus-cancel",
                "narrow",
                "short",
            ],
        ),
        entry(
            "settings.boat",
            "Settings",
            "Boat Cloud settings",
            "Integrated or Coder mode, machine size, optional template, named credentials, and saved workspace paths.",
            "ui/plugins.rs",
            "cloud_settings",
            &[
                "default",
                "integrated",
                "coder",
                "small",
                "large",
                "xlarge",
                "template",
                "credentials",
                "workspaces",
                "invalid-template",
                "invalid-credentials",
                "invalid-paths",
                "storage-error",
                "focus-mode",
                "focus-size",
                "focus-template",
                "focus-credentials",
                "focus-paths",
                "focus-save",
                "focus-cancel",
                "narrow",
                "short",
            ],
        ),
        entry(
            "settings.gce",
            "Settings",
            "GCE Cloud settings",
            "Fixed Coder runtime and granted pool shape with credential names and saved workspace paths.",
            "ui/plugins.rs",
            "cloud_settings",
            &[
                "default",
                "fixed-pool",
                "credentials",
                "workspaces",
                "invalid-credentials",
                "invalid-paths",
                "storage-error",
                "focus-credentials",
                "focus-paths",
                "focus-save",
                "focus-cancel",
                "narrow",
                "short",
            ],
        ),
        entry(
            "models.picker",
            "Pickers",
            "Model and generation picker",
            "Search, capability refresh, reasoning and output stages, current choices, and known catalog fallback.",
            "ui/models.rs",
            "render / choices / details",
            &[
                "models",
                "search",
                "empty",
                "loading",
                "error",
                "error-while-loading",
                "fallback",
                "metadata",
                "reasoning",
                "reasoning-all",
                "reasoning-required",
                "reasoning-active",
                "output",
                "output-limited",
                "output-active",
                "no-reasoning",
                "no-output",
                "no-options",
                "details-changed",
                "disabled-provider",
                "long-search",
                "long-label",
                "narrow",
                "short",
                "tiny",
                "closed",
            ],
        ),
        entry(
            "sessions.resume",
            "Sessions",
            "Resume recent conversations",
            "A bounded, numbered session picker with age, entry count, ID, working folder, and refusal notices.",
            "ui/resume.rs",
            "render",
            &[
                "recent",
                "empty",
                "many",
                "last-selected",
                "busy",
                "active-writer",
                "storage-unavailable",
                "invalid-id",
                "snapshot-error",
                "child-error",
                "save-error",
                "already-open",
                "cwd-mismatch",
                "narrow",
                "short",
            ],
        ),
        entry(
            "sessions.follow",
            "Sessions",
            "Follow and take over",
            "Passive following, manual scroll, pending takeover, acquisition, and agent reclaim; all synthetic.",
            "resume.rs",
            "follow_tick / follow_key / answer_reclaim",
            &[
                "following",
                "scrolled",
                "pending-takeover",
                "acquired",
                "agent-reclaim",
                "busy-reclaim",
                "narrow",
            ],
        ),
        entry(
            "approvals.disclosure",
            "Approvals",
            "Exact Brainstorm disclosure",
            "The exact lookup and recipient review, with confirmation gated on reaching the end.",
            "ui.rs",
            "render disclosure branch",
            &[
                "search",
                "rank",
                "long",
                "unreviewed",
                "reviewed",
                "confirmed",
                "rejected",
                "cancelled",
                "closed",
                "expired",
                "narrow",
                "short",
            ],
        ),
    ]
}

fn entry(
    id: &str,
    family: &str,
    title: &str,
    description: &str,
    path: &str,
    symbol: &str,
    variants: &[&str],
) -> CatalogEntry {
    CatalogEntry {
        id: id.into(),
        family: family.into(),
        title: title.into(),
        description: description.into(),
        sources: variants
            .iter()
            .flat_map(|variant| {
                symbol.split('/').map(move |part| SourceRef {
                    path: format!("crates/coder-new/src/{path}"),
                    symbol: part.trim().into(),
                    branch: format!("fixture:{variant}; {}", source_branch(id, variant)),
                })
            })
            .chain(supporting_sources(id))
            .collect(),
        variants: variants
            .iter()
            .map(|id| CatalogVariant {
                id: (*id).into(),
                label: id.replace('-', " "),
            })
            .collect(),
    }
}

fn source_branch(id: &str, variant: &str) -> String {
    match variant {
        "narrow" => "width < 40 for compact details; manager width < 28; model descriptions hidden; framed fields keep their focused segment".into(),
        "short" => "bounded viewport clips content and keeps the selected row or focused field visible".into(),
        "tiny" => "model picker returns before rendering when width < 4 or height < 4".into(),
        "checking" => "Connection::Checking; checking status and spinner".into(),
        "verified" => "Connection::Verified; success status; Brainstorm discovery details when present".into(),
        "failed" | "unavailable" => "Connection::Failed; unavailable status and error copy".into(),
        "configured" => "enabled and configured, without a verified connection".into(),
        "demo" => "preview mode connection label; Brainstorm fixture branch makes no service read".into(),
        "expired" if id == "approvals.disclosure" => "disclosure desk closes the pending event after expiry; closed fixture state".into(),
        "expired" => "Brainstorm discovery expiry is not later than the current fixture time".into(),
        "storage-error" => "storage_error is present; error text retains previous saved settings".into(),
        "key-added" | "memory-key" => "configured saved key and empty replacement draft; memory storage label when persistence is unavailable".into(),
        "key-draft" => "nonempty replacement key draft; hidden key label and masked editing".into(),
        "key-remove" => "pending removal and empty draft; removal takes effect only on save".into(),
        "changed-origin" => "gateway origin differs; discard replacement key and require a key for the new origin or explicit removal".into(),
        "changed-recipient" => "Brainstorm draft origin differs from saved recipient; discovery refuses until save".into(),
        "invalid-key" | "invalid-endpoint" | "invalid-model" | "invalid-origin" | "invalid-template" | "invalid-credentials" | "invalid-paths" => format!("save validation refuses {}; retain the prior saved configuration", variant.trim_start_matches("invalid-")),
        "empty" if id == "models.picker" => "matching model rows are empty; no selected model details".into(),
        "empty" if id == "settings.acp" => "detected agents are empty; installation and refresh hint replaces the list".into(),
        "empty" => "session choices are empty; no conversations to resume".into(),
        "all-off" | "all-agents-off" => "detected ACP choices exist, but no enabled executable is registered".into(),
        "selected-off" => "selected ACP checkbox is off; selected row remains visible".into(),
        "many" | "last-selected" => "bounded list pages and scrolls to a selected row beyond the first page".into(),
        "typesafe" | "vercel" | "custom" => format!("gateway_label selects the {variant} gateway endpoint and model default"),
        "search" | "long-search" if id == "models.picker" => "Models stage search filters immutable known names, IDs, and descriptions; focused query segment remains visible".into(),
        "loading" => "loading is true and error is absent; refreshing details spinner precedes known model choices".into(),
        "error" | "error-while-loading" | "fallback" => "error takes precedence over loading; retain the known model choices after partial metadata failure".into(),
        "metadata" => "selected model has public context and output metadata; render limit details instead of description".into(),
        "reasoning" | "reasoning-all" | "reasoning-required" | "reasoning-active" => "Reasoning stage; model default plus supported efforts, mandatory reasoning excludes none, active option is annotated".into(),
        "output" | "output-limited" | "output-active" => "Output stage; model default plus supported token limits bounded by provider maximum and 32768; active option is annotated".into(),
        "no-reasoning" | "no-output" | "no-options" => "selection skips stages absent from the selected model's capabilities".into(),
        "details-changed" => "capabilities changed for pending selection; reset to Models and require reselection".into(),
        "disabled-provider" => "OpenRouter plugin disabled; model picker refuses to open and leaves a notice".into(),
        "long-label" => "choice name exceeds the available label width; description keeps its bounded right column".into(),
        "busy" | "active-writer" | "storage-unavailable" | "invalid-id" | "snapshot-error" | "child-error" | "save-error" => format!("resume refusal for {variant}; keep the current conversation and lease until complete restoration succeeds"),
        "already-open" => "requested session is already active; select main and show notice".into(),
        "cwd-mismatch" => "saved working folder differs; resume into current working folder with explicit notice".into(),
        "following" | "scrolled" => "passive follow; stick controls newest-entry tracking without execution authority".into(),
        "pending-takeover" | "acquired" => "follow_key requests takeover; follow_tick acquires only after the holder releases its lease".into(),
        "agent-reclaim" | "busy-reclaim" => "idle holder yields and preserves draft; busy holder finishes its current reply before yielding".into(),
        "search" | "rank" | "long" | "unreviewed" | "reviewed" if id == "approvals.disclosure" => "render exact recipient and input; confirmation is allowed only after complete review".into(),
        "confirmed" | "rejected" | "cancelled" | "closed" => "disclosure or picker closes after its explicit answer; inert saved result fixture".into(),
        _ if variant.starts_with("focus-") => format!("focus is {}; keep that native field or action visible after resize", variant.trim_start_matches("focus-")),
        _ if PLUGINS.iter().any(|plugin| plugin.id == variant) => format!("selected plugin is {variant}; render its registered detail branch"),
        _ => format!("{id} renders the {variant} source configuration with synthetic values"),
    }
}

fn supporting_sources(id: &str) -> Vec<SourceRef> {
    let refs: &[(&str, &str, &str)] = match id {
        "plugins.manager" | "plugins.details" => &[
            (
                "plugin_definition.rs",
                "DEFINITIONS",
                "eight supported host bindings and default enable states",
            ),
            (
                "plugins.rs",
                "Plugins::status_for",
                "enabled, credential, connection, and ACP registration status",
            ),
        ],
        "settings.openrouter" => &[
            (
                "plugins.rs",
                "Plugins::save / key_label / restore_connection",
                "masked replacement, staged removal, and cancellation preserve saved settings",
            ),
            (
                "plugin_store.rs",
                "Store",
                "bounded saved settings; write failure retains the previous configuration",
            ),
        ],
        "settings.jev" => &[
            (
                "bundled_settings.rs",
                "save / gateway_label / key_label",
                "endpoint and model validation; keys remain bound to their origin",
            ),
            (
                "jev_plugin.rs",
                "DEFAULT_ENDPOINT / GATEWAY_ENDPOINT / default_model",
                "known gateway endpoints and model defaults",
            ),
        ],
        "settings.acp" => &[
            (
                "bundled_settings.rs",
                "begin_acp / toggle_acp_agent / acp_choices",
                "enabled choices and selected identity survive refresh",
            ),
            (
                "acp_discovery.rs",
                "catalog / discover",
                "metadata-only detection; unavailable executables omitted",
            ),
        ],
        "settings.brainstorm" => &[(
            "brainstorm.rs",
            "Settings / status",
            "saved recipient, explicit discovery, cancellation, and expiry",
        )],
        "settings.boat" | "settings.gce" => &[(
            "cloud_settings.rs",
            "Configuration::valid / Editor::key / Editor::value",
            "typed fields, named credentials, fixed GCE settings, and focused row scrolling",
        )],
        "models.picker" => &[
            (
                "models.rs",
                "Picker::select / back / refresh",
                "generation stages and capability-dependent transitions",
            ),
            (
                "model_catalog.rs",
                "Loader::sync / REFRESH_ERROR",
                "public metadata refresh preserves known identities on failure",
            ),
        ],
        "sessions.resume" => &[(
            "resume.rs",
            "App::resume",
            "busy, writer, snapshot, child restoration, and current folder check",
        )],
        "sessions.follow" => &[(
            "resume.rs",
            "follow_tick / follow_key / answer_reclaim",
            "holder release governs acquisition; unsent draft survives reclaim",
        )],
        "approvals.disclosure" => &[(
            "lib.rs",
            "poll_disclosure / answer_disclosure / key",
            "request-bound answer; closed desk events remove pending input; complete review gates Y",
        )],
        _ => &[],
    };
    refs.iter()
        .map(|(path, symbol, branch)| SourceRef {
            path: format!("crates/coder-new/src/{path}"),
            symbol: (*symbol).into(),
            branch: (*branch).into(),
        })
        .collect()
}

pub(super) fn defaults(state: &mut FixtureState) {
    if !owns(&state.component) {
        return;
    }
    let variant = state.variant.clone();
    for plugin in PLUGINS {
        state
            .flags
            .insert(format!("enabled:{}", plugin.id), plugin.enabled);
        state
            .flags
            .insert(format!("key-configured:{}", plugin.id), false);
        put(state, &format!("connection:{}", plugin.id), "unchecked");
    }
    put(state, "connection", "unchecked");
    put(state, "model", "openrouter/free");
    put(state, "jev-model", "jev-latest");
    put(state, "endpoint", TYPESAFE_ENDPOINT);
    put(state, "gateway", "TypeSafe direct");
    put(state, "origin", BRAINSTORM_ORIGIN);
    put(state, "saved-origin", BRAINSTORM_ORIGIN);
    put(state, "key-mask", "");
    put(state, "key-help", "");
    let mode = if state.component == "settings.gce" {
        "Coder runtime"
    } else {
        "Integrated agent"
    };
    put(state, "mode", mode);
    put(state, "size", "default");
    put(state, "template", "");
    put(state, "credentials", "");
    put(state, "paths", "");
    let focus = default_focus(&state.component);
    put(state, "focus", focus);
    put(state, "model-search", "");
    put(state, "active-model", "openrouter/free");
    put(state, "pending-model", "openai/gpt-6-luna");
    put(state, "reasoning", "");
    put(state, "output", "");
    put(state, "error", "");
    put(state, "storage-error", "");
    put(state, "editor-kind", "");
    put(state, "acp-count", "11");
    put(state, "resume-count", "5");
    state.stage = "models".into();
    state.flags.insert("output-supported".into(), true);
    state.flags.insert("reasoning-supported".into(), true);
    state.flags.insert("stick-to-end".into(), true);
    for index in 0..32 {
        state.flags.insert(format!("acp:{index}"), true);
    }

    if let Some(index) = PLUGINS.iter().position(|p| p.id == variant) {
        state.selected = index;
    }
    match variant.as_str() {
        "narrow" => { state.width = 24; state.height = 20; }
        "short" => { state.width = 80; state.height = 10; }
        "tiny" => { state.width = 3; state.height = 3; }
        "configured" | "key-added" | "memory-key" => { state.flags.insert("key-configured".into(), true); put(state, "connection", "configured"); }
        "key-draft" => put(state, "key-mask", "••••••••••••••••••••••••"),
        "key-remove" => { state.flags.insert("key-configured".into(), true); state.flags.insert("remove-key".into(), true); }
        "changed-origin" => { state.flags.insert("key-configured".into(), true); state.flags.insert("changed-origin".into(), true); put(state, "gateway", "Custom gateway"); put(state, "endpoint", "https://decisions.example.test"); }
        "checking" => put(state, "connection", "checking"),
        "verified" => { put(state, "connection", "verified"); state.flags.insert("key-configured".into(), true); }
        "unavailable" | "failed" => { put(state, "connection", "failed"); put(state, "error", "The configured service is unavailable."); }
        "demo" => put(state, "connection", "demo"),
        "demo-key-added" => { put(state, "connection", "demo"); state.flags.insert("key-configured".into(), true); }
        "expired" => put(state, "connection", "expired"),
        "storage-error" => put(state, "storage-error", "Couldn't save Coder plugin settings. The previous settings remain in use."),
        "invalid-key" => { put(state, "error", "The API key cannot contain spaces or control characters."); put(state, "key-mask", "••••••••••••"); state.flags.insert("key-invalid".into(), true); }
        "invalid-endpoint" => { put(state, "endpoint", "http://decisions.example.test/private?key=redacted"); put(state, "error", "Enter an HTTPS API base URL without credentials, query, or fragment."); }
        "invalid-model" => { put(state, "jev-model", "invalid model"); put(state, "error", "Enter a valid Jev model ID using at most 128 bytes."); }
        "long-model" => put(state, "model", "example-provider/a-long-text-model-name-for-horizontal-field-review"),
        "vercel" => { put(state, "gateway", "Vercel AI Gateway"); put(state, "endpoint", VERCEL_ENDPOINT); put(state, "jev-model", "typesafe-ai/jev"); }
        "custom" => { put(state, "gateway", "Custom gateway"); put(state, "endpoint", "https://decisions.example.test"); }
        "empty" if state.component == "settings.acp" => put(state, "acp-count", "0"),
        "all-off" | "all-agents-off" => { for index in 0..32 { state.flags.insert(format!("acp:{index}"), false); } }
        "selected-off" => { state.selected = 3; state.flags.insert("acp:3".into(), false); }
        "many" if state.component == "settings.acp" => put(state, "acp-count", "24"),
        "last-selected" if state.component == "settings.acp" => { put(state, "acp-count", "24"); state.selected = 23; }
        "changed-recipient" => put(state, "origin", "https://house.example.test"),
        "discovery" => put(state, "connection", "verified"),
        "invalid-origin" => { put(state, "origin", "http://house.example.test/path"); put(state, "error", "Enter an HTTPS origin without credentials, a path prefix, query, or fragment."); }
        "coder" => put(state, "mode", "Coder runtime"),
        "small" | "large" | "xlarge" => put(state, "size", &variant),
        "template" => put(state, "template", "coder-fixture-template"),
        "credentials" => put(state, "credentials", "OPENROUTER_API_KEY, TYPESAFE_API_KEY"),
        "workspaces" => put(state, "paths", "workspace/project, workspace/notes"),
        "invalid-template" => { put(state, "template", &"x".repeat(129)); put(state, "error", CLOUD_ERROR); }
        "invalid-credentials" => { put(state, "credentials", "INVALID NAME"); put(state, "error", CLOUD_ERROR); }
        "invalid-paths" => { put(state, "paths", "../../outside-workspace"); put(state, "error", CLOUD_ERROR); }
        "search" => put(state, "model-search", "OpenAI"),
        "empty" if state.component == "models.picker" => put(state, "model-search", "no-matching-fixture-model"),
        "loading" => { state.flags.insert("model-loading".into(), true); }
        "error" | "error-while-loading" | "fallback" => { put(state, "error", "Some model details could not refresh. Showing the known model choices."); state.flags.insert("model-loading".into(), variant == "error-while-loading"); }
        "metadata" => { state.flags.insert("model-metadata".into(), true); state.selected = 1; }
        "reasoning" | "reasoning-all" | "reasoning-required" | "reasoning-active" => { state.stage = "reasoning".into(); if variant == "reasoning-all" { state.flags.insert("all-efforts".into(), true); } if variant == "reasoning-required" { put(state, "pending-model", "openai/gpt-6.1-sol"); } if variant == "reasoning-active" { put(state, "active-model", "openai/gpt-6-luna"); state.selected = 3; } }
        "output" | "output-limited" | "output-active" => { state.stage = "output".into(); if variant == "output-limited" { put(state, "max-output", "8192"); } if variant == "output-active" { put(state, "active-model", "openai/gpt-6-luna"); state.selected = 2; } }
        "no-reasoning" => { state.flags.insert("reasoning-supported".into(), false); put(state, "pending-model", "openrouter/free"); }
        "no-output" => { state.flags.insert("output-supported".into(), false); state.selected = 1; }
        "no-options" => { state.flags.insert("reasoning-supported".into(), false); state.flags.insert("output-supported".into(), false); }
        "details-changed" => put(state, "error", "Model details changed. Select the model again."),
        "disabled-provider" => { state.stage = "closed".into(); state.notice = "Turn on OpenRouter BYOK from /plugins before choosing its models.".into(); }
        "long-search" => put(state, "model-search", "A long synthetic search query that keeps the caret's segment visible"),
        "long-label" => { state.flags.insert("long-label".into(), true); }
        "closed" => { state.stage = "closed".into(); state.notice = "The fixture is closed.".into(); }
        "empty" if state.component == "sessions.resume" => put(state, "resume-count", "0"),
        "many" if state.component == "sessions.resume" => put(state, "resume-count", "24"),
        "last-selected" if state.component == "sessions.resume" => { put(state, "resume-count", "24"); state.selected = 23; }
        "busy" => put(state, "error", "Stop the current work with Esc before resuming a conversation."),
        "active-writer" => put(state, "error", "Another process is using this chat session."),
        "storage-unavailable" => put(state, "error", "Conversation storage is unavailable."),
        "invalid-id" => put(state, "error", "Choose a number from the last /resume list, or use a session ID."),
        "snapshot-error" => put(state, "error", "Couldn't read the saved conversation."),
        "child-error" => put(state, "error", "Couldn't restore a saved child conversation."),
        "save-error" => put(state, "error", "Can't save this conversation."),
        "already-open" => state.notice = "Conversation fixture-session-1 is already open.".into(),
        "cwd-mismatch" => state.notice = "Resumed fixture-session-1. Saved in workspace/previous; continuing in workspace/current.".into(),
        "scrolled" => { state.scroll = 3; state.flags.insert("stick-to-end".into(), false); }
        "pending-takeover" => { state.flags.insert("takeover-pending".into(), true); state.notice = "Taking over: the conversation is yours as soon as its agent stops.".into(); }
        "acquired" => { state.flags.insert("takeover-acquired".into(), true); state.notice = "Conversation fixture-session-1 is yours now.".into(); }
        "agent-reclaim" => state.notice = "Your agent took the conversation back to answer you.".into(),
        "busy-reclaim" => { state.flags.insert("reclaim-busy".into(), true); state.notice = "The current reply finishes before the fixture agent can reclaim this conversation.".into(); }
        "reviewed" => { state.flags.insert("reviewed".into(), true); state.scroll = 30; }
        "confirmed" | "rejected" | "cancelled" => { state.stage = variant.clone(); state.flags.insert("reviewed".into(), true); state.notice = format!("Disclosure {}.", variant); }
        _ => {}
    }
    if let Some(focus) = variant.strip_prefix("focus-") {
        put(state, "focus", focus);
    }
    if variant == "reasoning-active" {
        put(state, "reasoning", "medium");
    }
    if variant == "output-active" {
        put(state, "output", "4096");
    }
    if state.component == "plugins.manager"
        && matches!(
            variant.as_str(),
            "configured" | "checking" | "verified" | "unavailable" | "demo" | "expired"
        )
    {
        state.flags.insert("enabled:openrouter-byok".into(), true);
        if matches!(variant.as_str(), "demo" | "expired") {
            state.selected = 5;
            state.flags.insert("enabled:brainstorm".into(), true);
        }
    }
    if state.component == "settings.brainstorm"
        && matches!(variant.as_str(), "verified" | "discovery" | "expired")
    {
        put(state, "house-key", &"f".repeat(64));
        put(state, "discovered-at", "1791432000000");
    }
    if state.component == "settings.brainstorm" && variant != "default" {
        state.flags.insert("enabled:brainstorm".into(), true);
    }
    if state.component == "plugins.details" && variant == "reasoning" {
        put(state, "reasoning", "high");
    }
    if state.component == "approvals.disclosure" && variant == "expired" {
        state.stage = "expired".into();
        state.notice = "This disclosure request expired.".into();
    }
    snapshot_settings(state);
    if variant == "changed-origin" {
        put(state, "saved.endpoint", TYPESAFE_ENDPOINT);
        put(state, "saved.gateway", "TypeSafe direct");
    }
    if variant == "changed-recipient" || variant == "invalid-origin" {
        put(state, "saved.origin", BRAINSTORM_ORIGIN);
    }
    if variant == "invalid-endpoint" {
        put(state, "saved.endpoint", TYPESAFE_ENDPOINT);
    }
    if variant == "invalid-model" {
        put(state, "saved.jev-model", "jev-latest");
    }
    if matches!(
        variant.as_str(),
        "invalid-template" | "invalid-credentials" | "invalid-paths"
    ) {
        put(state, "saved.template", "");
        put(state, "saved.credentials", "");
        put(state, "saved.paths", "");
    }
    if matches!(
        state.component.as_str(),
        "plugins.manager" | "plugins.details" | "plugins.row"
    ) && !matches!(variant.as_str(), "acp-subagents" | "all-agents-off")
    {
        put(state, "acp-count", "0");
    }
    if state.component == "settings.fields"
        && matches!(variant.as_str(), "secret" | "masking" | "long-secret")
    {
        put(
            state,
            "key-mask",
            &"•".repeat(if variant == "long-secret" {
                180
            } else if variant == "masking" {
                3
            } else {
                12
            }),
        );
    }
    if state.component == "plugins.row"
        && matches!(
            variant.as_str(),
            "enabled" | "configured" | "checking" | "verified" | "unavailable"
        )
    {
        let key = format!("enabled:{}", selected_plugin(state).id);
        state.flags.insert(key, true);
    }
    if matches!(
        state.component.as_str(),
        "plugins.manager" | "plugins.details" | "plugins.row"
    ) {
        let plugin = selected_plugin(state).id;
        put(state, "connection-plugin", plugin);
        retain_plugin_connection(state);
    }
}

fn default_focus(component: &str) -> &'static str {
    match component {
        "settings.jev" => "key",
        "settings.brainstorm" => "origin",
        "settings.boat" | "settings.gce" => "mode",
        _ => "key",
    }
}

fn owns(id: &str) -> bool {
    matches!(
        id,
        "plugins.row"
            | "plugin.status"
            | "settings.fields"
            | "settings.actions"
            | "models.choice"
            | "models.details"
            | "models.search"
            | "sessions.row"
            | "plugins.manager"
            | "plugins.details"
            | "settings.openrouter"
            | "settings.jev"
            | "settings.acp"
            | "settings.brainstorm"
            | "settings.boat"
            | "settings.gce"
            | "models.picker"
            | "sessions.resume"
            | "sessions.follow"
            | "approvals.disclosure"
    )
}

pub(super) fn render(
    id: &str,
    _variant: &str,
    state: &FixtureState,
) -> Option<Node<CatalogIntent>> {
    match id {
        "plugins.row" => {
            let index = state.selected.min(7);
            return Some(plugin_row(index, state, state.variant != "unselected"));
        }
        "plugin.status" => {
            let status = match state.variant.as_str() {
                "disabled" => "Disabled",
                "setup-required" => "Setup required",
                "configured" => "Configured",
                "checking" => "Checking",
                "verified" => "Verified",
                "unavailable" => "Unavailable",
                "demo" => "Demo fixture",
                "expired" => "Expired",
                "all-agents-off" => "All agents off",
                "no-agents" => "No agents detected",
                "storage-error" => get(state, "storage-error"),
                _ => "Enabled",
            };
            return Some(text(
                "plugin-status",
                status,
                if state.variant == "storage-error" {
                    RED
                } else {
                    status_color(status)
                },
            ));
        }
        "settings.fields" => return Some(isolated_field(state)),
        "settings.actions" => {
            let (label, name, color) = match state.variant.as_str() {
                "test" => ("Test API key", "settings.test", CYAN),
                "remove" | "destructive" => ("Remove API key", "settings.remove-key", RED),
                "cancel" => ("Cancel (Esc)", "settings.cancel", SECONDARY),
                _ => ("Save settings", "settings.save", CYAN),
            };
            return Some(action(
                "settings-action",
                label,
                name,
                state.variant != "disabled",
                color,
                !matches!(state.variant.as_str(), "unselected" | "disabled"),
            ));
        }
        "models.choice" => {
            let mut fixture = state.clone();
            if state.variant == "unselected" {
                fixture.selected = 1;
            } else {
                fixture.selected = 0;
            }
            let (label, description) = match state.stage.as_str() {
                "reasoning" => (
                    if state.variant == "reasoning-active" {
                        "Medium (active)"
                    } else {
                        "Medium"
                    },
                    "Balanced reasoning",
                ),
                "output" => (
                    if state.variant == "output-active" {
                        "4,096 tokens (active)"
                    } else {
                        "4,096 tokens"
                    },
                    "Maximum generated output",
                ),
                _ => (
                    if state.variant == "long-label" {
                        "A deliberately long fixture model name that remains bounded"
                    } else {
                        "Auto (current)"
                    },
                    "OpenAgents picks the model",
                ),
            };
            return Some(model_choice(0, label.into(), description, &fixture));
        }
        "models.details" => {
            let model = MODELS[state.selected.min(6)];
            return Some(column("model-details", model_details(model, state)));
        }
        "models.search" => return Some(search_field(state)),
        "sessions.row" => return Some(session_row(state)),
        "plugins.manager" => return Some(manager(state)),
        "plugins.details" => {
            let plugin = selected_plugin(state);
            let mut children = details(plugin, state);
            children.push(action(
                "info-back",
                "Esc Back · Turn on/off from the plugin list",
                "settings.back",
                true,
                SECONDARY,
                false,
            ));
            return Some(screen(plugin.name, children, state));
        }
        "settings.openrouter" => return Some(connection_settings(false, state)),
        "settings.jev" => return Some(connection_settings(true, state)),
        "settings.acp" => return Some(acp_settings(state)),
        "settings.brainstorm" => return Some(brainstorm_settings(state)),
        "settings.boat" => return Some(cloud_settings(false, state)),
        "settings.gce" => return Some(cloud_settings(true, state)),
        "models.picker" => return Some(model_picker(state)),
        "sessions.resume" => return Some(resume_picker(state)),
        "sessions.follow" => return Some(follow_screen(state)),
        "approvals.disclosure" => return Some(disclosure(state)),
        _ => return None,
    }
}

fn selected_plugin(state: &FixtureState) -> Plugin {
    PLUGINS[state.selected.min(PLUGINS.len() - 1)]
}

fn manager(state: &FixtureState) -> Node<CatalogIntent> {
    if state.stage == "configure" {
        return match get(state, "editor-kind") {
            "settings.openrouter" => connection_settings(false, state),
            "settings.jev" => connection_settings(true, state),
            "settings.acp" => acp_settings(state),
            "settings.brainstorm" => brainstorm_settings(state),
            "settings.boat" => cloud_settings(false, state),
            "settings.gce" => cloud_settings(true, state),
            _ => {
                let plugin = selected_plugin(state);
                let mut children = details(plugin, state);
                children.push(action(
                    "info-back",
                    "Esc Back · Turn on/off from the plugin list",
                    "settings.back",
                    true,
                    SECONDARY,
                    false,
                ));
                screen(plugin.name, children, state)
            }
        };
    }
    let wide = state.width >= 56;
    let selected = state.selected.min(7);
    let mut rows = Vec::new();
    if wide {
        rows.push(text(
            "manager-head",
            format!("  {:<30}{:<12}Status", "Plugin", "Enabled"),
            GRAY,
        ));
    }
    for index in 0..PLUGINS.len() {
        rows.push(plugin_row(index, state, index == selected));
    }
    let mut list = column("manager-list", rows);
    let row_height = if wide { 20 } else { 40 };
    let max_height = state
        .height
        .saturating_sub(if state.width >= 64 { 8 } else { 10 })
        .max(2)
        * 20;
    let selected_bottom =
        ((selected + 1) as u16 * row_height).saturating_add(if wide { 20 } else { 0 });
    list.style.viewport = Some(Viewport {
        max_height: max_height.min(180),
        offset: selected_bottom.saturating_sub(max_height.min(180)),
        fade: 0,
    });
    let hint = if state.width >= 64 {
        "Up/Down Select · Space Turn on/off · Enter Configure · Esc Back"
    } else if state.width >= 28 {
        "Up/Down Select · Space Turn on/off\nEnter Configure · Esc Back"
    } else {
        "Up/Down Select\nSpace Turn on/off\nEnter Configure\nEsc Back"
    };
    let mut children = vec![list, text("manager-hints", hint, BRIGHT)];
    children.extend(details(PLUGINS[selected], state));
    children.push(row(
        "manager-controls",
        vec![
            action("manager-prev", "↑", "plugins.previous", true, BRIGHT, false),
            action("manager-next", "↓", "plugins.next", true, BRIGHT, false),
            action(
                "manager-toggle",
                "Space Turn on/off",
                "plugins.toggle",
                true,
                CYAN,
                false,
            ),
            action(
                "manager-configure",
                "Enter Configure",
                "plugins.configure",
                true,
                CYAN,
                false,
            ),
        ],
    ));
    if !state.notice.is_empty() {
        children.push(text("manager-notice", &state.notice, BRIGHT));
    }
    screen("Plugins", children, state)
}

fn plugin_row(index: usize, state: &FixtureState, selected: bool) -> Node<CatalogIntent> {
    let wide = state.width >= 56;
    let plugin = PLUGINS[index.min(7)];
    let enabled = flag(state, &format!("enabled:{}", plugin.id));
    let toggle = if enabled { "[ on  ]" } else { "[ off ]" };
    let status = plugin_status(plugin.id, state);
    let label = if wide {
        format!(
            "{} {:<30}{:<12}{status}",
            if selected { "❯" } else { " " },
            plugin.name,
            toggle
        )
    } else {
        format!(
            "{} {}\n  {toggle} {status}",
            if selected { "❯" } else { " " },
            plugin.name
        )
    };
    let mut runs = vec![
        run(if selected { "❯ " } else { "  " }, CYAN, false),
        run(
            if wide {
                format!("{:<30}", plugin.name)
            } else {
                plugin.name.into()
            },
            PRIMARY,
            selected,
        ),
    ];
    if !wide {
        runs.push(run("\n  ", GRAY, false));
    }
    runs.push(run(
        if wide {
            format!("{toggle:<12}")
        } else {
            format!("{toggle} ")
        },
        if enabled { CYAN } else { GRAY },
        false,
    ));
    runs.push(run(status.clone(), status_color(&status), false));
    rich_choice(
        &format!("plugin-{index}"),
        label,
        selected,
        CatalogIntent::Pick {
            field: "plugin-index".into(),
            value: index.to_string(),
        },
        runs,
    )
}

fn plugin_status(id: &str, state: &FixtureState) -> String {
    if !flag(state, &format!("enabled:{id}")) {
        return "Disabled".into();
    }
    if id == "acp-subagents" {
        let count = number(state, "acp-count", 11);
        if count == 0 {
            return "No agents detected".into();
        }
        if (0..count).all(|i| !flag(state, &format!("acp:{i}"))) {
            return "All agents off".into();
        }
        return "Enabled".into();
    }
    if matches!(id, "openrouter-byok" | "jev" | "brainstorm") {
        let local =
            get(state, "connection-plugin").is_empty() || get(state, "connection-plugin") == id;
        let connection = if local {
            get(state, "connection")
        } else {
            get(state, &format!("connection:{id}"))
        };
        let configured = if local {
            flag(state, "key-configured")
        } else {
            flag(state, &format!("key-configured:{id}"))
        };
        return match connection {
            "checking" => "Checking",
            "verified" => "Verified",
            "failed" => "Unavailable",
            "demo" if id == "brainstorm" => "Demo fixture",
            "expired" if id == "brainstorm" => "Expired",
            _ if matches!(id, "openrouter-byok" | "jev")
                && !configured
                && get(state, "key-mask").is_empty() =>
            {
                "Setup required"
            }
            _ => "Configured",
        }
        .into();
    }
    "Enabled".into()
}

fn status_color(status: &str) -> Color {
    match status {
        "Disabled" => GRAY,
        "Setup required" | "Checking" => AMBER,
        "Unavailable" => RED,
        _ => GREEN,
    }
}

fn details(plugin: Plugin, state: &FixtureState) -> Vec<Node<CatalogIntent>> {
    if !get(state, "connection-plugin").is_empty() && get(state, "connection-plugin") != plugin.id {
        let mut projected = state.clone();
        put(&mut projected, "connection-plugin", plugin.id);
        put(
            &mut projected,
            "connection",
            get(state, &format!("connection:{}", plugin.id)),
        );
        projected.flags.insert(
            "key-configured".into(),
            flag(state, &format!("key-configured:{}", plugin.id)),
        );
        return details(plugin, &projected);
    }
    let mut children = vec![
        text(
            "detail-title",
            if plugin.id == "openrouter-byok" {
                "Model provider"
            } else {
                plugin.name
            },
            CYAN,
        ),
        text("detail-description", plugin.description, SECONDARY),
    ];
    let mut rows: Vec<(&str, String)> = Vec::new();
    match plugin.id {
        "openrouter-byok" => {
            children.push(text(
                "detail-billing",
                "Requests go directly to OpenRouter. Billed by OpenRouter.",
                GRAY,
            ));
            rows.extend([
                (
                    "API key",
                    if flag(state, "key-configured") {
                        if get(state, "connection") == "demo" {
                            "Added · not verified".into()
                        } else {
                            "Added".into()
                        }
                    } else {
                        "Not configured".into()
                    },
                ),
                ("Model", get(state, "model").into()),
                (
                    "Reasoning",
                    if state.variant == "reasoning" {
                        get(state, "reasoning").into()
                    } else {
                        "Model default".into()
                    },
                ),
                ("Endpoint", ROUTER_ENDPOINT.into()),
                ("Connection", connection_label(false, state)),
            ]);
        }
        "jev" => rows.extend([
            ("Tool", "jev".into()),
            ("Gateway", get(state, "gateway").into()),
            ("API key", key_help(true, state)),
            ("Model", get(state, "jev-model").into()),
            ("Endpoint", get(state, "endpoint").into()),
            ("Connection", connection_label(true, state)),
        ]),
        "microcoder" => {
            rows.extend([
                ("Tool", "microcoder".into()),
                (
                    "Provider",
                    "Local model login or configured OpenRouter".into(),
                ),
                ("Working folder", "Current checkout".into()),
            ]);
            children.push(text(
                "detail-limits",
                "The host applies command, step, and time limits.",
                GRAY,
            ));
        }
        "openagents-cli" => {
            rows.extend([
                ("Tool", "openagents_cli".into()),
                ("Command", "openagents".into()),
            ]);
            children.push(text("detail-cli-help", "Installed with Coder. Use --help to discover commands.\nRuns argument arrays in the current working folder.", GRAY));
        }
        "acp-subagents" => {
            rows.extend([
                ("Tool", "acp_subagent".into()),
                (
                    "Agents",
                    format!("{} detected", number(state, "acp-count", 11)),
                ),
            ]);
            children.push(text(
                "detail-acp-help",
                "Installed agents appear automatically. Choose which to use.",
                GRAY,
            ));
        }
        "brainstorm" => {
            rows.extend([
                ("Recipient", get(state, "origin").into()),
                ("Perspective", "Brainstorm house".into()),
            ]);
            children.push(text("detail-brainstorm-help", "Explicit queries and public keys go to this recipient.\nOpening or enabling makes no service read.", GRAY));
        }
        _ => {}
    }
    for (index, (name, value)) in rows.into_iter().enumerate() {
        children.push(detail(
            &format!("detail-value-{index}"),
            name,
            value,
            state.width,
        ));
    }
    if !get(state, "storage-error").is_empty() {
        children.push(text(
            "detail-storage-error",
            get(state, "storage-error"),
            RED,
        ));
    }
    children
}

fn connection_settings(jev: bool, state: &FixtureState) -> Node<CatalogIntent> {
    let mut children = vec![text("connection-heading", "Connection settings", CYAN)];
    if jev {
        children.push(action(
            "gateway-choice",
            format!("Gateway: {}", get(state, "gateway")),
            "settings.gateway",
            true,
            CYAN,
            focused(state, "gateway"),
        ));
        children.push(text(
            "gateway-hint",
            "Enter or Left/Right Select gateway",
            GRAY,
        ));
        children.push(field(
            "endpoint",
            "API base URL",
            get(state, "endpoint"),
            "https://decisions.example.test",
            false,
            true,
            state,
        ));
    } else {
        children.push(detail(
            "router-endpoint",
            "Endpoint",
            ROUTER_ENDPOINT,
            state.width,
        ));
    }
    children.push(field(
        "key",
        if jev {
            "Gateway API key"
        } else {
            "OpenRouter API key"
        },
        get(state, "key-mask"),
        "",
        true,
        true,
        state,
    ));
    children.push(text(
        "key-help",
        if get(state, "error").is_empty() {
            key_help(jev, state)
        } else {
            get(state, "error").into()
        },
        if get(state, "error").is_empty() {
            GRAY
        } else {
            RED
        },
    ));
    let model_field = if jev { "jev-model" } else { "model" };
    children.push(field(
        model_field,
        if jev { "Jev model ID" } else { "Model ID" },
        get(state, model_field),
        if jev { "jev-latest" } else { "openrouter/free" },
        false,
        true,
        state,
    ));
    children.push(text(
        "model-help",
        if jev {
            format!(
                "Default: {} · Typed decisions and probabilities",
                if get(state, "gateway") == "Vercel AI Gateway" {
                    "typesafe-ai/jev"
                } else {
                    "jev-latest"
                }
            )
        } else {
            "Default: openrouter/free · Use /models to choose.".into()
        },
        GRAY,
    ));
    children.push(action(
        "test-key",
        "Test API key",
        "settings.test",
        true,
        CYAN,
        focused(state, "test"),
    ));
    children.push(text(
        "connection-status",
        connection_label(jev, state),
        connection_color(state),
    ));
    children.push(text(
        "storage-label",
        if jev {
            "Saved with Coder plugin settings"
        } else if state.variant == "memory-key" {
            "The key stays in memory until you quit."
        } else {
            "Settings file: ~/.openagents/coder-new/plugins.json."
        },
        GRAY,
    ));
    if !get(state, "storage-error").is_empty() {
        children.push(text(
            "settings-storage-error",
            get(state, "storage-error"),
            RED,
        ));
    }
    children.extend(save_actions(state, true));
    children.push(text(
        "settings-hints",
        "Tab Move between fields · Enter Select",
        GRAY,
    ));
    if !state.notice.is_empty() {
        children.push(text("settings-notice", &state.notice, BRIGHT));
    }
    screen(if jev { "Jev" } else { "OpenRouter BYOK" }, children, state)
}

fn key_help(jev: bool, state: &FixtureState) -> String {
    if flag(state, "remove-key") && get(state, "key-mask").is_empty() {
        "Key will be removed on save".into()
    } else if !get(state, "key-mask").is_empty() {
        "Key hidden".into()
    } else if flag(state, "changed-origin") {
        "Add a key for the new gateway".into()
    } else if flag(state, "key-configured") {
        "Key added · paste to replace".into()
    } else if jev {
        "Add the API key for this gateway".into()
    } else {
        "Get a key at openrouter.ai/keys".into()
    }
}

fn connection_label(jev: bool, state: &FixtureState) -> String {
    let service = if jev { "Jev" } else { "OpenRouter" };
    match get(state, "connection") {
        "checking" => format!("⠋ Checking {service} API key…"),
        "verified" => format!("{service} API key verified"),
        "failed" => {
            if get(state, "error").is_empty() {
                format!("{service} API key verification failed")
            } else {
                get(state, "error").into()
            }
        }
        "demo" => "Demo · no requests sent".into(),
        _ => "Not checked".into(),
    }
}

fn connection_color(state: &FixtureState) -> Color {
    match get(state, "connection") {
        "verified" => GREEN,
        "failed" => RED,
        _ => GRAY,
    }
}

fn save_actions(state: &FixtureState, remove_key: bool) -> Vec<Node<CatalogIntent>> {
    let mut actions = vec![action(
        "save-settings",
        "Save settings",
        "settings.save",
        true,
        CYAN,
        focused(state, "save"),
    )];
    if remove_key {
        actions.push(action(
            "remove-key",
            "Remove API key",
            "settings.remove-key",
            true,
            RED,
            focused(state, "remove"),
        ));
    }
    actions.push(action(
        "cancel-settings",
        "Cancel (Esc)",
        "settings.cancel",
        true,
        SECONDARY,
        focused(state, "cancel"),
    ));
    actions
}

fn isolated_field(state: &FixtureState) -> Node<CatalogIntent> {
    let mut fixture = state.clone();
    let secret = matches!(state.variant.as_str(), "secret" | "masking" | "long-secret");
    let key = if secret { "key" } else { "model" };
    let focus = if state.variant == "unfocused" {
        "none"
    } else {
        key
    };
    put(&mut fixture, "focus", focus);
    let value = if secret {
        get(state, "key-mask").to_string()
    } else if state.variant == "long-value" {
        "example-provider/a-long-fixture-model-name/".repeat(5)
    } else if state.variant == "invalid" {
        "invalid model ID".into()
    } else {
        get(state, "model").into()
    };
    let field = field(
        key,
        if secret {
            "OpenRouter API key"
        } else {
            "Model ID"
        },
        &value,
        "",
        secret,
        state.variant != "disabled",
        &fixture,
    );
    if state.variant == "invalid" {
        column(
            "isolated-field-error",
            vec![
                field,
                text("isolated-field-error-copy", "Use a valid model ID.", RED),
            ],
        )
    } else {
        field
    }
}

const ACP: [(&str, &str); 11] = [
    ("Claude Code", "claude"),
    ("Codex", "codex"),
    ("Grok Build", "grok"),
    ("Devin", "devin"),
    ("OpenCode", "opencode"),
    ("Goose", "goose"),
    ("Cursor", "cursor-agent"),
    ("Oh My Pi", "omp"),
    ("Kimi Code", "kimi"),
    ("Amp", "amp"),
    ("Hermes Agent", "hermes"),
];

fn acp_settings(state: &FixtureState) -> Node<CatalogIntent> {
    let count = number(state, "acp-count", 11).min(32);
    let selected = state.selected.min(count.saturating_sub(1));
    let mut children = vec![text("acp-heading", "Detected on this computer", CYAN)];
    if count == 0 {
        children.push(text("acp-empty", "No ACP agents detected.", BRIGHT));
        children.push(text(
            "acp-install",
            "Install an ACP agent, then press R to refresh.",
            GRAY,
        ));
    } else {
        let mut rows = Vec::new();
        for index in 0..count {
            let (name, program): (String, String) = if count > ACP.len() {
                (format!("Agent {index}"), "fixture-agent".into())
            } else {
                (ACP[index].0.into(), ACP[index].1.into())
            };
            let checked = if flag(state, &format!("acp:{index}")) {
                "[x]"
            } else {
                "[ ]"
            };
            let label = format!(
                "{} {checked} {name}{}",
                if index == selected { "❯" } else { " " },
                if state.width >= 56 {
                    format!("  {program}")
                } else {
                    String::new()
                }
            );
            let mut runs = vec![
                run(if index == selected { "❯ " } else { "  " }, CYAN, false),
                run(
                    format!("{checked} "),
                    if flag(state, &format!("acp:{index}")) {
                        CYAN
                    } else {
                        GRAY
                    },
                    false,
                ),
                run(name, PRIMARY, index == selected),
            ];
            if state.width >= 56 {
                runs.push(run(format!("  {program}"), GRAY, false));
            }
            rows.push(rich_choice(
                &format!("acp-agent-{index}"),
                label,
                index == selected,
                CatalogIntent::Pick {
                    field: "acp-agent".into(),
                    value: index.to_string(),
                },
                runs,
            ));
        }
        let mut list = column("acp-list", rows);
        let height = state.height.saturating_sub(8).max(1) * 20;
        list.style.viewport = Some(Viewport {
            max_height: height,
            offset: ((selected + 1) as u16 * 20).saturating_sub(height),
            fade: 0,
        });
        children.push(list);
        let on = (0..count)
            .filter(|index| flag(state, &format!("acp:{index}")))
            .count();
        children.push(text(
            "acp-count",
            format!("{count} detected · {on} on"),
            GRAY,
        ));
    }
    children.push(text(
        "acp-hints",
        "Up/Down Select · Space/Enter Turn on/off · R Refresh · Esc Back",
        BRIGHT,
    ));
    children.push(row(
        "acp-controls",
        vec![
            action(
                "acp-previous",
                "↑",
                "acp.previous",
                count > 0,
                BRIGHT,
                false,
            ),
            action("acp-next", "↓", "acp.next", count > 0, BRIGHT, false),
            action(
                "acp-toggle",
                "Space/Enter Turn on/off",
                "acp.toggle",
                count > 0,
                CYAN,
                false,
            ),
            action("acp-refresh", "R Refresh", "acp.refresh", true, CYAN, false),
            action(
                "acp-back",
                "Esc Back",
                "settings.back",
                true,
                SECONDARY,
                false,
            ),
        ],
    ));
    if !get(state, "storage-error").is_empty() {
        children.push(text("acp-error", get(state, "storage-error"), RED));
    }
    if !state.notice.is_empty() {
        children.push(text("acp-notice", &state.notice, BRIGHT));
    }
    screen("ACP Subagents", children, state)
}

fn brainstorm_settings(state: &FixtureState) -> Node<CatalogIntent> {
    let mut children = vec![
        text("brainstorm-heading", "Brainstorm house perspective", CYAN),
        detail(
            "brainstorm-recipient",
            "Recipient",
            get(state, "saved-origin"),
            state.width,
        ),
        text(
            "brainstorm-disclosure",
            "Explicit queries and public keys go to this HTTPS recipient.\nOpening, saving, or enabling makes no service read.\nThis integration uses unsigned HTTP observations, not personal or signed scores.",
            GRAY,
        ),
        field(
            "origin",
            "HTTPS origin",
            get(state, "origin"),
            BRAINSTORM_ORIGIN,
            false,
            true,
            state,
        ),
        text(
            "brainstorm-save-first",
            "Save a changed recipient before testing it. Enable from the plugin list.",
            GRAY,
        ),
        action(
            "brainstorm-test",
            "Test connection (public discovery)",
            "settings.test",
            true,
            CYAN,
            focused(state, "test"),
        ),
    ];
    let status = if !flag(state, "enabled:brainstorm") {
        "Disabled"
    } else {
        match get(state, "connection") {
            "demo" => "Demo fixture · no service read",
            "checking" => "Checking",
            "verified" => "Verified",
            "expired" => "Expired",
            "failed" => "Unavailable",
            "configured" => "Configured",
            _ => "Unavailable",
        }
    };
    children.push(detail(
        "brainstorm-connection",
        "Connection",
        status,
        state.width,
    ));
    if !get(state, "house-key").is_empty() {
        children.push(detail(
            "brainstorm-key",
            "House key",
            get(state, "house-key"),
            state.width,
        ));
        children.push(detail(
            "brainstorm-time",
            "Discovered",
            format!("{} ms (Unix time)", get(state, "discovered-at")),
            state.width,
        ));
        children.push(text(
            "brainstorm-key-binding",
            "The separately discovered key is not bound atomically to score responses.",
            GRAY,
        ));
    }
    if !get(state, "error").is_empty() {
        children.push(text("brainstorm-error", get(state, "error"), RED));
    }
    if !get(state, "storage-error").is_empty() {
        children.push(text(
            "brainstorm-storage-error",
            get(state, "storage-error"),
            RED,
        ));
    }
    children.extend(save_actions(state, false));
    children.push(text(
        "brainstorm-usage",
        "/brainstorm search <public query> · /brainstorm rank <hex-or-npub> [more keys]",
        BRIGHT,
    ));
    children.push(text(
        "brainstorm-hint",
        "Tab Move between fields · Enter Select · Esc Cancel pending discovery",
        GRAY,
    ));
    if !state.notice.is_empty() {
        children.push(text("brainstorm-notice", &state.notice, BRIGHT));
    }
    screen("Brainstorm", children, state)
}

fn cloud_settings(gce: bool, state: &FixtureState) -> Node<CatalogIntent> {
    let mut children = vec![text(
        "cloud-credentials-help",
        "Credentials are selected by variable name. Values stay private.",
        GRAY,
    )];
    children.push(action(
        "cloud-mode",
        format!(
            "Mode: {}",
            if gce {
                "Coder runtime"
            } else {
                get(state, "mode")
            }
        ),
        "cloud.mode",
        !gce,
        if focused(state, "mode") {
            PRIMARY
        } else {
            SECONDARY
        },
        focused(state, "mode"),
    ));
    children.push(action(
        "cloud-size",
        format!(
            "Machine size: {}",
            if gce {
                "Granted pool shape"
            } else {
                get(state, "size")
            }
        ),
        "cloud.size",
        !gce,
        if focused(state, "size") {
            PRIMARY
        } else {
            SECONDARY
        },
        focused(state, "size"),
    ));
    children.push(field(
        "template",
        "Template",
        get(state, "template"),
        "",
        false,
        !gce,
        state,
    ));
    children.push(field(
        "credentials",
        "Credential variables",
        get(state, "credentials"),
        "OPENROUTER_API_KEY, TYPESAFE_API_KEY",
        false,
        true,
        state,
    ));
    children.push(field(
        "paths",
        "Workspace paths",
        get(state, "paths"),
        "workspace/project, workspace/notes",
        false,
        true,
        state,
    ));
    children.push(action(
        "cloud-save",
        "Save",
        "settings.save",
        true,
        if focused(state, "save") {
            PRIMARY
        } else {
            SECONDARY
        },
        focused(state, "save"),
    ));
    children.push(action(
        "cloud-cancel",
        "Cancel",
        "settings.cancel",
        true,
        if focused(state, "cancel") {
            PRIMARY
        } else {
            SECONDARY
        },
        focused(state, "cancel"),
    ));
    children.push(text(
        "cloud-hints",
        "Enter/Space: change choice · Tab: next · comma-separated names and paths",
        GRAY,
    ));
    if !get(state, "error").is_empty() {
        children.push(text("cloud-error", get(state, "error"), PRIMARY));
    }
    if !get(state, "storage-error").is_empty() {
        children.push(text(
            "cloud-storage-error",
            get(state, "storage-error"),
            RED,
        ));
    }
    if !state.notice.is_empty() {
        children.push(text("cloud-notice", &state.notice, BRIGHT));
    }
    screen(
        if gce { "GCE Cloud" } else { "Boat Cloud" },
        children,
        state,
    )
}

#[derive(Clone, Copy)]
struct Model {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    efforts: &'static [&'static str],
    default: &'static str,
}

const MODELS: [Model; 7] = [
    Model {
        id: "openrouter/free",
        name: "Auto",
        description: "OpenAgents picks the model",
        efforts: &[],
        default: "",
    },
    Model {
        id: "openai/gpt-6-luna",
        name: "GPT-6 Luna",
        description: "Fast OpenAI text model",
        efforts: &["none", "low", "medium", "high", "xhigh", "max"],
        default: "medium",
    },
    Model {
        id: "openai/gpt-6.1-sol",
        name: "GPT-6.1 Sol",
        description: "OpenAI text and reasoning",
        efforts: &["low", "medium", "high", "xhigh", "max"],
        default: "medium",
    },
    Model {
        id: "anthropic/claude-fable-5.1",
        name: "Claude Fable 5.1",
        description: "Anthropic text and reasoning",
        efforts: &["low", "medium", "high", "xhigh", "max"],
        default: "high",
    },
    Model {
        id: "google/gemini-3.5-flash",
        name: "Gemini 3.5 Flash",
        description: "Fast Google text model",
        efforts: &["minimal", "low", "medium", "high"],
        default: "medium",
    },
    Model {
        id: "deepseek/deepseek-v4.1-flash",
        name: "DeepSeek V4.1 Flash",
        description: "DeepSeek text and reasoning",
        efforts: &["low", "high", "max"],
        default: "high",
    },
    Model {
        id: "x-ai/grok-4.7",
        name: "Grok 4.7",
        description: "xAI text and reasoning",
        efforts: &["low", "medium", "high", "xhigh"],
        default: "high",
    },
];

fn matching_models(state: &FixtureState) -> Vec<Model> {
    let query = get(state, "model-search").to_lowercase();
    MODELS
        .into_iter()
        .filter(|model| {
            format!("{} {} {}", model.name, model.id, model.description)
                .to_lowercase()
                .contains(&query)
        })
        .collect()
}

fn pending_model(state: &FixtureState) -> Model {
    MODELS
        .into_iter()
        .find(|model| model.id == get(state, "pending-model"))
        .unwrap_or(MODELS[1])
}

fn reasoning_choices(state: &FixtureState) -> Vec<(&'static str, &'static str, &'static str)> {
    let mut choices = vec![("", "Model default", "Use the model's default")];
    let efforts = pending_model(state).efforts;
    for (id, label, detail) in [
        ("none", "None", "No reasoning"),
        ("minimal", "Minimal", "Minimal reasoning"),
        ("low", "Low", "Faster, lighter reasoning"),
        ("medium", "Medium", "Balanced reasoning"),
        ("high", "High", "Heavy reasoning"),
        ("xhigh", "Extra high", "Extended reasoning"),
        ("max", "Maximum", "Maximum reasoning"),
    ] {
        if flag(state, "all-efforts") || efforts.contains(&id) {
            choices.push((id, label, detail));
        }
    }
    choices
}

fn output_choices(state: &FixtureState) -> Vec<u32> {
    let maximum = number(state, "max-output", 32_768).min(32_768);
    std::iter::once(0)
        .chain(
            [2048, 4096, 8192, 16384, 32768]
                .into_iter()
                .filter(|tokens| *tokens as usize <= maximum),
        )
        .collect()
}

fn model_picker(state: &FixtureState) -> Node<CatalogIntent> {
    if state.width < 4 || state.height < 4 {
        return column("model-tiny", Vec::new());
    }
    if state.stage == "closed" {
        return super::conversation::render("screen.main", "live", state)
            .unwrap_or_else(|| column("model-closed", Vec::new()));
    }
    let mut children = Vec::new();
    if state.stage == "models" {
        children.push(search_field(state));
    }
    if !get(state, "error").is_empty() {
        children.push(text("model-error", get(state, "error"), RED));
    } else if flag(state, "model-loading") {
        children.push(text("model-loading", "⠋ Refreshing model details", CYAN));
    }
    let mut rows = Vec::new();
    let model;
    let active = get(state, "pending-model") == get(state, "active-model");
    match state.stage.as_str() {
        "reasoning" => {
            model = Some(pending_model(state));
            for (index, (value, label, description)) in reasoning_choices(state).iter().enumerate()
            {
                let label = format!(
                    "{label}{}",
                    if active && *value == get(state, "reasoning") {
                        " (active)"
                    } else {
                        ""
                    }
                );
                rows.push(model_choice(index, label, description, state));
            }
        }
        "output" => {
            model = Some(pending_model(state));
            for (index, tokens) in output_choices(state).iter().enumerate() {
                let label = if *tokens == 0 {
                    "Model default".into()
                } else {
                    format!("{} tokens", comma(*tokens))
                };
                let active = active
                    && if *tokens == 0 {
                        get(state, "output").is_empty()
                    } else {
                        tokens.to_string() == get(state, "output")
                    };
                rows.push(model_choice(
                    index,
                    format!("{label}{}", if active { " (active)" } else { "" }),
                    if *tokens == 0 {
                        "Use the model's default"
                    } else {
                        "Maximum generated output"
                    },
                    state,
                ));
            }
        }
        _ => {
            let models = matching_models(state);
            model = models.get(state.selected).copied();
            for (index, model) in models.iter().enumerate() {
                let name = if flag(state, "long-label") && index == 0 {
                    "A deliberately long fixture model name that remains bounded"
                } else {
                    model.name
                };
                let label = format!(
                    "{name}{}",
                    if model.id == get(state, "active-model") {
                        " (current)"
                    } else {
                        ""
                    }
                );
                rows.push(model_choice(index, label, model.description, state));
            }
            if rows.is_empty() {
                rows.push(text("model-empty", "No matching models", GRAY));
            }
        }
    }
    let list_height = state.height.saturating_sub(11).max(1).min(9) * 20;
    let mut list = column("model-choice-list", rows);
    let start = state
        .selected
        .saturating_sub(usize::from(list_height / 20) / 2);
    list.style.viewport = Some(Viewport {
        max_height: list_height,
        offset: (start as u16).saturating_mul(20),
        fade: 0,
    });
    children.push(list);
    if let Some(model) = model {
        children.extend(model_details(model, state));
    }
    children.push(text(
        "model-footer",
        if model_inner_width(state) >= 36 {
            "↑/↓ Move · Enter Select · Esc Back"
        } else {
            "↑/↓ · Enter · Esc"
        },
        BRIGHT,
    ));
    children.push(row(
        "model-controls",
        vec![
            action(
                "model-previous",
                "↑",
                "models.previous",
                true,
                BRIGHT,
                false,
            ),
            action("model-next", "↓", "models.next", true, BRIGHT, false),
            action(
                "model-select",
                "Enter Select",
                "models.select",
                model.is_some(),
                CYAN,
                false,
            ),
            action(
                "model-back",
                "Esc Back",
                "models.back",
                true,
                SECONDARY,
                false,
            ),
            action(
                "model-refresh",
                "Refresh fixture",
                "models.refresh",
                true,
                CYAN,
                false,
            ),
        ],
    ));
    let title = match state.stage.as_str() {
        "reasoning" => "Pick reasoning level",
        "output" => "Maximum output tokens",
        _ => "Pick model",
    };
    let mut dialog = node(
        "model-dialog",
        Element::Dialog {
            label: title.into(),
            open: true,
            on_close: CatalogIntent::Action {
                name: "models.back".into(),
            },
            children: vec![
                text("model-title", title, PRIMARY),
                column("model-popup-body", children),
            ],
        },
    );
    dialog.style.border = Some(BORDER);
    dialog.style.radius = Some(8);
    dialog.style.background = Some(BASE);
    dialog.style.padding_points = Some([0, 9, 0, 9]);
    dialog
}

fn search_field(state: &FixtureState) -> Node<CatalogIntent> {
    let mut search = field(
        "model-search",
        "Search: ",
        get(state, "model-search"),
        "",
        false,
        true,
        state,
    );
    search.style.border = None;
    search.style.padding_points = Some([0; 4]);
    search.style.gap_points = Some(0);
    search
}

fn model_details(model: Model, state: &FixtureState) -> Vec<Node<CatalogIntent>> {
    vec![
        text("model-provider", "Provider  OpenRouter BYOK", CYAN),
        text("model-identity", format!("ID  {}", model.id), SECONDARY),
        text(
            "model-limits",
            if flag(state, "model-metadata") || !get(state, "max-output").is_empty() {
                format!(
                    "Context 128,000 · Output {}",
                    comma(number(state, "max-output", 32_768) as u32)
                )
            } else {
                model.description.into()
            },
            BRIGHT,
        ),
    ]
}

fn model_choice(
    index: usize,
    label: String,
    description: &str,
    state: &FixtureState,
) -> Node<CatalogIntent> {
    let width = model_inner_width(state);
    let right = if width >= 42 {
        crate::components::truncate(description, usize::from(width / 3))
    } else {
        String::new()
    };
    let label_width = width
        .saturating_sub(2 + right.chars().count() as u16 + if right.is_empty() { 0 } else { 2 });
    let label = crate::components::truncate(&label, usize::from(label_width));
    let mut runs = vec![
        run(
            if index == state.selected {
                "❯ "
            } else {
                "  "
            },
            CYAN,
            false,
        ),
        run(&label, PRIMARY, index == state.selected),
    ];
    if !right.is_empty() {
        let padding = usize::from(label_width).saturating_sub(label.chars().count()) + 2;
        runs.push(run(
            format!("{}{right}", " ".repeat(padding)),
            BRIGHT,
            false,
        ));
    }
    let label = format!(
        "{} {label}{}",
        if index == state.selected { "❯" } else { " " },
        if !right.is_empty() {
            format!("  {right}")
        } else {
            String::new()
        }
    );
    rich_choice(
        &format!("model-choice-{index}"),
        label,
        index == state.selected,
        CatalogIntent::Pick {
            field: "model-index".into(),
            value: index.to_string(),
        },
        runs,
    )
}

fn model_inner_width(state: &FixtureState) -> u16 {
    (state.width / 2)
        .clamp(44, 80)
        .min(state.width.saturating_sub(2))
        .saturating_sub(2)
}

fn resume_picker(state: &FixtureState) -> Node<CatalogIntent> {
    let count = number(state, "resume-count", 5).min(100);
    let selected = state.selected.min(count.saturating_sub(1));
    let page = usize::from(
        state
            .height
            .saturating_sub(if get(state, "error").is_empty() { 4 } else { 6 })
            / 2,
    )
    .max(1);
    let start = selected
        .saturating_sub(page / 2)
        .min(count.saturating_sub(page));
    let mut children = Vec::new();
    if count == 0 {
        children.push(text("resume-empty", "No conversations to resume.", BRIGHT));
    }
    let titles = [
        "Build the shared Coder components",
        "Review the cloud handoff",
        "Investigate model capability refresh",
        "Restore the saved conversation",
        "Write the release notes",
    ];
    let ages = ["just now", "4m ago", "2h ago", "1d ago", "4d ago"];
    for index in start..count.min(start + page) {
        let title = if index < titles.len() {
            titles[index].to_string()
        } else {
            format!("Saved fixture conversation {}", index + 1)
        };
        let mut session = choice(
            &format!("resume-session-{index}"),
            format!(
                "{} {}. {title}",
                if index == selected { "❯" } else { " " },
                index + 1
            ),
            index == selected,
            CatalogIntent::Pick {
                field: "resume-index".into(),
                value: index.to_string(),
            },
            if index == selected {
                PRIMARY
            } else {
                SECONDARY
            },
        );
        session.style.min_height = Some(20);
        children.push(session);
        children.push(text(
            &format!("resume-detail-{index}"),
            format!(
                "  {} · {} entries · fixture-session-{} · workspace/openagents",
                ages[index.min(4)],
                12 + index * 7,
                index + 1
            ),
            BRIGHT,
        ));
    }
    if !get(state, "error").is_empty() {
        children.push(text("resume-error", get(state, "error"), RED));
    }
    if !state.notice.is_empty() {
        children.push(text("resume-notice", &state.notice, BRIGHT));
    }
    children.push(text(
        "resume-footer",
        "↑/↓ Choose · Enter Resume · Esc Back",
        BRIGHT,
    ));
    children.push(row(
        "resume-controls",
        vec![
            action(
                "resume-prev",
                "↑",
                "resume.previous",
                count > 0,
                BRIGHT,
                false,
            ),
            action("resume-next", "↓", "resume.next", count > 0, BRIGHT, false),
            action(
                "resume-page-up",
                "PgUp",
                "resume.page-up",
                count > 0,
                BRIGHT,
                false,
            ),
            action(
                "resume-page-down",
                "PgDn",
                "resume.page-down",
                count > 0,
                BRIGHT,
                false,
            ),
            action(
                "resume-home",
                "Home",
                "resume.home",
                count > 0,
                BRIGHT,
                false,
            ),
            action("resume-end", "End", "resume.end", count > 0, BRIGHT, false),
            action(
                "resume-select",
                "Enter Resume",
                "resume.select",
                count > 0,
                CYAN,
                false,
            ),
            action(
                "resume-back",
                "Esc Back",
                "settings.back",
                true,
                SECONDARY,
                false,
            ),
        ],
    ));
    screen("Resume · recent conversations", children, state)
}

fn session_row(state: &FixtureState) -> Node<CatalogIntent> {
    let selected = state.variant != "unselected";
    let age = match state.variant.as_str() {
        "minutes" => "4m ago",
        "hours" => "2h ago",
        "days" => "4d ago",
        _ => "just now",
    };
    let title = if state.variant == "long-title" {
        "A saved fixture conversation with a title that exceeds the available row width and stays clipped"
    } else {
        "Build the shared Coder components"
    };
    let label = format!("{} 1. {title}", if selected { "❯" } else { " " });
    let runs = vec![
        run(
            if selected { "❯ " } else { "  " },
            if selected { PRIMARY } else { GRAY },
            false,
        ),
        run(
            format!("1. {title}"),
            if selected { PRIMARY } else { SECONDARY },
            false,
        ),
    ];
    column(
        "session-row",
        vec![
            rich_choice(
                "session-row-choice",
                label,
                selected,
                CatalogIntent::Pick {
                    field: "resume-index".into(),
                    value: "0".into(),
                },
                runs,
            ),
            text(
                "session-row-details",
                format!("  {age} · 12 entries · fixture-session-1 · workspace/openagents"),
                BRIGHT,
            ),
        ],
    )
}

fn follow_screen(state: &FixtureState) -> Node<CatalogIntent> {
    let acquired = flag(state, "takeover-acquired");
    let mut children = vec![text(
        "follow-session",
        "fixture-session-1 · workspace/openagents",
        BRIGHT,
    )];
    if !state.notice.is_empty() {
        children.push(text("follow-notice", &state.notice, BRIGHT));
    }
    let transcript = vec![
        text(
            "follow-user",
            "❯ Review the shared Coder component fixtures.",
            PRIMARY,
        ),
        text(
            "follow-assistant",
            "The agent is reviewing model settings and saved conversations.",
            SECONDARY,
        ),
        text(
            "follow-delegate",
            "│ microcoder · Inspect fixture coverage",
            CYAN,
        ),
        text(
            "follow-status",
            if flag(state, "reclaim-busy") {
                "⠋ The current reply is running."
            } else {
                "⠋ The fixture agent is checking source branches."
            },
            CYAN,
        ),
    ];
    let mut history = column("follow-transcript", transcript);
    history.style.viewport = Some(Viewport {
        max_height: 200,
        offset: state.scroll.min(100) as u16 * 20,
        fade: 0,
    });
    children.push(history);
    children.push(text(
        "follow-position",
        if flag(state, "stick-to-end") {
            "Following the newest entry"
        } else {
            "Reviewing earlier entries"
        },
        GRAY,
    ));
    children.push(row(
        "follow-controls",
        vec![
            action(
                "follow-up",
                "PgUp Review earlier",
                "follow.up",
                true,
                BRIGHT,
                false,
            ),
            action(
                "follow-down",
                "PgDn Review later",
                "follow.down",
                true,
                BRIGHT,
                false,
            ),
            action(
                "follow-end",
                "End Newest",
                "follow.end",
                true,
                BRIGHT,
                false,
            ),
            action(
                "follow-takeover",
                "Take over",
                "follow.takeover",
                !acquired && !flag(state, "takeover-pending"),
                CYAN,
                false,
            ),
            action(
                "follow-release",
                "Agent releases fixture",
                "follow.release",
                flag(state, "takeover-pending"),
                CYAN,
                false,
            ),
            action(
                "follow-reclaim",
                "Agent reclaims fixture",
                "follow.reclaim",
                acquired,
                CYAN,
                false,
            ),
        ],
    ));
    children.push(field(
        "follow-draft",
        "Draft",
        &state.draft,
        if acquired {
            "Write a message"
        } else {
            "Take over to write"
        },
        false,
        acquired,
        state,
    ));
    screen(
        if acquired {
            "Coder"
        } else {
            "Coder · following"
        },
        children,
        state,
    )
}

fn disclosure_input(state: &FixtureState) -> String {
    if state.variant == "rank" {
        format!(
            "{{\n  \"algorithm\": \"graperank\",\n  \"pubkeys\": [\n    \"{}\",\n    \"{}\"\n  ],\n  \"recipient\": \"{}\"\n}}",
            "1".repeat(64),
            "2".repeat(64),
            get(state, "saved-origin")
        )
    } else {
        let long = matches!(
            state.variant.as_str(),
            "long" | "unreviewed" | "reviewed" | "short" | "narrow"
        );
        let query = if long {
            "Public synthetic component research across model settings, saved conversations, and explicit public lookup review. ".repeat(30)
        } else {
            "Public synthetic component research".into()
        };
        format!(
            "{{\n  \"algorithm\": \"relevance\",\n  \"limit\": 10,\n  \"query\": \"{}\",\n  \"recipient\": \"{}\"\n}}",
            query.trim(),
            get(state, "saved-origin")
        )
    }
}

fn disclosure(state: &FixtureState) -> Node<CatalogIntent> {
    let closed = matches!(
        state.stage.as_str(),
        "confirmed" | "rejected" | "cancelled" | "closed" | "expired"
    );
    if closed {
        return super::conversation::render("screen.main", "live", state)
            .unwrap_or_else(|| column("disclosure-closed", Vec::new()));
    }
    let input = disclosure_input(state);
    let content = vec![
        text(
            "disclosure-question",
            "Send this exact lookup to Brainstorm?",
            PRIMARY,
        ),
        text("disclosure-question-gap", " ", PRIMARY),
        text(
            "disclosure-recipient",
            format!("Recipient: {}", get(state, "saved-origin")),
            PRIMARY,
        ),
        text("disclosure-recipient-gap", " ", PRIMARY),
        code("disclosure-input", input),
        text("disclosure-input-gap", " ", PRIMARY),
        text(
            "disclosure-scope",
            "No files or conversation are added. This approves this lookup input only.",
            PRIMARY,
        ),
        text("disclosure-scope-gap", " ", PRIMARY),
        text(
            "disclosure-source-hint",
            "Y: confirm · N: reject · Esc: cancel · PgUp/PgDn: review",
            PRIMARY,
        ),
    ];
    let mut body = column("disclosure-review", content);
    body.style.viewport = Some(Viewport {
        max_height: state.height.saturating_sub(8).max(2) * 20,
        offset: state.scroll.min(u16::MAX as usize / 20) as u16 * 20,
        fade: 0,
    });
    let reviewed = flag(state, "reviewed") || disclosure_max_scroll(state) == 0;
    let mut children = vec![
        body,
        row(
            "disclosure-review-controls",
            vec![
                action(
                    "disclosure-up",
                    "PgUp Review",
                    "disclosure.up",
                    true,
                    BRIGHT,
                    false,
                ),
                action(
                    "disclosure-down",
                    "PgDn Review",
                    "disclosure.down",
                    true,
                    BRIGHT,
                    false,
                ),
                action(
                    "disclosure-home",
                    "Home",
                    "disclosure.home",
                    true,
                    BRIGHT,
                    false,
                ),
                action(
                    "disclosure-end",
                    "End",
                    "disclosure.end",
                    true,
                    BRIGHT,
                    false,
                ),
            ],
        ),
        row(
            "disclosure-answer-controls",
            vec![
                action(
                    "disclosure-confirm",
                    "Y Confirm",
                    "disclosure.confirm",
                    reviewed,
                    PRIMARY,
                    false,
                ),
                action(
                    "disclosure-reject",
                    "N Reject",
                    "disclosure.reject",
                    true,
                    PRIMARY,
                    false,
                ),
                action(
                    "disclosure-cancel",
                    "Esc Cancel",
                    "disclosure.cancel",
                    true,
                    PRIMARY,
                    false,
                ),
            ],
        ),
    ];
    if !state.notice.is_empty() {
        children.push(text("disclosure-notice", &state.notice, BRIGHT));
    }
    let mut root = column("disclosure-screen", children);
    root.style.padding_points = Some([20, 18, 20, 18]);
    root.style.fill_height = Some(true);
    root
}

fn disclosure_max_scroll(state: &FixtureState) -> usize {
    let width = usize::from(state.width.saturating_sub(4).max(1));
    let text = format!(
        "Send this exact lookup to Brainstorm?\n\nRecipient: {}\n\n{}\n\nNo files or conversation are added. This approves this lookup input only.\n\nY: confirm · N: reject · Esc: cancel · PgUp/PgDn: review",
        get(state, "saved-origin"),
        disclosure_input(state)
    );
    let lines: usize = text
        .lines()
        .map(|line| line.chars().count().max(1).div_ceil(width))
        .sum();
    lines.saturating_sub(usize::from(state.height.saturating_sub(8).max(2)))
}

pub(super) fn reduce(
    state: &mut FixtureState,
    intent: &CatalogIntent,
    input: Option<&str>,
) -> Option<Result<(), String>> {
    if !owns(&state.component) {
        return None;
    }
    let result = match intent {
        CatalogIntent::Input { field } => {
            let value = input.unwrap_or_default();
            if value.len() > 8192 {
                return Some(Err("The fixture input exceeds 8 KiB.".into()));
            }
            let accepted = matches!(
                field.as_str(),
                "key"
                    | "model"
                    | "jev-model"
                    | "endpoint"
                    | "origin"
                    | "template"
                    | "credentials"
                    | "paths"
                    | "model-search"
                    | "follow-draft"
            );
            if !accepted {
                return None;
            }
            if field == "key" {
                put(
                    state,
                    "key-mask",
                    &"•".repeat(value.graphemes(true).count().min(2048)),
                );
                state.flags.insert(
                    "key-invalid".into(),
                    value.chars().any(|c| c.is_whitespace() || c.is_control()),
                );
                state.flags.insert("remove-key".into(), false);
            } else if field == "follow-draft" {
                if !flag(state, "takeover-acquired") {
                    return Some(Err("Take over the fixture before writing.".into()));
                }
                state.draft = value.into();
            } else {
                if field == "endpoint" {
                    if !same_origin(value, get(state, "endpoint")) {
                        put(state, "key-mask", "");
                    }
                    state.flags.insert(
                        "changed-origin".into(),
                        !same_origin(value, get(state, "saved.endpoint")),
                    );
                }
                put(state, field, value);
                if field == "model-search" {
                    state.selected = 0;
                }
            }
            put(
                state,
                "focus",
                if field == "jev-model" { "model" } else { field },
            );
            put(state, "connection", "unchecked");
            put(state, "error", "");
            Ok(())
        }
        CatalogIntent::Pick { field, value }
            if matches!(
                field.as_str(),
                "plugin-index" | "acp-agent" | "model-index" | "resume-index"
            ) =>
        {
            let Ok(index) = value.parse::<usize>() else {
                return Some(Err("Choose a fixture row.".into()));
            };
            let count = match field.as_str() {
                "plugin-index" => 8,
                "acp-agent" => number(state, "acp-count", 11),
                "resume-index" => number(state, "resume-count", 5),
                _ => model_choice_count(state),
            };
            if index >= count {
                return Some(Err("The fixture row is unavailable.".into()));
            }
            state.selected = index;
            if field == "acp-agent" {
                toggle_acp(state);
            }
            Ok(())
        }
        CatalogIntent::Action { name } => match name.as_str() {
            "plugins.previous" => {
                move_selection(state, -1, 8);
                Ok(())
            }
            "plugins.next" => {
                move_selection(state, 1, 8);
                Ok(())
            }
            "plugins.toggle" => {
                let key = format!("enabled:{}", selected_plugin(state).id);
                state.flags.insert(key.clone(), !flag(state, &key));
                Ok(())
            }
            "plugins.configure" => {
                let kind = match selected_plugin(state).id {
                    "openrouter-byok" => "settings.openrouter",
                    "jev" => "settings.jev",
                    "acp-subagents" => "settings.acp",
                    "brainstorm" => "settings.brainstorm",
                    "boat-cloud" => "settings.boat",
                    "gce-cloud" => "settings.gce",
                    _ => "plugins.details",
                };
                put(state, "editor-kind", kind);
                let selected = state.selected.to_string();
                put(state, "manager-selected", &selected);
                let plugin = selected_plugin(state).id;
                put(state, "connection-plugin", plugin);
                let connection =
                    get(state, &format!("connection:{}", selected_plugin(state).id)).to_string();
                put(state, "connection", &connection);
                state.flags.insert(
                    "key-configured".into(),
                    flag(
                        state,
                        &format!("key-configured:{}", selected_plugin(state).id),
                    ),
                );
                let focus = default_focus(kind);
                put(state, "focus", focus);
                if kind == "settings.gce" {
                    put(state, "mode", "Coder runtime");
                }
                state.stage = "configure".into();
                state.notice.clear();
                snapshot_settings(state);
                Ok(())
            }
            "settings.back" | "settings.cancel" => {
                restore_settings(state);
                if state.component == "plugins.manager" {
                    state.stage = "models".into();
                    state.selected = number(state, "manager-selected", state.selected).min(7);
                }
                state.notice = "Cancelled the fixture changes.".into();
                Ok(())
            }
            "settings.remove-key" => {
                put(state, "key-mask", "");
                state.flags.insert("remove-key".into(), true);
                state.flags.insert("key-invalid".into(), false);
                put(state, "connection", "unchecked");
                Ok(())
            }
            "settings.gateway" => {
                let (gateway, endpoint, model) = match get(state, "gateway") {
                    "TypeSafe direct" => ("Vercel AI Gateway", VERCEL_ENDPOINT, "typesafe-ai/jev"),
                    "Vercel AI Gateway" => (
                        "Custom gateway",
                        "https://decisions.example.test",
                        "jev-latest",
                    ),
                    _ => ("TypeSafe direct", TYPESAFE_ENDPOINT, "jev-latest"),
                };
                put(state, "gateway", gateway);
                put(state, "endpoint", endpoint);
                put(state, "jev-model", model);
                put(state, "key-mask", "");
                state.flags.insert("changed-origin".into(), true);
                put(state, "connection", "unchecked");
                Ok(())
            }
            "settings.test" => {
                if editing_component(state) == "settings.brainstorm"
                    && get(state, "origin") != get(state, "saved-origin")
                {
                    put(
                        state,
                        "error",
                        "Save the changed recipient before testing it.",
                    );
                } else if editing_component(state) != "settings.brainstorm"
                    && (!flag(state, "key-configured")
                        || flag(state, "changed-origin")
                        || flag(state, "remove-key"))
                    && get(state, "key-mask").is_empty()
                {
                    put(state, "error", "Add an API key first.");
                } else {
                    put(state, "error", "");
                    put(state, "connection", "checking");
                    state.phase = 0;
                    state.notice = "The fixture test advances locally when you press Tick.".into();
                }
                Ok(())
            }
            "settings.save" => save_settings(state),
            "cloud.mode" => {
                if editing_component(state) != "settings.gce" {
                    let mode = if get(state, "mode") == "Integrated agent" {
                        "Coder runtime"
                    } else {
                        "Integrated agent"
                    };
                    put(state, "mode", mode);
                }
                Ok(())
            }
            "cloud.size" => {
                if editing_component(state) != "settings.gce" {
                    let choices = ["small", "default", "large", "xlarge"];
                    let index = choices
                        .iter()
                        .position(|size| *size == get(state, "size"))
                        .unwrap_or(1);
                    put(state, "size", choices[(index + 1) % 4]);
                }
                Ok(())
            }
            "acp.previous" => {
                move_named_selection(state, -1, "acp-count", 11);
                Ok(())
            }
            "acp.next" => {
                move_named_selection(state, 1, "acp-count", 11);
                Ok(())
            }
            "acp.toggle" => {
                toggle_acp(state);
                Ok(())
            }
            "acp.refresh" => {
                if number(state, "acp-count", 11) == 0 {
                    put(state, "acp-count", "11");
                }
                state.notice = "Refreshed the synthetic agent list.".into();
                Ok(())
            }
            "models.previous" => {
                let count = model_choice_count(state);
                move_selection(state, -1, count);
                Ok(())
            }
            "models.next" => {
                let count = model_choice_count(state);
                move_selection(state, 1, count);
                Ok(())
            }
            "models.select" => select_model(state),
            "models.back" => {
                state.stage = match state.stage.as_str() {
                    "output"
                        if flag(state, "reasoning-supported")
                            && !pending_model(state).efforts.is_empty() =>
                    {
                        "reasoning"
                    }
                    "output" | "reasoning" => "models",
                    _ => "closed",
                }
                .into();
                state.selected = 0;
                Ok(())
            }
            "models.reopen" => {
                state.stage = "models".into();
                state.selected = 0;
                state.notice.clear();
                Ok(())
            }
            "models.refresh" => {
                state.flags.insert("model-loading".into(), true);
                put(state, "error", "");
                state.phase = 0;
                Ok(())
            }
            "resume.previous" => {
                move_named_selection(state, -1, "resume-count", 5);
                Ok(())
            }
            "resume.next" => {
                move_named_selection(state, 1, "resume-count", 5);
                Ok(())
            }
            "resume.page-up" => {
                let page = (state.height.saturating_sub(4) / 2).max(1) as i32;
                move_named_selection(state, -page, "resume-count", 5);
                Ok(())
            }
            "resume.page-down" => {
                let page = (state.height.saturating_sub(4) / 2).max(1) as i32;
                move_named_selection(state, page, "resume-count", 5);
                Ok(())
            }
            "resume.home" => {
                state.selected = 0;
                Ok(())
            }
            "resume.end" => {
                state.selected = number(state, "resume-count", 5).saturating_sub(1);
                Ok(())
            }
            "resume.select" => {
                if number(state, "resume-count", 5) == 0 {
                    Err("There are no fixture conversations to resume.".into())
                } else if !get(state, "error").is_empty() {
                    state.notice = get(state, "error").into();
                    Ok(())
                } else {
                    state.notice = format!("Resumed fixture-session-{}.", state.selected + 1);
                    Ok(())
                }
            }
            "follow.up" => {
                state.scroll = state.scroll.saturating_add(3);
                state.flags.insert("stick-to-end".into(), false);
                Ok(())
            }
            "follow.down" => {
                state.scroll = state.scroll.saturating_sub(3);
                Ok(())
            }
            "follow.end" => {
                state.scroll = 0;
                state.flags.insert("stick-to-end".into(), true);
                Ok(())
            }
            "follow.takeover" => {
                state.flags.insert("takeover-pending".into(), true);
                state.notice =
                    "Taking over: the conversation is yours as soon as its agent stops.".into();
                Ok(())
            }
            "follow.release" => {
                if flag(state, "takeover-pending") {
                    state.flags.insert("takeover-pending".into(), false);
                    state.flags.insert("takeover-acquired".into(), true);
                    state.notice = "Conversation fixture-session-1 is yours now.".into();
                }
                Ok(())
            }
            "follow.reclaim" => {
                state.flags.insert("takeover-acquired".into(), false);
                state.flags.insert("stick-to-end".into(), true);
                state.notice = "Your agent took the conversation back to answer you.".into();
                Ok(())
            }
            "disclosure.up" => {
                state.scroll = state.scroll.saturating_sub(5);
                Ok(())
            }
            "disclosure.down" => {
                state.scroll = state
                    .scroll
                    .saturating_add(5)
                    .min(disclosure_max_scroll(state));
                Ok(())
            }
            "disclosure.home" => {
                state.scroll = 0;
                Ok(())
            }
            "disclosure.end" => {
                state.scroll = disclosure_max_scroll(state);
                Ok(())
            }
            "review-complete" => {
                state.flags.insert("reviewed".into(), true);
                Ok(())
            }
            "disclosure.confirm" => {
                if !flag(state, "reviewed") && disclosure_max_scroll(state) > 0 {
                    Err("Review the complete lookup before confirming.".into())
                } else {
                    state.stage = "confirmed".into();
                    state.notice = "Disclosure confirmed for this synthetic input only.".into();
                    Ok(())
                }
            }
            "disclosure.reject" => {
                state.stage = "rejected".into();
                state.notice = "Disclosure rejected.".into();
                Ok(())
            }
            "disclosure.cancel" => {
                state.stage = "cancelled".into();
                state.notice = "Disclosure cancelled.".into();
                Ok(())
            }
            "disclosure.reopen" => {
                state.stage = "review".into();
                state.scroll = 0;
                state.flags.insert("reviewed".into(), false);
                state.notice.clear();
                Ok(())
            }
            _ => return None,
        },
        CatalogIntent::Tick => {
            if get(state, "connection") == "checking" {
                put(state, "connection", "verified");
                state.notice = "Completed the local fixture test.".into();
                if editing_component(state) == "settings.brainstorm" {
                    put(state, "house-key", &"f".repeat(64));
                    put(state, "discovered-at", "1791432000000");
                }
                retain_plugin_connection(state);
            }
            if flag(state, "model-loading") {
                state.flags.insert("model-loading".into(), false);
                state.flags.insert("model-metadata".into(), true);
            }
            return None;
        }
        CatalogIntent::Scroll { delta } if state.component == "approvals.disclosure" => {
            state.scroll = (state.scroll as i64 + i64::from(*delta))
                .clamp(0, disclosure_max_scroll(state) as i64) as usize;
            Ok(())
        }
        _ => return None,
    };
    Some(result)
}

fn save_settings(state: &mut FixtureState) -> Result<(), String> {
    if !get(state, "storage-error").is_empty() {
        state.notice = get(state, "storage-error").into();
        return Ok(());
    }
    let mut error = None;
    if editing_component(state) == "settings.brainstorm" {
        if !valid_origin(get(state, "origin")) {
            error = Some(
                "Enter an HTTPS origin without credentials, a path prefix, query, or fragment.",
            );
        } else {
            let origin = get(state, "origin").to_string();
            put(state, "saved-origin", &origin);
            put(state, "connection", "configured");
        }
    } else if matches!(editing_component(state), "settings.boat" | "settings.gce") {
        let names: Vec<_> = get(state, "credentials")
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect();
        let paths: Vec<_> = get(state, "paths")
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect();
        let valid_names = names.len() <= 64
            && names.iter().all(|name| {
                !matches!(
                    *name,
                    "BOAT_API_KEY"
                        | "GOOGLE_APPLICATION_CREDENTIALS"
                        | "CLOUDSDK_AUTH_ACCESS_TOKEN"
                        | "GOOGLE_OAUTH_ACCESS_TOKEN"
                        | "HOME"
                        | "PATH"
                        | "SHELL"
                ) && name.len() <= 128
                    && name.bytes().enumerate().all(|(index, byte)| {
                        byte == b'_'
                            || byte.is_ascii_uppercase()
                            || (index > 0 && byte.is_ascii_digit())
                    })
            });
        let valid_paths = paths.len() <= 128
            && paths.iter().all(|path| {
                !path.contains('\0')
                    && !path.split('/').any(|part| {
                        matches!(
                            part,
                            ".." | "."
                                | ".git"
                                | ".env"
                                | "auth.json"
                                | "credentials.json"
                                | "google-services.json"
                        )
                    })
                    && !path.starts_with('/')
            });
        if !valid_names || !valid_paths || get(state, "template").len() > 128 {
            error = Some(CLOUD_ERROR);
        }
    } else {
        if flag(state, "key-invalid") {
            error = Some("The API key cannot contain spaces or control characters.");
        } else if editing_component(state) == "settings.jev" {
            let endpoint = get(state, "endpoint");
            if !valid_endpoint(endpoint) {
                error =
                    Some("Enter an HTTPS API base URL without credentials, query, or fragment.");
            } else if !valid_model(get(state, "jev-model")) {
                error = Some("Enter a valid Jev model ID using at most 128 bytes.");
            } else if flag(state, "changed-origin")
                && get(state, "key-mask").is_empty()
                && !flag(state, "remove-key")
            {
                error = Some("Add an API key for the new gateway or remove the saved key.");
            }
        }
        if error.is_none() {
            if flag(state, "remove-key") {
                state.flags.insert("key-configured".into(), false);
            } else if !get(state, "key-mask").is_empty() {
                state.flags.insert("key-configured".into(), true);
            }
            put(state, "key-mask", "");
            state.flags.insert("changed-origin".into(), false);
            state.flags.insert("remove-key".into(), false);
            if get(state, "model").trim().is_empty() {
                put(state, "model", "openrouter/free");
            }
        }
    }
    if let Some(error) = error {
        put(state, "error", error);
        state.notice = "The fixture keeps its previous saved settings.".into();
    } else {
        put(state, "error", "");
        state.notice = "Saved the synthetic settings in this fixture.".into();
        snapshot_settings(state);
        retain_plugin_connection(state);
        if state.component == "plugins.manager" {
            state.stage = "models".into();
            state.selected = number(state, "manager-selected", state.selected).min(7);
        }
    }
    Ok(())
}

fn editing_component(state: &FixtureState) -> &str {
    if state.component == "plugins.manager" && !get(state, "editor-kind").is_empty() {
        get(state, "editor-kind")
    } else {
        &state.component
    }
}

const SAVED_FIELDS: [&str; 15] = [
    "model",
    "jev-model",
    "endpoint",
    "gateway",
    "origin",
    "saved-origin",
    "mode",
    "size",
    "template",
    "credentials",
    "paths",
    "reasoning",
    "output",
    "connection",
    "house-key",
];

fn snapshot_settings(state: &mut FixtureState) {
    for field in SAVED_FIELDS {
        let value = get(state, field).to_string();
        put(state, &format!("saved.{field}"), &value);
    }
    state
        .flags
        .insert("saved.key-configured".into(), flag(state, "key-configured"));
}

fn restore_settings(state: &mut FixtureState) {
    for field in SAVED_FIELDS {
        let value = get(state, &format!("saved.{field}")).to_string();
        put(state, field, &value);
    }
    if get(state, "connection") == "checking" {
        put(state, "connection", "unchecked");
    }
    state
        .flags
        .insert("key-configured".into(), flag(state, "saved.key-configured"));
    state.flags.insert("remove-key".into(), false);
    state.flags.insert("changed-origin".into(), false);
    state.flags.insert("key-invalid".into(), false);
    put(state, "key-mask", "");
    put(state, "error", "");
}

fn retain_plugin_connection(state: &mut FixtureState) {
    let plugin = get(state, "connection-plugin").to_string();
    if !plugin.is_empty() {
        let connection = get(state, "connection").to_string();
        put(state, &format!("connection:{plugin}"), &connection);
        state.flags.insert(
            format!("key-configured:{plugin}"),
            flag(state, "key-configured"),
        );
    }
}

fn valid_model(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/:".contains(&byte))
}

fn valid_endpoint(value: &str) -> bool {
    let authority = value.strip_prefix("https://").or_else(|| {
        value.strip_prefix("http://").filter(|rest| {
            let authority = rest.split('/').next().unwrap_or_default();
            let host = if authority.starts_with('[') {
                authority
                    .split(']')
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches('[')
            } else {
                authority.split(':').next().unwrap_or_default()
            };
            host.parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
        })
    });
    authority.is_some_and(|rest| {
        !rest.is_empty()
            && !rest.starts_with('/')
            && !rest.contains('@')
            && !rest.contains('?')
            && !rest.contains('#')
    }) && value.len() <= 2048
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn valid_origin(value: &str) -> bool {
    value.strip_prefix("https://").is_some_and(|rest| {
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        !rest.is_empty()
            && !rest.contains('/')
            && !rest.contains('@')
            && !rest.contains('?')
            && !rest.contains('#')
    }) && value.len() <= 2048
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn same_origin(left: &str, right: &str) -> bool {
    let origin = |value: &str| {
        let (scheme, rest) = value.trim().split_once("://")?;
        let mut authority = rest.split('/').next()?.to_ascii_lowercase();
        if authority.is_empty()
            || authority.contains('@')
            || authority.contains('?')
            || authority.contains('#')
        {
            return None;
        }
        if scheme == "https" && authority.ends_with(":443") {
            authority.truncate(authority.len() - 4);
        }
        if scheme == "http" && authority.ends_with(":80") {
            authority.truncate(authority.len() - 3);
        }
        Some(format!("{}://{authority}", scheme.to_ascii_lowercase()))
    };
    match (origin(left), origin(right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

fn model_choice_count(state: &FixtureState) -> usize {
    match state.stage.as_str() {
        "reasoning" => reasoning_choices(state).len(),
        "output" => output_choices(state).len(),
        _ => matching_models(state).len(),
    }
}

fn select_model(state: &mut FixtureState) -> Result<(), String> {
    match state.stage.as_str() {
        "models" => {
            let models = matching_models(state);
            let Some(model) = models.get(state.selected).copied() else {
                return Err("Choose a matching fixture model.".into());
            };
            put(state, "pending-model", model.id);
            if model.id != get(state, "active-model") {
                put(state, "reasoning", model.default);
                put(state, "output", "");
            }
            state.stage = if flag(state, "reasoning-supported") && !model.efforts.is_empty() {
                "reasoning"
            } else if flag(state, "output-supported") {
                "output"
            } else {
                "closed"
            }
            .into();
        }
        "reasoning" => {
            let choices = reasoning_choices(state);
            let Some((effort, _, _)) = choices.get(state.selected) else {
                return Err("Choose a supported reasoning level.".into());
            };
            put(state, "reasoning", effort);
            state.stage = if flag(state, "output-supported") {
                "output"
            } else {
                "closed"
            }
            .into();
        }
        "output" => {
            let choices = output_choices(state);
            let Some(tokens) = choices.get(state.selected) else {
                return Err("Choose a supported output limit.".into());
            };
            put(
                state,
                "output",
                &if *tokens == 0 {
                    String::new()
                } else {
                    tokens.to_string()
                },
            );
            state.stage = "closed".into();
        }
        _ => return Err("Open the fixture picker before choosing a model.".into()),
    }
    if state.stage == "closed" {
        let model = get(state, "pending-model").to_string();
        put(state, "active-model", &model);
        put(state, "model", &model);
        state.notice = format!("Selected {model} for the fixture.");
    }
    state.selected = match state.stage.as_str() {
        "reasoning" => reasoning_choices(state)
            .iter()
            .position(|(effort, _, _)| *effort == get(state, "reasoning"))
            .unwrap_or(0),
        "output" => output_choices(state)
            .iter()
            .position(|tokens| {
                if *tokens == 0 {
                    get(state, "output").is_empty()
                } else {
                    tokens.to_string() == get(state, "output")
                }
            })
            .unwrap_or(0),
        _ => 0,
    };
    Ok(())
}

fn toggle_acp(state: &mut FixtureState) {
    if state.selected < number(state, "acp-count", 11) {
        let key = format!("acp:{}", state.selected);
        state.flags.insert(key.clone(), !flag(state, &key));
    }
}

fn move_selection(state: &mut FixtureState, delta: i32, count: usize) {
    state.selected = (state.selected as i64 + i64::from(delta))
        .clamp(0, count.saturating_sub(1) as i64) as usize;
}

fn move_named_selection(state: &mut FixtureState, delta: i32, field: &str, default: usize) {
    let count = number(state, field, default);
    move_selection(state, delta, count);
}

fn put(state: &mut FixtureState, name: &str, value: &str) {
    state.fields.insert(name.into(), value.into());
}
fn get<'a>(state: &'a FixtureState, name: &str) -> &'a str {
    state
        .fields
        .get(name)
        .map(String::as_str)
        .unwrap_or_default()
}
fn flag(state: &FixtureState, name: &str) -> bool {
    state.flags.get(name).copied().unwrap_or(false)
}
fn number(state: &FixtureState, name: &str, default: usize) -> usize {
    get(state, name).parse().unwrap_or(default)
}
fn focused(state: &FixtureState, field: &str) -> bool {
    get(state, "focus") == field
}

fn node(key: &str, element: Element<CatalogIntent>) -> Node<CatalogIntent> {
    Node {
        key: key.into(),
        style: crate::source_theme::style(),
        element,
    }
}

fn column(key: &str, children: Vec<Node<CatalogIntent>>) -> Node<CatalogIntent> {
    node(
        key,
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}
fn row(key: &str, children: Vec<Node<CatalogIntent>>) -> Node<CatalogIntent> {
    node(
        key,
        Element::Stack {
            axis: Axis::Horizontal,
            children,
        },
    )
}
fn text(key: &str, value: impl Into<String>, color: Color) -> Node<CatalogIntent> {
    let mut result = node(
        key,
        Element::Text {
            value: value.into(),
            role: TextRole::Body,
        },
    );
    result.style.foreground = Some(color);
    result
}
fn code(key: &str, value: impl Into<String>) -> Node<CatalogIntent> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role: TextRole::Code,
        },
    )
}

fn detail(key: &str, label: &str, value: impl AsRef<str>, width: u16) -> Node<CatalogIntent> {
    node(
        key,
        Element::RichText {
            runs: vec![
                run(
                    if width < 40 {
                        format!("{label}  ")
                    } else {
                        format!("{label:<16}")
                    },
                    GRAY,
                    false,
                ),
                run(value.as_ref(), SECONDARY, false),
            ],
            role: TextRole::Body,
        },
    )
}

fn action(
    key: &str,
    label: impl Into<String>,
    name: &str,
    enabled: bool,
    color: Color,
    selected: bool,
) -> Node<CatalogIntent> {
    let label = label.into();
    let mut result = node(
        key,
        Element::Button {
            label: format!("{} {label}", if selected { "❯" } else { " " }),
            enabled,
            icon: None,
            shortcut: None,
            intent: CatalogIntent::Action { name: name.into() },
        },
    );
    result.style.foreground = Some(color);
    result.style.weight = Some(if selected {
        TextWeight::Bold
    } else {
        TextWeight::Normal
    });
    result.style.button_padding = Some([0, 0]);
    result.style.radius = Some(0);
    result.style.min_height = Some(20);
    result.style.align = Some(TextAlign::Start);
    result
}

fn choice(
    key: &str,
    label: impl Into<String>,
    selected: bool,
    intent: CatalogIntent,
    color: Color,
) -> Node<CatalogIntent> {
    let mut result = node(
        key,
        Element::Choice {
            label: label.into(),
            selected,
            enabled: true,
            intent,
            children: Vec::new(),
        },
    );
    result.style.foreground = Some(color);
    result.style.background = Some(if selected { DARK } else { BASE });
    result.style.weight = Some(if selected {
        TextWeight::Bold
    } else {
        TextWeight::Normal
    });
    result.style.button_padding = Some([0, 0]);
    result.style.radius = Some(0);
    result.style.min_height = Some(20);
    result.style.align = Some(TextAlign::Start);
    result
}

fn run(text: impl Into<String>, foreground: Color, bold: bool) -> RichRun {
    RichRun {
        text: text.into(),
        foreground: Some(foreground),
        background: None,
        bold,
        italic: false,
        underline: false,
        strike: false,
        dim: false,
    }
}

fn rich_choice(
    key: &str,
    label: impl Into<String>,
    selected: bool,
    intent: CatalogIntent,
    runs: Vec<RichRun>,
) -> Node<CatalogIntent> {
    let mut result = choice(key, label, selected, intent, PRIMARY);
    let mut content = node(
        &format!("{key}-runs"),
        Element::RichText {
            runs,
            role: TextRole::Terminal,
        },
    );
    content.style.background = Some(if selected { DARK } else { BASE });
    if let Element::Choice { children, .. } = &mut result.element {
        children.push(content);
    }
    result
}

fn field(
    key: &str,
    label: &str,
    value: &str,
    placeholder: &str,
    secret: bool,
    enabled: bool,
    state: &FixtureState,
) -> Node<CatalogIntent> {
    let mut result = node(
        &format!("field-{key}"),
        Element::Field {
            label: label.into(),
            value: if secret { String::new() } else { value.into() },
            placeholder: if secret && !value.is_empty() {
                value.into()
            } else {
                placeholder.into()
            },
            secret,
            multiline: false,
            enabled,
            max_bytes: 8192,
            on_change: CatalogIntent::Input { field: key.into() },
        },
    );
    result.style.foreground = Some(
        if focused(state, key) || (key == "jev-model" && focused(state, "model")) {
            PRIMARY
        } else {
            SECONDARY
        },
    );
    result.style.border = Some(
        if focused(state, key) || (key == "jev-model" && focused(state, "model")) {
            BORDER
        } else {
            LIGHT
        },
    );
    result.style.radius = Some(0);
    result.style.padding_points = Some([0, 9, 0, 9]);
    result
}

fn screen(
    title: &str,
    children: Vec<Node<CatalogIntent>>,
    state: &FixtureState,
) -> Node<CatalogIntent> {
    let mut heading = text("settings-screen-title", title, PRIMARY);
    heading.style.weight = Some(TextWeight::Bold);
    let mut header = vec![heading];
    if state.width >= 42 {
        let mut context = node(
            "settings-screen-context",
            Element::RichText {
                runs: vec![
                    run("openagents", PRIMARY, false),
                    run(" / main", GRAY, false),
                ],
                role: TextRole::Terminal,
            },
        );
        context.style.align = Some(TextAlign::End);
        header.push(context);
    }
    let mut body = column("settings-screen-body", children);
    body.style.viewport = Some(Viewport {
        max_height: state.height.saturating_sub(4).max(1) * 20,
        offset: 0,
        fade: 0,
    });
    let mut result = column(
        "settings-screen",
        vec![
            row("settings-screen-header", header),
            text("settings-screen-gap", " ", GRAY),
            body,
        ],
    );
    result.style.background = Some(BASE);
    result.style.padding_points = Some([20, 18, 20, 18]);
    result.style.fill_height = Some(true);
    result
}

fn comma(value: u32) -> String {
    let digits = value.to_string();
    let mut result = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_masked_before_the_fixture_state_retains_them() {
        let mut state = FixtureState::default_for("settings.openrouter", "default");
        reduce(
            &mut state,
            &CatalogIntent::Input {
                field: "key".into(),
            },
            Some("never-retain-this-value"),
        )
        .unwrap()
        .unwrap();
        assert!(
            state
                .fields
                .values()
                .all(|value| !value.contains("never-retain"))
        );
        assert!(!get(&state, "key-mask").is_empty());
    }

    #[test]
    fn long_disclosures_require_complete_review() {
        let mut state = FixtureState::default_for("approvals.disclosure", "long");
        assert!(
            reduce(
                &mut state,
                &CatalogIntent::Action {
                    name: "disclosure.confirm".into()
                },
                None
            )
            .unwrap()
            .is_err()
        );
        reduce(
            &mut state,
            &CatalogIntent::Action {
                name: "disclosure.end".into(),
            },
            None,
        )
        .unwrap()
        .unwrap();
        assert!(
            reduce(
                &mut state,
                &CatalogIntent::Action {
                    name: "disclosure.confirm".into()
                },
                None
            )
            .unwrap()
            .is_err()
        );
        reduce(
            &mut state,
            &CatalogIntent::Action {
                name: "review-complete".into(),
            },
            None,
        )
        .unwrap()
        .unwrap();
        reduce(
            &mut state,
            &CatalogIntent::Action {
                name: "disclosure.confirm".into(),
            },
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(state.stage, "confirmed");
    }

    #[test]
    fn model_selection_skips_unsupported_stages_and_caps_output() {
        let mut state = FixtureState::default_for("models.picker", "no-options");
        select_model(&mut state).unwrap();
        assert_eq!(state.stage, "closed");
        let state = FixtureState::default_for("models.picker", "output-limited");
        assert_eq!(output_choices(&state), vec![0, 2048, 4096, 8192]);
    }

    #[test]
    fn follow_requires_explicit_takeover_and_holder_release() {
        let mut state = FixtureState::default_for("sessions.follow", "following");
        let action = |name: &str| CatalogIntent::Action { name: name.into() };
        reduce(&mut state, &action("follow.takeover"), None)
            .unwrap()
            .unwrap();
        assert!(!flag(&state, "takeover-acquired"));
        reduce(&mut state, &action("follow.release"), None)
            .unwrap()
            .unwrap();
        assert!(flag(&state, "takeover-acquired"));
    }
}
