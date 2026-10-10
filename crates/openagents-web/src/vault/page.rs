//! The vault page, its script, styles and WebAssembly, and the digests
//! that pin them.
//!
//! The page runs only this site's vault script and the `oa-vault` build: it
//! is otherwise scriptless, its policy allows no inline script, and Trusted
//! Types forbid every HTML sink, so the script fills the page from
//! `<template>` clones and `textContent` only. The script and the
//! WebAssembly glue load with Subresource Integrity, and the script checks
//! the WebAssembly's SHA-384 before running it. `GET /vault/release.json`
//! publishes the same digests.

use std::sync::OnceLock;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonVariant, Color, ControlSize};
use openagents_ui::forms::Textarea;
use sha2::{Digest, Sha256, Sha384};

use super::api::{CSRF_SCOPE, TOKEN_HEADER};
use super::{PAGE, RELEASE, SCRIPT, STYLE};
use crate::App;
use crate::cloud::protect;
use crate::ui_page::action_link;

/// The policy for the vault page: this site's scripts and WebAssembly, the
/// person's own local model server (On this device), and no HTML sinks.
pub(crate) const POLICY: &str = "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; \
     style-src 'self'; font-src 'self'; img-src 'self' blob:; \
     connect-src 'self' http://127.0.0.1:8091 http://localhost:8091; \
     base-uri 'none'; form-action 'self'; frame-ancestors 'none'; \
     require-trusted-types-for 'script'; trusted-types 'none'";

/// The glue and the module `wasm-bindgen` writes for `oa-vault-web`, in the
/// chat build directory (`--chat-build`).
const GLUE: &str = "oa_vault_web.js";
const WASM: &str = "oa_vault_web_bg.wasm";
const GLUE_PATH: &str = "/vault/assets/oa_vault_web.js";

const SOURCE: &str = include_str!("../../static/vault.js");
const STYLES: &str = include_str!("../../static/vault.css");

/// SHA-384 as an integrity value, and SHA-256 as hex.
fn digests(bytes: &[u8]) -> (String, String) {
    let sri = format!(
        "sha384-{}",
        base64::engine::general_purpose::STANDARD.encode(Sha384::digest(bytes))
    );
    let hex = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    (sri, hex)
}

#[derive(Clone)]
struct Build {
    glue: Vec<u8>,
    wasm: Vec<u8>,
}

/// The WebAssembly build, read once.
fn build(app: &App) -> Option<&'static Build> {
    static BUILD: OnceLock<Option<Build>> = OnceLock::new();
    BUILD
        .get_or_init(|| {
            let dir = app.config.chat_build.as_ref()?;
            Some(Build {
                glue: std::fs::read(dir.join(GLUE)).ok()?,
                wasm: std::fs::read(dir.join(WASM)).ok()?,
            })
        })
        .as_ref()
}

/// `GET /vault/vault.js`.
pub(crate) async fn script() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        SOURCE,
    )
        .into_response()
}

/// `GET /vault/vault.css`.
pub(crate) async fn style() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/css; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        STYLES,
    )
        .into_response()
}

/// `GET /vault/assets/{file}`: the glue and the module.
pub(crate) async fn asset(State(app): State<App>, Path(file): Path<String>) -> Response {
    let Some(build) = build(&app) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let (content_type, bytes) = match file.as_str() {
        GLUE => ("text/javascript; charset=utf-8", build.glue.clone()),
        WASM => ("application/wasm", build.wasm.clone()),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        bytes,
    )
        .into_response()
}

/// `GET /vault/release.json`: what the vault page runs, by digest.
pub(crate) async fn release(State(app): State<App>) -> Response {
    let (script_sri, script_sha) = digests(SOURCE.as_bytes());
    let mut files = vec![serde_json::json!({
        "path": SCRIPT, "sha256": script_sha, "integrity": script_sri,
    })];
    if let Some(build) = build(&app) {
        for (path, bytes) in [
            (GLUE_PATH, &build.glue),
            ("/vault/assets/oa_vault_web_bg.wasm", &build.wasm),
        ] {
            let (sri, sha) = digests(bytes);
            files.push(serde_json::json!({ "path": path, "sha256": sha, "integrity": sri }));
        }
    }
    (
        [(header::CACHE_CONTROL, "no-cache")],
        axum::Json(serde_json::json!({
            "v": "openagents.vault-release.v1",
            "commit": option_env!("OPENAGENTS_COMMIT").unwrap_or("unknown"),
            "files": files,
        })),
    )
        .into_response()
}

/// The Settings row that opens the vault.
pub(crate) fn settings_row() -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby="settings-vault" {
            h2 #settings-vault { "Vault" }
            div class="oa-settings-row" {
                div class="oa-settings-text" {
                    span class="oa-settings-label" { "Vault (only you)" }
                    span class="oa-settings-hint" {
                        "Files that only your devices can open. We store them locked and can't open them."
                    }
                }
                div class="oa-settings-control" { (action_link("Open", PAGE)) }
            }
        }
    }
}

/// `GET /settings/vault`.
pub(crate) async fn settings_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PAGE).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    render(&app, &headers, service, &viewer, None, PAGE)
}

/// `GET /projects/{id}/vault`: the same vault, showing the project's files.
pub(crate) async fn project_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let back = super::project_href(&id);
    let (service, viewer) = match crate::settings::viewer(&app, &headers, &back).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match service.github_status(&headers).await {
        Ok(status) => status,
        Err(_) => {
            return protect(crate::layout::problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "Vault",
                "Your projects couldn't be read right now. Try again in a minute.",
                (crate::projects::PAGE, "Projects"),
            ));
        }
    };
    let Some(project) = status.projects.into_iter().find(|p| p.id == id) else {
        return protect(crate::layout::problem(
            StatusCode::NOT_FOUND,
            "Vault",
            "That project isn't one of yours.",
            (crate::projects::PAGE, "Projects"),
        ));
    };
    render(
        &app,
        &headers,
        service,
        &viewer,
        Some((&project.id, &project.name)),
        &back,
    )
}

fn render(
    app: &App,
    headers: &HeaderMap,
    service: &crate::cloud::session::CloudSession,
    viewer: &crate::cloud::session::Viewer,
    project: Option<(&str, &str)>,
    path: &str,
) -> Response {
    let csrf = service
        .csrf(headers, viewer, CSRF_SCOPE, &viewer.account_id)
        .unwrap_or_default();
    let (script_sri, _) = digests(SOURCE.as_bytes());
    let built = build(app).map(|build| (digests(&build.glue).0, digests(&build.wasm).0));
    let head = html! {
        link rel="stylesheet" href=(STYLE);
        @if let Some((glue, _)) = &built {
            link rel="modulepreload" href=(GLUE_PATH) integrity=(glue);
        }
        script type="module" src=(SCRIPT) integrity=(script_sri) {}
    };
    let title = match project {
        Some((_, name)) => format!("Vault · {name}"),
        None => "Vault (only you)".to_owned(),
    };
    let body = content(
        &csrf,
        project,
        built
            .as_ref()
            .map(|(glue, wasm)| (glue.as_str(), wasm.as_str())),
        &viewer.account_label,
    );
    let account = crate::account::Account::SignedIn {
        name: viewer.account_label.clone(),
        sign_out: service.logout_csrf(headers, viewer).ok(),
        picture: viewer.avatar_url.is_some(),
        admin: viewer.admin,
    };
    let mut response = protect(
        crate::ui_page::UiPage::new(title)
            .path(path)
            .section(crate::settings::PAGE)
            .account(account)
            .scriptless()
            .head(head)
            .content(openagents_ui::content::PageColumn::new(body))
            .respond(headers),
    );
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response
}

fn small(label: &str, id: &str) -> Button {
    Button::new(label)
        .id(id)
        .size(ControlSize::Sm)
        .variant(ButtonVariant::Soft)
        .color(Color::Secondary)
}

fn primary(label: &str, id: &str) -> Button {
    Button::new(label).id(id)
}

/// The page's markup: every state is here, hidden until the script shows it.
pub(crate) fn content(
    csrf: &str,
    project: Option<(&str, &str)>,
    built: Option<(&str, &str)>,
    account: &str,
) -> Markup {
    let (project_id, project_name) = project.unwrap_or_default();
    html! {
        div #vault class="oa-vault"
            data-csrf=(csrf)
            data-csrf-header=(TOKEN_HEADER)
            data-project=(project_id)
            data-account=(account)
            data-wasm=(built.map(|(_, wasm)| wasm).unwrap_or_default())
            data-glue=(GLUE_PATH)
            data-release=(RELEASE)
        {
            h1 class="oa-heading" data-level="1" {
                @if project.is_some() { "Vault · " (project_name) } @else { "Vault (only you)" }
            }
            p class="oa-vault-lede" {
                "Files here are locked on your device before they upload. We store them locked and can't open them. "
                "Only your devices and your recovery code can."
            }
            p #vault-status class="oa-vault-status" role="status" aria-live="polite" { "Opening your vault…" }
            p #vault-error class="oa-vault-error" role="alert" hidden {}
            @if built.is_none() {
                p class="oa-vault-error" role="alert" { "The vault isn't available on this server." }
            }

            section #vault-unsupported hidden {
                p { "This browser can't run the vault. Use a current Chrome, Edge, Firefox or Safari." }
            }

            section #vault-new class="oa-vault-step" aria-labelledby="vault-new-title" hidden {
                h2 #vault-new-title { "Set up your vault" }
                p { "You'll get a 24-word recovery code, plus one more way to unlock: a passkey or your Nostr key." }
                p { strong { "If you lose all your devices and your recovery code, your files are gone. We can't get them back." } }
                div class="oa-vault-actions" {
                    (primary("Set up with a passkey", "vault-setup-passkey").attr("hidden", "hidden"))
                    (small("Set up with my Nostr key", "vault-setup-nostr").attr("hidden", "hidden"))
                }
                p #vault-setup-none hidden {
                    "This browser can't make a passkey that locks files. Set up your vault in Coder with "
                    code { "openagents vault setup" }
                    ", then unlock it here with your recovery code."
                }
            }

            section #vault-code class="oa-vault-step" aria-labelledby="vault-code-title" hidden {
                h2 #vault-code-title { "Write down your recovery code" }
                p { "These 24 words are the only way back in if you lose your devices. We never see them." }
                ol #vault-words class="oa-vault-words" {}
                p { "To check, type these words from your code:" }
                div class="oa-vault-check" {
                    label { span #vault-check-a-label { "Word" } input #vault-check-a type="text" autocomplete="off" spellcheck="false"; }
                    label { span #vault-check-b-label { "Word" } input #vault-check-b type="text" autocomplete="off" spellcheck="false"; }
                    label { span #vault-check-c-label { "Word" } input #vault-check-c type="text" autocomplete="off" spellcheck="false"; }
                }
                div class="oa-vault-actions" { (primary("Finish setup", "vault-finish")) }
            }

            section #vault-locked class="oa-vault-step" aria-labelledby="vault-locked-title" hidden {
                h2 #vault-locked-title { "Unlock your vault" }
                div class="oa-vault-actions" {
                    (primary("Unlock with a passkey", "vault-unlock-passkey").attr("hidden", "hidden"))
                    (small("Unlock with my Nostr key", "vault-unlock-nostr").attr("hidden", "hidden"))
                }
                p #vault-pairing-note hidden { "Unlocking with the link from your other device…" }
                details #vault-recovery {
                    summary { "Use your recovery code" }
                    (Textarea::new("vault-recovery-words").id("vault-recovery-words").rows(3).placeholder("24 words, in order"))
                    div class="oa-vault-actions" { (small("Unlock", "vault-unlock-recovery")) }
                }
            }

            section #vault-open aria-label="Your vault" hidden {
                div class="oa-vault-toolbar" {
                    label class="oa-vault-upload" {
                        span { "Add files" }
                        input #vault-file type="file" multiple;
                    }
                    span class="oa-vault-hint" { "Up to 10 MB each. Locked on this device before they upload." }
                    (small("Lock", "vault-lock"))
                }
                h2 { "Files" }
                p #vault-files-empty { "No files yet." }
                ul #vault-files class="oa-vault-list" role="list" {}

                section #vault-ask aria-labelledby="vault-ask-title" {
                    h2 #vault-ask-title { "Ask about your files" }
                    p { "Tick files above, then ask. They're opened on this device for this one answer." }
                    (Textarea::new("vault-question").id("vault-question").rows(3).placeholder("What do you want to know?"))
                    fieldset class="oa-vault-routes" {
                        legend { "Who reads the files" }
                        label #vault-route-device-row {
                            input #vault-route-device type="radio" name="vault-route" value="device";
                            span { strong { "On this device" } " · A model on this computer reads them. They never leave it." }
                        }
                        p #vault-route-device-off class="oa-vault-hint" hidden {
                            "To answer on this device, start Psionic on this computer: "
                            code { "openagents vault serve-local" }
                        }
                        p #vault-route-device-blocked class="oa-vault-hint" hidden {
                            "This browser is set not to let this site reach apps on this computer. Allow it in the site's settings (the icon left of the address), then reload."
                        }
                        label {
                            input #vault-route-fast type="radio" name="vault-route" value="fast";
                            span { strong { "Fast (Google sees it)" } " · Google Gemini reads them to answer. Google doesn't train on them, but sees them while answering." }
                        }
                    }
                    div class="oa-vault-actions" { (primary("Ask", "vault-ask-go")) }
                    div #vault-answer class="oa-vault-answer" hidden {
                        p #vault-answer-label class="oa-vault-route-label" {}
                        p #vault-answer-text class="oa-vault-answer-text" {}
                    }
                }

                h2 { "Answers" }
                p #vault-answers-empty { "Answers you get are kept here, locked like your files." }
                ul #vault-answers class="oa-vault-list" role="list" {}

                section aria-labelledby="vault-devices-title" {
                    h2 #vault-devices-title { "Ways to unlock" }
                    ul #vault-devices class="oa-vault-list" role="list" {}
                    div class="oa-vault-actions" {
                        (small("Add a passkey on this device", "vault-add-passkey").attr("hidden", "hidden"))
                        (small("Add my Nostr key", "vault-add-nostr").attr("hidden", "hidden"))
                        (small("Add another device", "vault-pair"))
                    }
                    div #vault-pair-box class="oa-vault-pair" hidden {
                        p { "On your other device, sign in and open this link, or scan the code. It works once, for 10 minutes." }
                        svg #vault-qr class="oa-vault-qr" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="QR code of the link" {}
                        input #vault-pair-link type="text" readonly aria-label="Link for your other device";
                        div class="oa-vault-actions" { (small("Copy link", "vault-pair-copy")) }
                    }
                }

                section aria-labelledby="vault-danger-title" {
                    h2 #vault-danger-title { "Delete" }
                    p { "Deleting your vault removes every file and key. Nobody can open the files after that, including you." }
                    (Button::new("Delete vault").id("vault-delete").size(ControlSize::Sm).variant(ButtonVariant::Outline).color(Color::Danger))
                }
            }

            template #vault-file-row {
                li class="oa-vault-row" {
                    label class="oa-vault-pick" {
                        input type="checkbox" data-pick;
                        span class="oa-vault-name" data-name {}
                    }
                    span class="oa-vault-meta" data-meta {}
                    span class="oa-vault-row-actions" {
                        (Button::new("Open").attr("data-act", "open").size(ControlSize::Sm).variant(ButtonVariant::Ghost).color(Color::Secondary))
                        (Button::new("Download").attr("data-act", "download").size(ControlSize::Sm).variant(ButtonVariant::Ghost).color(Color::Secondary))
                        (Button::new("Delete").attr("data-act", "delete").size(ControlSize::Sm).variant(ButtonVariant::Ghost).color(Color::Secondary))
                    }
                }
            }
            template #vault-device-row {
                li class="oa-vault-row" {
                    span class="oa-vault-name" data-name {}
                    span class="oa-vault-meta" data-meta {}
                    span class="oa-vault-row-actions" {
                        (Button::new("Remove").attr("data-act", "remove").size(ControlSize::Sm).variant(ButtonVariant::Ghost).color(Color::Secondary))
                    }
                }
            }
        }
    }
}
