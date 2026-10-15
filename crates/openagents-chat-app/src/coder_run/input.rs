//! A queued message keeps its attachments and delivery identity together.

use crate::attachments::{Image, Upload};
use route_contract::{resources as wire, session::control};

#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub text: String,
    pub images: Vec<Upload>,
    pub drafts: Vec<Image>,
}
impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("text", &self.text)
            .field(
                "images",
                &self
                    .images
                    .iter()
                    .map(|image| &image.reference)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}
impl From<String> for Message {
    fn from(text: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            text,
            images: vec![],
            drafts: vec![],
        }
    }
}
impl From<&str> for Message {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}
impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)?;
        if !self.images.is_empty() {
            write!(
                f,
                " · {} image{}",
                self.images.len(),
                if self.images.len() == 1 { "" } else { "s" }
            )?;
        }
        Ok(())
    }
}
impl Message {
    pub fn with_images(
        text: &str,
        images: Vec<Upload>,
        drafts: Vec<Image>,
    ) -> Result<Self, String> {
        if images.len() > wire::MAX_INPUTS
            || images.iter().any(|image| {
                image.reference.validate().is_err()
                    || image.bytes.len() != image.reference.size as usize
                    || image.bytes.len() > wire::MAX_BYTES
                    || coder_host::access::media::digest(&image.bytes) != image.reference.digest
                    || coder_host::access::media::sniff(&image.bytes)
                        != Some(image.reference.media_type.as_str())
            })
            || images.iter().map(|image| image.bytes.len()).sum::<usize>() > wire::MAX_INPUT_BYTES
        {
            return Err("Attach up to four PNG or JPEG images, totaling 16 MiB or less.".into());
        }
        if !drafts.is_empty()
            && (drafts.len() != images.len()
                || drafts
                    .iter()
                    .zip(&images)
                    .any(|(draft, image)| *draft.bytes != *image.bytes))
        {
            return Err("These images changed. Add them again.".into());
        }
        let mut message = Self::from(text);
        message.images = images;
        message.drafts = drafts;
        Ok(message)
    }
    pub fn image_ids(&self) -> Vec<String> {
        self.drafts.iter().map(|image| image.id.clone()).collect()
    }
    pub fn acknowledged(
        &self,
        task: &str,
        configuration: Option<&control::Request>,
        input: &wire::Input,
        receipt: &wire::Receipt,
    ) -> bool {
        input.valid_for(task, &self.text)
            && input.message == self.id
            && input.owner.host == super::LOCAL
            && input.choice == configuration.map(route_contract::digest_of)
            && receipt.valid_for(input)
            && input.resources.len() == self.images.len()
            && input
                .resources
                .iter()
                .zip(&self.images)
                .all(|(reference, image)| {
                    reference.digest.as_str() == image.reference.digest
                        && reference.size as u64 == image.reference.size
                        && reference.media_type.as_str() == image.reference.media_type
                        && reference.name == wire::name(&image.reference.name)
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coder_run::{Action, Answer, Request, Run, State};
    use std::time::Instant;
    fn image() -> Image {
        Image::decode(
            "fixture.png",
            include_bytes!("../../../coder-sdk/fixtures/resources/one-pixel.png").to_vec(),
        )
        .unwrap()
    }
    fn run() -> Run {
        let now = Instant::now();
        let mut run = Run::follow("chat", "task", None, now);
        let (ticket, _) = run.tick(now).unwrap();
        run.outcome(
            ticket,
            Ok(Answer::Lines {
                configuration: None,
                native: None,
                children: None,
                lines: vec![],
                state: State::Running,
            }),
            now,
        );
        let mut session = route_contract::session::Descriptor::default();
        session.schema = route_contract::session::SCHEMA.into();
        session.owner = "local".into();
        session.task = "task".into();
        session.turn = 1;
        session.controls.queue = true;
        session.controls.continue_task = true;
        session.resolved.native.image_followups = true;
        session.resolved.adapter = "claude-sdk".into();
        session.resolved.selection.provider = "claude".into();
        session.resolved.selection.model = "fixture".into();
        run.session = Some(session);
        run
    }
    #[test]
    fn queue_and_retry_keep_one_message_identity_and_exact_images() {
        let mut run = run();
        let image = image();
        let uploads = vec![Upload::new(&image.name, image.bytes.clone()).unwrap()];
        assert!(
            run.action_with_images(
                Action::Queue,
                "Inspect next",
                uploads.clone(),
                vec![image.clone()]
            )
            .is_none()
        );
        assert!(run.take_consumed_input());
        let id = run.queue.front().unwrap().id.clone();
        assert_eq!(run.queue.front().unwrap().images, uploads);
        run.state = State::Ended;
        let (ticket, request) = run.tick(Instant::now()).unwrap();
        let Request::ContinueInput { message, .. } = &request else {
            panic!("image message")
        };
        assert_eq!(message.id, id);
        assert_eq!(message.images, uploads);
        run.outcome(ticket, Err("The owner is offline.".into()), Instant::now());
        let (_, retry) = run.action(Action::Retry, "").unwrap();
        assert_eq!(retry, request);
        assert!(run.take_delivered_ids().is_empty());
    }
    #[test]
    fn unsupported_images_preserve_the_message() {
        let mut run = run();
        run.session
            .as_mut()
            .unwrap()
            .resolved
            .native
            .image_followups = false;
        let image = image();
        let uploads = vec![Upload::new(&image.name, image.bytes.clone()).unwrap()];
        assert!(
            run.action_with_images(Action::Queue, "Keep", uploads, vec![image])
                .is_none()
        );
        assert!(!run.take_consumed_input());
        assert_eq!(run.queued(), 0);
        assert!(run.notice.as_deref().unwrap().contains("Claude Code"));
    }
    #[test]
    fn a_failed_queued_photo_can_be_edited_without_losing_its_bytes() {
        let mut run = run();
        let image = image();
        let upload = Upload::new(&image.name, image.bytes.clone()).unwrap();
        run.action_with_images(
            Action::Queue,
            "Inspect next",
            vec![upload],
            vec![image.clone()],
        );
        run.state = State::Ended;
        let (ticket, _) = run.tick(Instant::now()).unwrap();
        run.outcome(
            ticket,
            Err("This coding engine doesn't accept images.".into()),
            Instant::now(),
        );
        assert!(run.action(Action::Send, "Another message").is_none());
        assert!(matches!(run.failed, Some(Request::ContinueInput { .. })));
        assert!(run.error.is_some());
        assert!(run.action(Action::EditFailedInput, "").is_none());
        let restored = run.take_restored_input().unwrap();
        assert_eq!(restored.text, "Inspect next");
        assert_eq!(restored.drafts, vec![image.clone()]);
        assert_eq!(*restored.images[0].bytes, *image.bytes);
        assert!(run.failed.is_none());
        assert!(run.take_delivered_ids().is_empty());
    }
    #[test]
    fn an_acknowledgment_for_another_message_cannot_discard_a_photo() {
        use route_contract::{Digest, digest_of};
        let mut run = run();
        let image = image();
        run.action_with_images(
            Action::Queue,
            "Inspect",
            vec![Upload::new(&image.name, image.bytes.clone()).unwrap()],
            vec![image.clone()],
        );
        run.state = State::Ended;
        let (ticket, request) = run.tick(Instant::now()).unwrap();
        let Request::ContinueInput { message, .. } = &request else {
            panic!("photo message")
        };
        let mut reference = wire::Reference {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            owner: wire::Owner {
                host: "local".into(),
                task: "task".into(),
            },
            origin: wire::Origin::Input {
                message: "another-message".into(),
            },
            digest: Digest::of_bytes(&image.bytes),
            media_type: wire::MediaType::Png,
            size: image.bytes.len(),
            name: wire::name(&image.name),
            dimensions: Some(wire::Dimensions {
                width: 1,
                height: 1,
            }),
            uses: vec![wire::Usage::PromptImage, wire::Usage::Preview],
        };
        reference.seal();
        let mut input = wire::Input {
            schema: wire::SCHEMA.into(),
            id: digest_of(&()),
            message: "another-message".into(),
            turn: 2,
            owner: reference.owner.clone(),
            text: Digest::of_bytes(message.text.as_bytes()),
            choice: None,
            resources: vec![reference],
        };
        input.seal();
        let receipt = wire::Receipt {
            schema: wire::SCHEMA.into(),
            input: input.id.clone(),
            task: "task".into(),
            turn: 2,
            revision: 3,
        };
        assert!(!run.outcome(
            ticket,
            Ok(Answer::InputAccepted { input, receipt }),
            Instant::now()
        ));
        assert_eq!(run.failed.as_ref(), Some(&request));
        assert!(run.error.is_some());
        assert!(run.take_delivered_ids().is_empty());
        assert_eq!(run.held_image_sizes(), vec![(image.id, image.bytes.len())]);
        let (_, retry) = run.action(Action::Retry, "").unwrap();
        assert_eq!(retry, request);
    }
}
