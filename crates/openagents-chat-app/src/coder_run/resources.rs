//! Attachment previews use bounded owner reads and independent view identities.

use route_contract::{
    Digest, digest_of,
    observation::{Body, Observation},
    resources as wire,
};
use rust_native::{Axis, Element, Node, TextRole};
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq)]
pub struct Preview {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub pixels: Arc<Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq)]
pub enum Content {
    Image(Preview),
    Text { text: String, truncated: bool },
}
impl std::fmt::Debug for Content {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Image(image) => f
                .debug_tuple("Image")
                .field(&(image.width, image.height))
                .finish(),
            Self::Text { text, truncated } => f
                .debug_struct("Text")
                .field("bytes", &text.len())
                .field("truncated", truncated)
                .finish(),
        }
    }
}
impl Content {
    fn valid_for(&self, reference: &wire::Reference) -> bool {
        match self {
            Self::Image(image) => {
                reference.media_type.image()
                    && image.width > 0
                    && image.width <= 512
                    && image.height > 0
                    && image.height <= 320
                    && image.pixels.len() == image.width as usize * image.height as usize * 4
                    && !image.id.is_empty()
                    && image.id.len() <= 128
                    && !image.id.chars().any(char::is_control)
            }
            Self::Text { text, .. } => {
                !reference.media_type.image() && text.len() <= 64 * 1024 && !text.contains('\0')
            }
        }
    }
}
/// Run on the input worker. Encoded bytes and full-size decoded images are
/// released after producing a bounded preview.
pub fn decode(reference: &wire::Reference, bytes: Arc<Vec<u8>>) -> Result<Content, String> {
    if !reference.allows(wire::Usage::Preview)
        || bytes.len() != reference.size
        || Digest::of_bytes(&bytes) != reference.digest
    {
        return Err("This attachment changed. Refresh it and try again.".into());
    }
    let bytes = Arc::try_unwrap(bytes).unwrap_or_else(|bytes| (*bytes).clone());
    if reference.media_type.image() {
        let image = crate::attachments::Image::decode_preview(&reference.name, bytes, 512, 320)?;
        if image.mime != reference.media_type.as_str()
            || reference.dimensions
                != Some(wire::Dimensions {
                    width: image.width,
                    height: image.height,
                })
        {
            return Err("This image changed. Refresh it and try again.".into());
        }
        Ok(Content::Image(Preview {
            id: image.id,
            width: image.preview_width,
            height: image.preview_height,
            pixels: image.preview,
        }))
    } else {
        let mut text =
            String::from_utf8(bytes).map_err(|_| "This attachment isn't readable text.")?;
        let truncated = text.len() > 64 * 1024;
        if truncated {
            let mut end = 64 * 1024;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
        }
        Ok(Content::Text { text, truncated })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Control {
    Open(Digest),
    Close,
    Refresh,
    Previous,
    Next,
    Earlier,
    More,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    pub view: Digest,
    pub control: Control,
}
#[derive(Default)]
pub struct Inspector {
    task: String,
    references: Vec<wire::Reference>,
    scope: Option<(usize, u64)>,
    seq: u64,
    closed: bool,
    selected: Option<wire::Reference>,
    content: Option<Content>,
    expected: bool,
    generation: u64,
    page: usize,
    list: usize,
    error: Option<String>,
    incomplete: bool,
}
impl Inspector {
    pub fn new(task: &str) -> Self {
        Self {
            task: task.into(),
            ..Self::default()
        }
    }
    pub fn references(&self) -> &[wire::Reference] {
        &self.references
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn preview(&self) -> Option<&Preview> {
        match self.content.as_ref()? {
            Content::Image(image) => Some(image),
            _ => None,
        }
    }
    pub fn register(&mut self, reference: &wire::Reference) -> bool {
        if !reference.allows(wire::Usage::Preview)
            || reference.owner.host != super::LOCAL
            || reference.owner.task != self.task
            || self.references.contains(reference)
        {
            return false;
        }
        if self.references.len() >= 64 {
            self.references.remove(0);
            self.incomplete = true;
        }
        self.references.push(reference.clone());
        true
    }
    pub fn consume(&mut self, observation: &Observation) -> bool {
        if !observation.applies_to(super::LOCAL, &self.task, observation.scope.turn) {
            return false;
        }
        let scope = (observation.scope.turn, observation.scope.generation);
        if self.scope.is_some_and(|old| scope < old) {
            return false;
        }
        if self.scope != Some(scope) {
            self.scope = Some(scope);
            self.seq = 0;
            self.closed = false;
        }
        if observation.seq <= self.seq {
            return false;
        }
        self.seq = observation.seq;
        if self.closed {
            return false;
        }
        if observation.child.is_none() && matches!(observation.body, Body::Ended { .. }) {
            self.closed = true;
            return false;
        }
        if let Body::Resource { reference } = &observation.body {
            if matches!(&reference.origin,wire::Origin::Produced{scope,..} if scope!=&observation.scope)
            {
                return false;
            }
            return self.register(reference);
        }
        false
    }
    fn identity(&self) -> Digest {
        digest_of(&(
            &self.task,
            &self.selected,
            self.generation,
            self.page,
            self.list,
        ))
    }
    pub fn target(&self, control: Control) -> Target {
        Target {
            view: self.identity(),
            control,
        }
    }
    pub fn apply(&mut self, target: &Target) -> Option<wire::Reference> {
        if target.view != self.identity() {
            return None;
        }
        match &target.control {
            Control::Open(id) => {
                self.selected = Some(
                    self.references
                        .iter()
                        .find(|reference| reference.id == *id)?
                        .clone(),
                );
                self.generation += 1;
                self.content = None;
                self.page = 0;
                self.error = None;
                self.expected = true;
                self.selected.clone()
            }
            Control::Refresh if !self.expected => {
                self.generation += 1;
                self.expected = true;
                self.error = None;
                self.selected.clone()
            }
            Control::Close => {
                self.generation += 1;
                self.selected = None;
                self.content = None;
                self.expected = false;
                self.error = None;
                None
            }
            Control::Previous => {
                self.page = self.page.saturating_sub(1);
                None
            }
            Control::Next => {
                if (self.page + 1) * 12 < self.fragments().len() {
                    self.page += 1;
                }
                None
            }
            Control::Earlier => {
                self.list = self.list.saturating_sub(1);
                None
            }
            Control::More => {
                if (self.list + 1) * 4 < self.references.len() {
                    self.list += 1;
                }
                None
            }
            _ => None,
        }
    }
    pub fn open(&self) -> bool {
        self.selected.is_some()
    }
    pub fn received(
        &mut self,
        generation: u64,
        reference: &wire::Reference,
        result: Result<Content, String>,
    ) -> bool {
        if generation != self.generation
            || self.selected.as_ref() != Some(reference)
            || !self.expected
        {
            return false;
        }
        self.expected = false;
        self.page = 0;
        let result = result.and_then(|content| {
            if content.valid_for(reference) {
                Ok(content)
            } else {
                Err("This attachment couldn't be shown. Refresh it and try again.".into())
            }
        });
        match result {
            Ok(content) => {
                self.content = Some(content);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }
    fn fragments(&self) -> Vec<String> {
        match &self.content {
            Some(Content::Text { text, .. }) => text
                .lines()
                .flat_map(|line| {
                    let chars: Vec<_> = line.chars().collect();
                    if chars.is_empty() {
                        vec![String::new()]
                    } else {
                        chars
                            .chunks(140)
                            .map(|part| part.iter().collect())
                            .collect()
                    }
                })
                .collect(),
            _ => vec![],
        }
    }
    pub fn list_view<I>(&self, intent: impl FnMut(&str, Target) -> I) -> Option<Node<I>> {
        self.view(false, intent)
    }
    pub fn preview_view<I>(&self, intent: impl FnMut(&str, Target) -> I) -> Option<Node<I>> {
        if self.selected.is_none() {
            None
        } else {
            self.view(true, intent)
        }
    }
    fn view<I>(
        &self,
        preview_only: bool,
        mut intent: impl FnMut(&str, Target) -> I,
    ) -> Option<Node<I>> {
        if self.references.is_empty() {
            return None;
        }
        let text = |key: &str, value: String| Node {
            key: format!("coder-resources-{key}"),
            style: Default::default(),
            element: Element::Text {
                value,
                role: TextRole::Body,
            },
        };
        let mut button = |key: &str, label: String, control, enabled| {
            let key = format!("coder-resources-{key}");
            let action = intent(&key, self.target(control));
            Node {
                key,
                style: Default::default(),
                element: Element::Button {
                    label,
                    enabled,
                    intent: action,
                    shortcut: None,
                    icon: None,
                },
            }
        };
        let mut children = vec![text("title", "Attachments".into())];
        if !preview_only {
            for reference in self.references.iter().rev().skip(self.list * 4).take(4) {
                children.push(button(
                    reference.id.as_str(),
                    format!("Open {}", reference.name),
                    Control::Open(reference.id.clone()),
                    true,
                ));
            }
            if self.references.len() > 4 {
                children.push(button(
                    "earlier",
                    "Previous attachments".into(),
                    Control::Earlier,
                    self.list > 0,
                ));
                children.push(button(
                    "more",
                    "More attachments".into(),
                    Control::More,
                    (self.list + 1) * 4 < self.references.len(),
                ));
            }
            if self.incomplete {
                children.push(text(
                    "limited",
                    "Some older attachments are not shown.".into(),
                ));
            }
        }
        if preview_only && let Some(reference) = &self.selected {
            children.push(text(
                "name",
                format!(
                    "{} · {} · {} bytes",
                    reference.name,
                    reference.media_type.as_str(),
                    reference.size
                ),
            ));
            children.push(button(
                "refresh",
                "Refresh attachment".into(),
                Control::Refresh,
                !self.expected,
            ));
            children.push(button(
                "close",
                "Close attachment".into(),
                Control::Close,
                true,
            ));
            if self.expected {
                children.push(text("loading", "Loading attachment…".into()));
            }
            if let Some(error) = &self.error {
                children.push(text("error", error.clone()));
            }
            match &self.content {
                Some(Content::Image(image)) => children.push(Node {
                    key: "coder-resource-image".into(),
                    style: Default::default(),
                    element: Element::Surface {
                        label: reference.name.clone(),
                        resource: format!("image:{}", image.id),
                    },
                }),
                Some(Content::Text {
                    text: content,
                    truncated,
                }) => {
                    let fragments = self.fragments();
                    children.push(text(
                        "text",
                        if content.is_empty() {
                            "This attachment is empty.".into()
                        } else {
                            fragments
                                .iter()
                                .skip(self.page * 12)
                                .take(12)
                                .cloned()
                                .collect::<Vec<_>>()
                                .join("\n")
                        },
                    ));
                    if *truncated {
                        children.push(text("partial", "Only the beginning is shown.".into()));
                    }
                    children.push(button(
                        "previous",
                        "Previous page".into(),
                        Control::Previous,
                        self.page > 0,
                    ));
                    children.push(button(
                        "next",
                        "Next page".into(),
                        Control::Next,
                        (self.page + 1) * 12 < fragments.len(),
                    ));
                }
                _ => {}
            }
        }
        Some(Node {
            key: "coder-resources".into(),
            style: Default::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reference() -> wire::Reference {
        let mut reference = wire::Reference {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            owner: wire::Owner {
                host: "local".into(),
                task: "task".into(),
            },
            origin: wire::Origin::Produced {
                scope: route_contract::observation::Scope {
                    host: "local".into(),
                    task: "task".into(),
                    turn: 1,
                    generation: 1,
                },
                item: "result".into(),
            },
            digest: Digest::of_bytes(b"Preview"),
            media_type: wire::MediaType::Text,
            size: 7,
            name: "preview.txt".into(),
            dimensions: None,
            uses: vec![wire::Usage::Preview],
        };
        reference.seal();
        reference
    }
    #[test]
    fn closing_and_reopening_a_preview_rejects_old_success_and_error_results() {
        let reference = reference();
        let mut inspector = Inspector::new("task");
        assert!(inspector.register(&reference));
        inspector.apply(&inspector.target(Control::Open(reference.id.clone())));
        let old = inspector.generation();
        inspector.apply(&inspector.target(Control::Close));
        inspector.apply(&inspector.target(Control::Open(reference.id.clone())));
        assert!(!inspector.received(old, &reference, Err("late".into())));
        assert!(inspector.received(
            inspector.generation(),
            &reference,
            Ok(Content::Text {
                text: "Preview".into(),
                truncated: false
            })
        ));
        let mut labels = vec![];
        let node = inspector.preview_view(|_, _| ()).unwrap();
        fn check(node: &Node<()>, labels: &mut Vec<String>) {
            match &node.element {
                Element::Text { value, .. } => labels.push(value.clone()),
                Element::Button { label, .. } => labels.push(label.clone()),
                Element::Stack { children, .. } => {
                    for child in children {
                        check(child, labels)
                    }
                }
                _ => {}
            }
        }
        check(&node, &mut labels);
        for label in labels {
            assert!(oa_copy::violations(&label, &[]).is_empty(), "{label}");
        }
    }
    #[test]
    fn preview_decoding_checks_the_digest_dimensions_and_pixel_budget() {
        let bytes = Arc::new(
            include_bytes!("../../../coder-sdk/fixtures/resources/one-pixel.png").to_vec(),
        );
        let mut reference = reference();
        reference.media_type = wire::MediaType::Png;
        reference.digest = Digest::of_bytes(&bytes);
        reference.size = bytes.len();
        reference.dimensions = Some(wire::Dimensions {
            width: 1,
            height: 1,
        });
        reference.seal();
        let Content::Image(image) = decode(&reference, bytes.clone()).unwrap() else {
            panic!("image")
        };
        assert_eq!(image.pixels.len(), 4);
        reference.dimensions = Some(wire::Dimensions {
            width: 2,
            height: 1,
        });
        reference.seal();
        assert!(decode(&reference, bytes).is_err());
    }
}
