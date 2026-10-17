//! Additive continuation references; no transcript text enters the registry.
use super::*;
impl RegistryDoc {
    pub fn chat_move_targets(&self) -> HashMap<String, chat_home::PartRef> {
        #[derive(Deserialize)]
        struct Entry {
            id: String,
            #[serde(default)]
            move_target: Option<chat_home::PartRef>,
        }
        self.read_kind::<Entry>(KIND_CHATS)
            .into_iter()
            .filter_map(|entry| {
                entry
                    .move_target
                    .filter(|target| target.valid() && target.id != entry.id)
                    .map(|target| (entry.id, target))
            })
            .collect()
    }

    pub fn set_chat_move_target(
        &mut self,
        chat: &str,
        target: &chat_home::PartRef,
    ) -> Result<bool, DocError> {
        if !target.valid() || target.id == chat {
            return Err(DocError::Schema("Invalid continuation destination".into()));
        }
        if self.chat(chat)?.is_none() {
            return Ok(false);
        }
        self.write(
            KIND_CHATS,
            chat,
            OpKind::Update,
            fields([("move_target", serde_json::to_value(target)?)]),
        );
        Ok(true)
    }
    pub fn set_chat_destination(
        &mut self,
        chat: &str,
        target: &chat_home::PartRef,
    ) -> Result<bool, DocError> {
        if !target.valid() || target.id == chat {
            return Err(DocError::Schema("Invalid continuation destination".into()));
        }
        if self.chat(chat)?.is_none() {
            return Ok(false);
        }
        self.write(
            KIND_CHATS,
            chat,
            OpKind::Update,
            fields([
                ("continued_by", serde_json::to_value(target)?),
                ("move_target", serde_json::Value::Null),
            ]),
        );
        Ok(true)
    }
    /// References only: the session index never reads transcript text.
    pub fn chat_continuations(&self) -> HashMap<String, chat_home::PartRef> {
        #[derive(Deserialize)]
        struct Entry {
            id: String,
            #[serde(default)]
            continues: Option<chat_home::PartRef>,
        }
        self.read_kind::<Entry>(KIND_CHATS)
            .into_iter()
            .filter_map(|entry| {
                entry
                    .continues
                    .filter(|source| source.valid() && source.id != entry.id)
                    .map(|source| (entry.id, source))
            })
            .collect()
    }

    pub fn set_chat_continuation(
        &mut self,
        chat: &str,
        source: &chat_home::PartRef,
    ) -> Result<bool, DocError> {
        if self.chat(chat)?.is_none() {
            return Ok(false);
        }
        self.write(
            KIND_CHATS,
            chat,
            OpKind::Update,
            fields([("continues", serde_json::to_value(source)?)]),
        );
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuation_reference_survives_a_normal_chat_update_without_copying_text() {
        let id = "a1000000-0000-4000-8000-000000000001";
        let mut chat: Chat = serde_json::from_value(json!({
            "id": id, "deviceId": "fixture", "title": "First title", "archived": false,
            "cwd": null, "branch": null, "checkoutId": null, "config": null,
            "lastMessagePreview": "private transcript text", "lastMessageAt": null,
            "createdAt": "2026-10-17T00:00:00Z"
        }))
        .unwrap();
        let source = chat_home::PartRef {
            id: "a1000000-0000-4000-8000-000000000002".into(),
            home: chat_home::Home::Cloud,
        };
        let mut doc = RegistryDoc::new("fixture");
        doc.upsert_chat(&chat).unwrap();
        doc.set_chat_continuation(id, &source).unwrap();
        doc.set_chat_move_target(id, &source).unwrap();
        chat.title = Some("New title".into());
        doc.upsert_chat(&chat).unwrap();
        assert_eq!(doc.chat_move_targets().get(id), Some(&source));
        doc.set_chat_destination(id, &source).unwrap();
        assert!(doc.chat_move_targets().is_empty());
        let links = doc.chat_continuations();
        assert_eq!(links.get(id), Some(&source));
        assert_eq!(doc.chat(id).unwrap().unwrap().links.continues, Some(source));
        assert!(
            !serde_json::to_string(&links)
                .unwrap()
                .contains("private transcript text")
        );
    }
}
