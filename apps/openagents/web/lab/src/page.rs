use gpui::Context;
use lab_kit::Card;
use openagents_chat_view::settings_forms::lab::{Action, State, view};
use openagents_web_page_host::{Host, Intent, Load, Page, Paints, plain};
use rust_native::Node;
use serde_json::Value;
use wasm_bindgen::JsCast;

/// The player's height over the canvas.
const PLAYER: f32 = 440.;
/// The host's header, which the player sits under.
const HEADER: f32 = 52.;

pub struct Lab {
    state: State,
    /// `KIND/DOCUMENT/VERSION/SPACE` of the open document.
    path: String,
}

type Cx<'a> = Context<'a, Host<Lab>>;

fn encode(text: &str) -> String {
    String::from(js_sys::encode_uri_component(text))
}

/// The address of `kind`/`document` at `version` (0 for the latest) in
/// `space`, after `labs/`.
fn path_of(kind: &str, document: &str, version: u64, space: &str) -> String {
    format!("{kind}/{document}/{version}/{space}")
}

impl Page for Lab {
    const NAME: &'static str = "labs";

    fn new() -> Self {
        Self {
            state: State::default(),
            path: String::new(),
        }
    }

    fn title(&self) -> String {
        self.state
            .card
            .ready()
            .map_or_else(|| "Lab".into(), |card| card.title.clone())
    }

    fn view(&self, paints: &Paints) -> Node<Intent> {
        view(&self.state, paints)
    }

    fn pinned(&self) -> Option<f32> {
        self.state.player().then_some(PLAYER)
    }

    fn open(host: &mut Host<Self>, path: &str, cx: &mut Cx) {
        let mut parts = path.splitn(4, '/');
        let (Some(kind), Some(document)) = (parts.next(), parts.next()) else {
            host.page.state.card = Load::Failed("That document isn't here.".into());
            return;
        };
        let version = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let space = parts.next().filter(|s| !s.is_empty()).unwrap_or("me");
        load(
            host,
            kind.into(),
            document.into(),
            version,
            space.into(),
            cx,
        );
    }

    fn intent(host: &mut Host<Self>, intent: Intent, cx: &mut Cx) {
        let Intent::Lab(action) = intent else {
            return;
        };
        match action {
            Action::Open {
                space,
                kind,
                document,
                version,
            } => load(host, kind, document, version, space, cx),
            Action::Download(url) | Action::Visit(url) => host.go(&url),
        }
    }
}

fn load(
    host: &mut Host<Lab>,
    kind: String,
    document: String,
    version: u64,
    space: String,
    cx: &mut Cx,
) {
    let path = path_of(&kind, &document, version, &space);
    if host.page.path == path && matches!(host.page.state.card, Load::Ready(_)) {
        return;
    }
    host.page.path = path.clone();
    host.page.state = State {
        space: space.clone(),
        card: Load::Loading,
    };
    player(None);
    host.moved(&path);
    let client = host.client.clone();
    cx.spawn(async move |this, cx| {
        let base = format!("/v1/labs/documents/{}", encode(&document));
        let space_query = format!("space={}", encode(&space));
        let version = if version > 0 {
            Ok(version)
        } else {
            client
                .get::<Value>(&format!("{base}?{space_query}"))
                .await
                .map(|d| d["latest"].as_u64().unwrap_or(1))
        };
        let card = match version {
            Ok(version) => client
                .get::<Card>(&format!("{base}/versions/{version}/card?{space_query}"))
                .await
                .map(|card| (card, version)),
            Err(error) => Err(error),
        };
        this.update(cx, |host, cx| {
            if host.page.path != path {
                return;
            }
            match card {
                Ok((card, version)) if card.kind == kind => {
                    host.page.state.card = Load::Ready(card);
                    if host.page.state.player() {
                        player(Some(&format!(
                            "/labs/{}/{}/media?v={version}&space={}",
                            encode(&kind),
                            encode(&document),
                            encode(&space)
                        )));
                    }
                }
                Ok(_) => host.page.state.card = Load::Failed("That document isn't here.".into()),
                Err(error) => host.page.state.card = Load::Failed(plain(&error)),
            }
            host.changed(cx);
        })
        .ok();
    })
    .detach();
}

/// Mount the document's player at `src` over the canvas, under the
/// header, or take it away (`None`).
fn player(src: Option<&str>) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(old) = document.get_element_by_id("lab-player") {
        old.remove();
    }
    let (Some(src), Some(page_box)) = (src, document.get_element_by_id("gpui-page-labs")) else {
        return;
    };
    let Some(frame) = document
        .create_element("iframe")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlIFrameElement>().ok())
    else {
        return;
    };
    frame.set_id("lab-player");
    frame.set_src(src);
    frame.set_title("Player");
    let _ = frame.set_attribute(
        "style",
        &format!(
            "position:absolute;top:{HEADER}px;left:0;right:0;height:{PLAYER}px;width:100%;border:0;background:transparent;"
        ),
    );
    let _ = frame.set_attribute("allow", "fullscreen");
    let _ = page_box.append_child(&frame);
}
