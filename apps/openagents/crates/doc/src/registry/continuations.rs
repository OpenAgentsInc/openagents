//! Additive continuation references; no transcript text enters the registry.
use super::*;
impl RegistryDoc {
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
