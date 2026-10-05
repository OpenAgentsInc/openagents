//! Bounded device-loss observation shared by native render owners.
#[derive(Clone, Default)]
pub struct Health(std::sync::Arc<std::sync::Mutex<Option<String>>>);
impl Health {
    pub fn attach(device: &wgpu::Device) -> Self {
        let value = Self::default();
        let callback = value.clone();
        device.set_device_lost_callback(move |reason, message| {
            callback.fail(format!("{reason:?}: {message}"));
        });
        value
    }
    fn fail(&self, reason: String) {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if state.is_none() {
            *state = Some(reason.chars().take(512).collect());
        }
    }
    pub fn reason(&self, device: &wgpu::Device) -> Option<String> {
        if let Err(error) = device.poll(wgpu::PollType::Poll) {
            self.fail(error.to_string());
        }
        self.0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}
