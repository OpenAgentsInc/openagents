//! Haunted House review shell for the pinned, separately hosted private engine.

use super::medieval_town::{Release, response as engine_response};
fn response(
    status: StatusCode,
    body: impl Into<String>,
    kind: &'static str,
    origin: Option<&str>,
) -> Response {
    let mut result = engine_response(status, body.into(), kind, origin);
    // Voice input belongs to this host. The cross-origin engine receives no mic access.
    result.headers_mut().insert(
        "permissions-policy",
        axum::http::HeaderValue::from_static(
            "camera=(), microphone=(self), geolocation=(), payment=()",
        ),
    );
    if let Some(value) = result
        .headers()
        .get(axum::http::header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
    {
        let value = value.replace(
            "connect-src 'self'",
            "img-src data: blob:; connect-src 'self' wss://relay.openagents.com",
        );
        if let Ok(value) = value.parse() {
            result
                .headers_mut()
                .insert(axum::http::header::CONTENT_SECURITY_POLICY, value);
        }
    }
    result
}
use crate::App;
use axum::{Router, http::StatusCode, response::Response, routing::get};
use maud::{DOCTYPE, PreEscaped, html};

pub(crate) const HOUSE: &str = "/new-lander/house";
const MAP: &str = "/new-lander/verse?world=haunted-house";

fn configured(text: &str) -> Option<Release> {
    let r = Release::parse(text)?;
    (r.project.id == "haunted-house" && r.project.branch == "main").then_some(r)
}
pub(crate) fn release() -> Option<Release> {
    let text = std::env::var("OPENAGENTS_HAUNT_RELEASE")
        .unwrap_or_else(|_| include_str!("../../content/demos/haunted-house.json").into());
    configured(&text)
}

pub(crate) fn engine_release() -> Option<String> {
    release().map(|r| r.release)
}

pub(crate) fn available() -> bool {
    release().is_some()
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(HOUSE, get(page))
        .route("/new-lander/house/boot.js", get(boot))
        .route("/new-lander/house/style.css", get(style))
        .route(
            "/new-lander/house/{*file}",
            get(|| async { StatusCode::NOT_FOUND }),
        )
}

async fn page(axum::extract::State(app): axum::extract::State<App>) -> Response {
    let Some(r) = release() else {
        return response(StatusCode::SERVICE_UNAVAILABLE, html! {
            (DOCTYPE) html lang="en" { head { meta name="viewport" content="width=device-width,initial-scale=1"; title { "Haunted House · OpenAgents" } }
                body { h1 { "The house isn't open here yet." } p { "You can return to the planets and keep exploring." }
                    a href=(HOUSE) { "Try again" } " · " a href=(MAP) { "Back to planets" }
                }
            }
        }.into_string(), "text/html; charset=utf-8", None);
    };
    render(&app, r, None)
}
pub(crate) fn personal_page(app: &App, planet: &aethyrforge_host::PersonalPlanet) -> Response {
    let Some(runtime) = app.config.verse_personal_runtime.as_ref() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Your courtyard isn't available here yet. Return to the planets and try again later.",
            "text/plain; charset=utf-8",
            None,
        );
    };
    let r = Release {
        engine_url: runtime.engine_url.clone(),
        sdk_url: runtime.sdk_url.clone(),
        release: runtime.engine_release.clone(),
        engine_abi: runtime.engine_abi.clone(),
        project_format: runtime.project_format,
        project: aethyrforge_host::Project {
            id: "personal-planet".into(),
            branch: "main".into(),
            revision: 0,
        },
    };
    render(app, r, Some(planet))
}
fn render(app: &App, r: Release, planet: Option<&aethyrforge_host::PersonalPlanet>) -> Response {
    let personal = planet.is_some();
    let title = planet.map(|p| p.name.as_str()).unwrap_or("Haunted House");
    let runtime = if personal {
        app.config.verse_personal_runtime.as_ref()
    } else {
        app.config.verse_publish_runtime.as_ref()
    };
    let config = serde_json::json!({"release":r.release,"engine_abi":r.engine_abi,"project_format":r.project_format,"project":r.project});
    let text = serde_json::to_string(&config)
        .expect("configuration")
        .replace('<', "\\u003c");
    // Only public catalog metadata enters the host. The engine independently checks availability.
    let assets: Vec<aethyrforge_host::CreatorAsset> = aethyrforge_host::FantasyCatalog::decode(
        include_bytes!("../../../../docs/verse/fantasy-props-catalog.json"),
    )
    .map(|c| {
        c.runtime
            .into_iter()
            .filter(|a| !a.id.starts_with("medieval-town:"))
            .map(|a| aethyrforge_host::CreatorAsset {
                id: a.id,
                name: a.name.replace('_', " "),
            })
            .collect()
    })
    .unwrap_or_default();
    let assets_json = serde_json::to_string(&assets)
        .expect("asset metadata")
        .replace('<', "\\u003c");
    let body = html! {
        (DOCTYPE) html lang="en" { head {
            meta id="world-namespace" content=(if personal { "personal-planet" } else { "haunted-house" });
            @if let Some(planet) = planet {
                meta id="personal-owner" content=(&planet.owner);
                meta id="personal-project" content=(&planet.project);
                @if let Some(runtime) = runtime {
                    script id="personal-template" type="application/json" { (PreEscaped(serde_json::to_string(&aethyrforge_host::WorldTemplate {world:runtime.template_world.clone(), engine_release:runtime.engine_release.clone(), asset_set_sha256:runtime.asset_set_sha256.clone(), scene_sha256:runtime.scene_sha256.clone()}).unwrap().replace('<',"\\u003c"))) }
                }
            }
            meta id="engine-release" content=(r.release);
            meta id="publish-enabled" content=(runtime.is_some_and(|p|p.engine_release==r.release&&p.sdk_url==r.sdk_url).to_string());
            meta charset="utf-8"; meta name="viewport" content="width=device-width,initial-scale=1";
            title { (title) " · OpenAgents" }
            link rel="stylesheet" href="/new-lander/house/style.css";
        } body {
            main {
                iframe id="engine" title=(title) data-engine-url=(r.engine_url) {}
                p id="world-notice" role="status" aria-live="polite" { "Opening the house…" }
            }
            div id="world-dock" {
                nav id="world-controls" aria-label="World controls" {
                    div.hud-navigation {
                        a id="world-map" href=(MAP) title="Back to planets" { (super::verse_ui::icon("map")) span { "Back to planets" } }
                        details.hud-utilities {
                            summary aria-label="World display" title="World display" { (super::verse_ui::icon("expand")) }
                            div.hud-utility-menu {
                                a id="world-retry" href=(planet.map(|p|format!("/new-lander/planet/{}/world",p.id)).unwrap_or_else(||HOUSE.into())) { (super::verse_ui::icon("reload")) span { "Reload world" } }
                                button id="world-fullscreen" type="button" { (super::verse_ui::icon("expand")) span { "Full screen" } }
                            }
                        }
                    }
                    p id="world-touch-help" { "Left side: move · Right side: look" }
                    span { "Click to look · WASD to walk · Escape to release the pointer" }
                    div.hud-authoring {
                        button id="world-mesh" type="button" hidden aria-pressed="false" title="Mesh overlay" { (super::verse_ui::icon("mesh")) span { "Mesh overlay" } }
                        button id="world-edit" type="button" { (super::verse_ui::icon("edit")) span { "Edit" } }
                        button id="project-save" type="button" hidden data-tutorial-option="ready" title="Save project" { (super::verse_ui::icon("save")) span { "Save project" } }
                    }
                }
                @if personal {
                    aside id="planet-settings" aria-label="Courtyard settings" {
                        details class="planet-card" open {
                            summary { "Your courtyard" }
                            p { "64 × 64 metres · Up to 128 objects" }
                            label { "Style" select id="planet-style" { option value="soft-pastels" { "Soft pastels" } option value="bold-fantasy" { "Bold fantasy" } } }
                            label { "Starting hour" input id="planet-hour" type="number" min="0" max="23.99" step="0.5" value="8"; }
                            label { input id="planet-cycle" type="checkbox" checked; "Advancing day and night" }
                            label { "Minutes per day" input id="planet-day" type="number" min="2" max="60" value="10"; }
                            label { "Layout seed" input id="planet-seed" type="number" min="0" max="4294967295" value="1"; }
                            button id="planet-generate" type="button" { "Generate courtyard" }
                            button id="planet-apply" type="button" { "Review style and time" }
                            p id="planet-status" role="status" { "Generate a courtyard, or open your latest saved world in Jarvis." }
                            p { "Say Generate courtyard, or use the controls." }
                            details { summary { "Voice choices" } ul {
                                li { "Use soft pastels / Use bold fantasy" }
                                li { "Set time to 18" }
                                li { "Pause day cycle / Start day cycle" }
                                li { "Confirm change / Cancel change" }
                            } }
                        }
                    }
                }
                aside id="world-voice" hidden aria-label="Voice controls" {
                    button id="edit-voice-start" type="button" hidden { (super::verse_ui::icon("mic")) span { "Listen" } }
                    button id="edit-voice-stop" type="button" { (super::verse_ui::icon("stop")) span { "Stop / mute" } }
                    p id="edit-voice-status" role="status" aria-live="polite" { "Microphone off" }
                    p id="voice-heard" {}
                    p id="voice-response" role="status" aria-live="polite" {}
                    p class="voice-help" { "Browser speech service · say Show options, Review change, or Confirm change" }
                }
            }
                        aside id="request-choices" hidden aria-label="Request options" {
                            p.hud-section-label { "WORLD ACTIONS" }
                            button id="project-publish" type="button" hidden data-tutorial-option="ready" { (super::verse_ui::icon("share")) "Publish" }
                            button id="project-play" type="button" hidden disabled data-tutorial-option="ready" { (super::verse_ui::icon("play")) "Play" }
                            button id="project-share" type="button" hidden disabled data-tutorial-option="ready" { (super::verse_ui::icon("share")) "Share" }
                            details.hud-command-menu {
                                summary { "More world actions" }
                                div {
                                    button id="project-revert" type="button" hidden disabled data-tutorial-option="ready" { "Revert to Starting house" }
                                    @for (id,label) in [("request-inspect","Inspect world"),("request-mesh","Show mesh"),("request-map","Back to planets"),("request-compile","Compile world effect"),("request-undo","Undo")] {
                                        button id=(id) type="button" { (label) }
                                    }
                                }
                            }
                        }
            div id="edit-shadow" hidden aria-hidden="true" {}
            aside id="edit-panel" hidden role="dialog" aria-modal="true" aria-labelledby="edit-title" data-active="objects" {
                header class="workspace-header" {
                    div { p class="eyebrow" { "OPENAGENTS / STUDIO" } h1 id="edit-title" { "Jarvis" } }
                    button id="edit-close" type="button" { (super::verse_ui::icon("play")) "Inspect world" }
                }
                div id="jarvis-toolbar" hidden role="tablist" aria-label="Editing tools" {
                    @for (id,label) in [("objects","Object"),("materials","Materials"),("request","Request"),("compile","Compile"),("history","History")] {
                        button id=(format!("tab-{id}")) type="button" role="tab" aria-controls=(format!("panel-{id}")) aria-selected="false" { (super::verse_ui::icon(id)) span { (label) } }
                    }
                }
                section id="panel-objects" class="tool-window" aria-labelledby="objects-title" {
                    header { h2 id="objects-title" { "01 / Object" } button id="close-objects" class="window-close" type="button" aria-label="Close Object" { "×" } }
                    p id="edit-target" { "Click an object in the house." }
                    button id="edit-pick" type="button" { "Select in world" }
                    div id="prop-tools" hidden {
                    label for="edit-place-asset" { "Prop to add at X, Y, Z below" }
                    input id="edit-place-asset" type="text" list="edit-props" maxlength="128";
                    datalist id="edit-props" {@for asset in &assets {option value=(&asset.id) {(&asset.name)}}}
                    button id="edit-place" type="button" { "Add prop" }
                    }
                    form id="edit-form" {
                        div class="position-fields" {
                            @for (id,label) in [("edit-x","X"),("edit-y","Y"),("edit-z","Z")] { label { (label) input id=(id) type="number" step="any" required value="0"; } }
                        }
                        label for="edit-note" { "Notes" } textarea id="edit-note" maxlength="48000" rows="3" {}
                        input id="edit-submit" type="submit" value="Move object" disabled;
                    }
                }
                div id="author-tools" hidden {
                    section id="panel-materials" class="tool-window" aria-labelledby="materials-title" {
                        header { h2 id="materials-title" { "02 / Materials" } button id="close-materials" class="window-close" type="button" aria-label="Close Materials" { "×" } }
                        h3 { "Werewolf face" }
                        p { "Edit one selected werewolf. The local scar generator uses no model and has no charge." }
                        p { "Face layers are 256 × 256 PNGs, up to 16 KiB each for preview and Apply. You can export your sources if a layer is too large." }
                        button id="face-open" type="button" { "Edit selected werewolf face" }
                        fieldset id="face-controls" disabled {
                            legend { "Face color layers" }
                            label for="face-area" { "Scar area" }
                            select id="face-area" { option value="left_cheek" { "Left cheek" } option value="right_cheek" { "Right cheek" } option value="forehead" { "Forehead" } }
                            label for="face-request" { "Describe scars" }
                            textarea id="face-request" maxlength="1200" rows="2" placeholder="Two thin healed blue scars on the left cheek" {}
                            p { "Choose one cheek or the forehead, one to eight scars, thin or thick, healed or fresh, and a solid color. Painting also works on a blank face layer." }
                            details { summary { "Generator colors" } p { "Pink, brown, red, blue, green, purple, tan, black, white, orange, yellow, or gray. Use the brush for other colors." } }
                            button id="face-generate" type="button" { "Generate local scars" }
                            canvas id="face-canvas" width="256" height="256" aria-label="Face color map; drag to paint" aria-disabled="true" {}
                            label for="face-brush-mode" { "Brush" }
                            select id="face-brush-mode" { option value="paint" { "Paint" } option value="erase" { "Erase handmade paint" } }
                            label for="face-brush-color" { "Paint color" } input id="face-brush-color" type="color" value="#8096c0";
                            label for="face-brush-size" { "Brush size" } input id="face-brush-size" type="range" min="1" max="32" value="5";
                            label for="face-brush-opacity" { "Brush opacity" } input id="face-brush-opacity" type="range" min="0.05" max="1" step="0.05" value="1";
                            label class="check-label" { input id="face-generated-visible" type="checkbox" checked; "Show generated scars" }
                            label for="face-generated-strength" { "Scar strength" } input id="face-generated-strength" type="range" min="0" max="1" step="0.05" value="1";
                            label class="check-label" { input id="face-handmade-visible" type="checkbox" checked; "Show handmade paint" }
                            label for="face-handmade-strength" { "Paint strength" } input id="face-handmade-strength" type="range" min="0" max="1" step="0.05" value="1";
                            button id="face-preview" type="button" { "Preview face" }
                            button id="face-cancel" type="button" { "Cancel preview" }
                            button id="face-apply" type="button" { "Review face change" }
                        }
                        button id="face-export" type="button" disabled { "Export editable face layers" }
                        button id="face-discard" type="button" disabled { "Discard face draft" }
                        label for="face-import" { "Open an exported face-layer file" } input id="face-import" type="file" accept="application/json,.json";
                        p id="face-status" role="status" { "Select a werewolf in the house to begin." }
                        details { summary { "Shared material and fog" }
                        label for="edit-material" { "Shared material ID (all surfaces using this ID)" } input id="edit-material" type="text" maxlength="128";
                        p id="edit-material-ids" {}
                        label for="edit-parameter" { "Parameter: red, green, blue, roughness, metallic" } input id="edit-parameter" type="text" value="roughness";
                        label for="edit-value" { "Value" } input id="edit-value" type="number" step="any" value="0.8";
                        button id="edit-material-apply" type="button" { "Apply material" }
                        label for="edit-fog" { "Fog density (0–0.06)" } input id="edit-fog" type="number" step="0.001" min="0" max="0.06" value="0.01";
                        button id="edit-fog-apply" type="button" { "Apply fog" }
                        }
                    }
                    section id="panel-request" class="tool-window" aria-labelledby="request-title" {
                        header { h2 id="request-title" { "03 / Kiln" } button id="close-request" class="window-close" type="button" aria-label="Close Request" { "×" } }
                        label for="edit-kiln-request" { "Request" } textarea id="edit-kiln-request" maxlength="4800" rows="5" {}
                        label class="check-label" { input id="edit-kiln-world" type="checkbox"; "Allow a world effect (all materials)" }
                        button id="edit-request-review" type="button" { "Review request" }
                        button id="edit-kiln-ask" type="button" hidden { "Ask Kiln" }
                        button id="request-options" type="button" { "Show options" }
                        button id="edit-kiln-stop" type="button" { "Stop" }
                        button id="edit-kiln-review" type="button" { "Review Kiln's change" }
                        p id="edit-kiln-status" role="status" {}
                    }
                    section id="panel-compile" class="tool-window" aria-labelledby="compile-title" {
                        header { h2 id="compile-title" { "04 / World effect" } button id="close-compile" class="window-close" type="button" aria-label="Close Compile" { "×" } }
                        p { "Applies to every material in the house." }
                        label for="edit-source" { "WGSL source" } textarea id="edit-source" maxlength="4096" rows="8" spellcheck="false" {}
                        button id="edit-source-draft" type="button" { "Keep world-effect draft" }
                        button id="edit-source-compile" type="button" { "Compile world effect" }
                        p id="edit-source-status" role="status" {}
                        section id="publish-tools" hidden aria-label="Playable releases" {
                            h3 { "Playable versions" }
                            p { "Compile and inspect your change, then save before publishing. Revert returns to Starting house as a new change." }
                            label for="publish-visibility" { "Who can play" }
                            select id="publish-visibility" { option value="private" { "Project members" } option value="public" { "Anyone with the link" } }
                            button id="publish-refresh" type="button" { "Find published versions" }
                            label for="publish-version" { "Published version" } select id="publish-version" {}
                            button id="publish-rollback" type="button" { "Review shared-link rollback" }
                            button id="publish-revoke" type="button" { "Review turning off a link" }
                            a id="published-link" hidden { "Open this playable version" }
                            a id="visit-manage" hidden { "Invite people" }
                            p id="publish-status" role="status" {}
                        }
                        button id="edit-capture" type="button" { "Capture preview" }
                        img id="edit-capture-image" hidden alt="Captured view of the house" width="280";
                    }
                    section id="panel-history" class="tool-window" aria-labelledby="history-title" {
                        header { h2 id="history-title" { "05 / This visit" } button id="close-history" class="window-close" type="button" aria-label="Close History" { "×" } }
                        button id="edit-undo" type="button" { "Undo" } button id="edit-redo" type="button" { "Redo" }
                        ol id="edit-history-entries" {}
                        div id="project-tools" hidden {
                            section id="contribution-tools" hidden {
                                h3 { "Saved changes" }
                                button id="project-history-refresh" type="button" { "Refresh saved changes" }
                                p id="project-history-status" role="status" {}
                                ol id="project-history-rows" {}
                                button id="project-history-more" type="button" hidden { "Show earlier changes" }
                                section id="project-history-detail" hidden aria-labelledby="project-history-version" {
                                    h4 id="project-history-version" {}
                                    ul id="project-history-changes" {}
                                    p id="project-history-change-limit" {}
                                    button id="project-history-download" type="button" { "Download editable version" }
                                }
                            }
                            h3 { "Saved worlds" }
                            label for="project-picker" { "Project" } select id="project-picker" { option value="" { "Choose a project" } }
                            button id="project-list" type="button" { "Find projects" }
                            label for="project-name" { "New project name" } input id="project-name" maxlength="120";
                            button id="project-create" type="button" { "Create project" }
                            button id="project-open" type="button" { "Open latest" }
                            button id="project-save-copy" type="button" { "Save project" }
                            button id="project-undo" type="button" { "Undo my saved change" }
                            button id="project-device" type="button" { "Keep on this device" }
                            button id="project-recover" type="button" { "Open device copy" }
                            button id="project-export" type="button" { "Export editable copy" }
                            p id="project-status" role="status" { "Changes aren't saved to a project yet." }
                            details {
                                summary { "Import an editable or older Kiln save" }
                                p { "Export files from the older editor first. Choose its scene, material, and changes files together. Originals stay in your editable export." }
                                input id="project-import-files" type="file" accept="application/json,.json,.wgsl" multiple;
                                label for="project-import-mapping" { "Old asset name to current asset ID (JSON)" }
                                textarea id="project-import-mapping" rows="3" maxlength="4096" { "{}" }
                                button id="project-import" type="button" { "Review import" }
                                p id="project-import-report" {}
                            }
                        }
                        p id="visit-earnings" { "Earnings: " strong { "Not recorded" } }
                    }
                }
                p id="edit-status" role="status" aria-live="polite" {}
            }
            dialog id="edit-auth" hidden {
                section role="dialog" aria-modal="true" aria-labelledby="edit-auth-title" {
                    h2 id="edit-auth-title" { "Sign in to make this change" }
                    p id="edit-auth-description" { "Your view and requested change will stay here." }
                    ul id="edit-auth-changes" {}
                    a id="edit-signin" href="/login?return_to=%2Fnew-lander%2Fhouse" { "Sign in" }
                    button id="edit-confirm" type="button" hidden { "Confirm change" }
                    button id="edit-auth-voice-stop" type="button" { "Stop / mute" }
                    button id="edit-cancel" type="button" { "Cancel" }
                }
            }
            dialog id="publish-review" hidden aria-labelledby="publish-review-title" {
                section { h2 id="publish-review-title" { "Publish this saved version?" }
                    p id="publish-review-target" {}
                    a id="publish-review-link" hidden target="_blank" rel="noopener" { "Open the chosen playable version" }
                    p id="publish-review-description" {}
                    label { input id="publish-confirm-rights" type="checkbox"; span id="publish-confirm-label" { "Confirm this release change." } }
                    p { "Say Confirm publish or Cancel publish, or use the buttons." }
                    button id="publish-confirm" type="button" { "Confirm release change" }
                    button id="publish-cancel" type="button" { "Cancel" }
                }
            }
            aside id="publish-share-card" hidden aria-label="Share this version" {
                p { "Exact playable version" } a id="publish-share-link" {}
                button id="publish-share-close" type="button" { "Close" }
            }
            script id="creator-assets" type="application/json" {(PreEscaped(assets_json))}
            script id="engine-config" type="application/json" { (PreEscaped(text)) }
            script type="module" src=(if personal { "/new-lander/planet/boot.js" } else { "/new-lander/house/boot.js" }) {}
        } }
    }
    .into_string();
    response(
        StatusCode::OK,
        super::verse_ui::document(body, "studio"),
        "text/html; charset=utf-8",
        Some(&r.origin()),
    )
}

async fn boot() -> Response {
    let Some(r) = release() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "",
            "text/javascript; charset=utf-8",
            None,
        );
    };
    boot_release(r)
}
pub(crate) async fn personal_boot(
    axum::extract::State(app): axum::extract::State<App>,
) -> Response {
    let Some(runtime) = app.config.verse_personal_runtime.as_ref() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "",
            "text/javascript; charset=utf-8",
            None,
        );
    };
    boot_release(Release {
        engine_url: runtime.engine_url.clone(),
        sdk_url: runtime.sdk_url.clone(),
        release: runtime.engine_release.clone(),
        engine_abi: runtime.engine_abi.clone(),
        project_format: runtime.project_format,
        project: aethyrforge_host::Project {
            id: "personal-planet".into(),
            branch: "main".into(),
            revision: 0,
        },
    })
}
fn boot_release(r: Release) -> Response {
    let url = serde_json::to_string(&r.sdk_url).expect("SDK URL");
    let kiln_url = serde_json::to_string(&r.sdk_url.replace("aethyrforge_host.js", "kiln.js"))
        .expect("Kiln URL");
    response(
        StatusCode::OK,
        format!(
            "window.loadVerseKiln=async context=>{{const k=await import({kiln_url});await k.default();await k.start_verse('/kiln/',context);}};try{{const {{default:init,mount_studio}}=await import({url});await init();window.hauntDemo=mount_studio();}}catch{{const n=document.getElementById('world-notice');n.hidden=false;n.dataset.state='error';n.textContent='The house could not open. Reload to try again.';}}"
        ),
        "text/javascript; charset=utf-8",
        Some(&r.origin()),
    )
}

async fn style() -> Response {
    response(
        StatusCode::OK,
        include_str!("../../content/new-lander-house.css"),
        "text/css; charset=utf-8",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header},
    };
    use tower::ServiceExt;

    #[test]
    fn registered_creator_runtimes_match_the_shipped_worlds() {
        let house: aethyrforge_host::PublishRuntime = serde_json::from_str(include_str!(
            "../../content/demos/runtimes/haunted-house.json"
        ))
        .unwrap();
        let personal: aethyrforge_host::PublishRuntime = serde_json::from_str(include_str!(
            "../../content/demos/runtimes/personal-planet.json"
        ))
        .unwrap();
        house.validate_current().unwrap();
        personal.validate_current().unwrap();
        let demo: serde_json::Value =
            serde_json::from_str(include_str!("../../content/demos/haunted-house.json")).unwrap();
        assert_eq!(house.engine_url, demo["engine_url"]);
        assert_eq!(house.sdk_url, demo["sdk_url"]);
        assert_eq!(house.engine_release, demo["release"]);
        assert_eq!(house.template_world, "haunted-house");
        assert_eq!(personal.template_world, "personal-planet");
        assert_ne!(personal.engine_url, house.engine_url);
    }

    #[tokio::test]
    async fn new_lander_house_never_serves_legacy_engine_or_source_files() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        for file in [
            "verse/index.html",
            "material.wgsl",
            "package.json",
            "src/main.ts",
            "assets/old.js",
            "kiln/pkg/kiln.js",
        ] {
            let uri = format!("{HOUSE}/{file}");
            let result = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&uri)
                        .header(header::HOST, "127.0.0.1:4300")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(result.status(), StatusCode::NOT_FOUND);
            assert!(crate::upstream::owned(&uri));
            assert!(to_bytes(result.into_body(), 4096).await.unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn new_lander_house_gates_creator_tools_and_names_effect_scope() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().join("tasks")));
        let response = app
            .oneshot(
                Request::builder()
                    .uri(HOUSE)
                    .header(header::HOST, "127.0.0.1:4300")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap();
        assert!(csp.contains("wss://relay.openagents.com"));
        assert_eq!(
            response.headers()["permissions-policy"],
            "camera=(), microphone=(self), geolocation=(), payment=()"
        );
        let read_only =
            super::super::medieval_town::response(StatusCode::OK, "", "text/html", None);
        assert_eq!(
            read_only.headers()["permissions-policy"],
            "camera=(), microphone=(), geolocation=(), payment=()"
        );
        let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("id=\"author-tools\" hidden"));
        assert!(text.contains("World effect"));
        for panel in ["objects", "materials", "request", "compile", "history"] {
            assert!(text.contains(&format!("id=\"panel-{panel}\"")));
            assert!(text.contains(&format!("id=\"tab-{panel}\"")));
        }
        assert!(text.contains("Earnings: "));
        assert!(text.contains("Not recorded"));
        assert!(text.contains("Applies to every material in the house."));
        assert!(text.contains("Allow a world effect (all materials)"));
        assert!(!text.contains("id=\"edit-kiln-world\" type=\"checkbox\" checked"));
    }

    #[test]
    fn new_lander_house_refuses_another_world_or_branch() {
        let base = serde_json::json!({"engine_url":"https://engine.example/","sdk_url":"https://engine.example/sdk/hash/aethyrforge_host.js","release":"house-1","engine_abi":"abi-1","project_format":1,"project":{"id":"haunted-house","branch":"main","revision":0}});
        assert!(configured(&base.to_string()).is_some());
        for (field, value) in [("id", "medieval-town-lab"), ("branch", "someone-else")] {
            let mut bad = base.clone();
            bad["project"][field] = value.into();
            assert!(configured(&bad.to_string()).is_none());
        }
    }
}
