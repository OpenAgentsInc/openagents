//! Offline SDK fixtures for local Android transcript and navigation captures.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidDemoChat {
    Picker,
    Visuals,
    Cjk,
    Veil,
}
impl AndroidDemoChat {
    const ALL: [Self; 4] = [Self::Picker, Self::Visuals, Self::Cjk, Self::Veil];
    fn id(self) -> &'static str {
        match self {
            Self::Picker => "chat-picker",
            Self::Visuals => "chat-visuals",
            Self::Cjk => "chat-cjk",
            Self::Veil => "chat-veil",
        }
    }
}

#[derive(uniffi::Record)]
pub struct AndroidDemoChoice {
    pub fixture: AndroidDemoChat,
    pub row: SessionRow,
}

/// Retains one synthetic SDK workspace. The shell closes its views before shutdown.
#[derive(uniffi::Object)]
pub struct CoreAndroidDemo {
    owner: Arc<CoreAndroidHost>,
    client: Arc<CoreClient>,
    closed: AtomicBool,
}

#[uniffi::export]
impl CoreAndroidDemo {
    /// `data_dir` must be a new absolute child of the caller's scratch directory.
    /// Existing directories are refused, including empty ones; account storage is never opened.
    #[uniffi::constructor]
    pub fn new(data_dir: String, listener: Arc<dyn AndroidHostListener>) -> CoreResult<Arc<Self>> {
        let path = std::path::Path::new(&data_dir);
        if !path.is_absolute() {
            return Err(CoreError::InvalidArgument {
                message: "Choose a new folder for the demo.".into(),
            });
        }
        std::fs::create_dir(path).map_err(|_| CoreError::Storage {
            message: "Couldn't create a new demo folder.".into(),
        })?;
        let owner = CoreAndroidHost::demo(listener);
        // Callers cannot supply credentials, deployment endpoints, or arbitrary demo datasets.
        let client = CoreClient::new(
            CoreConfig {
                edge_url: "https://android-demo.invalid".into(),
                data_dir,
                device_id: "android-demo".into(),
                device_name: "Demo phone".into(),
                platform: "android".into(),
                app_version: "demo".into(),
            },
            Credentials::Demo {
                options: DemoOptions {
                    fixture: DemoFixture::Standard,
                    transcript_scale: TranscriptScale::Normal,
                    stream_speed: StreamSpeed::Realistic,
                    long_reply: false,
                },
            },
            owner.events.clone(),
        )?;
        let demo = Arc::new(Self {
            owner,
            client,
            closed: AtomicBool::new(false),
        });
        demo.select(AndroidDemoChat::Veil)?;
        Ok(demo)
    }

    /// Borrow the existing host, composer, and transcript attachment path.
    pub fn host(&self) -> CoreResult<Arc<CoreAndroidHost>> {
        self.ensure_open()?;
        Ok(self.owner.clone())
    }

    /// Labels, ordering, and metadata come from the same Rust fixture workspace.
    pub fn choices(&self) -> CoreResult<Vec<AndroidDemoChoice>> {
        self.ensure_open()?;
        AndroidDemoChat::ALL
            .into_iter()
            .map(|fixture| {
                let row = self
                    .client
                    .session_row(fixture.id().into())
                    .ok_or(CoreError::Closed)?;
                Ok(AndroidDemoChoice { fixture, row })
            })
            .collect()
    }

    /// Only Rust's finite fixture identities can select a session.
    pub fn select(&self, fixture: AndroidDemoChat) -> CoreResult<()> {
        let mut state = self.owner.state.lock().map_err(|_| CoreError::Closed)?;
        self.ensure_open()?;
        let session = self.client.open_session(fixture.id().into())?;
        state.client = Some(self.client.clone());
        let composer = CoreAndroidComposer::live(session.clone());
        self.owner.replace(
            &mut state,
            Content::Live {
                id: fixture.id().into(),
                client: self.client.clone(),
                session,
            },
            Some(composer),
        )?;
        drop(state);
        self.owner.events.0.on_android_host_changed();
        Ok(())
    }

    /// Retire views first. Synthetic files remain in scratch for the capture receipt.
    pub fn shutdown(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Ok(mut state) = self.owner.state.lock() {
            state.foreground = false;
            state.retire_content();
            if let Some(composer) = state.composer.take() {
                composer.dispose();
            }
            state.client = None;
        }
        self.client.shutdown();
    }
}
impl CoreAndroidDemo {
    fn ensure_open(&self) -> CoreResult<()> {
        if self.closed.load(Ordering::Acquire) || !self.client.is_demo() {
            Err(CoreError::Closed)
        } else {
            Ok(())
        }
    }
}
impl Drop for CoreAndroidDemo {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Quiet;
    impl AndroidHostListener for Quiet {
        fn on_android_host_changed(&self) {}
    }

    #[test]
    fn only_offline_fixture_rows_select_and_retired_content_cannot_save_an_anchor() {
        let scratch = tempfile::tempdir().unwrap();
        let demo = CoreAndroidDemo::new(
            scratch.path().join("demo-run").display().to_string(),
            Arc::new(Quiet),
        )
        .unwrap();
        assert!(demo.client.is_demo());
        let owner = demo.host().unwrap();
        assert!(owner.account().is_none());
        let choices = demo.choices().unwrap();
        assert_eq!(choices.len(), 4);
        for choice in choices {
            let before = owner.snapshot().unwrap();
            assert_eq!(choice.row.id, choice.fixture.id());
            demo.select(choice.fixture).unwrap();
            let after = owner.snapshot().unwrap();
            assert_eq!(after.selected_chat.as_deref(), Some(choice.fixture.id()));
            assert_eq!(after.source, AndroidHostContent::Live);
            assert!(after.content_revision > before.content_revision);
            assert!(after.account.is_none() && after.sessions.is_none() && after.history.is_none());
            assert!(!after.can_start_voice);
            assert!(matches!(
                owner.save_transcript_anchor(
                    before.generation,
                    before.content_revision,
                    AndroidTranscriptAnchor {
                        row: 1,
                        inside: 0.0,
                        following: false
                    }
                ),
                Err(CoreError::Closed)
            ));
        }
        assert!(owner.navigate(AndroidHostRoute::Wallet).is_err());
        assert!(owner.navigate(AndroidHostRoute::Recovery).is_err());
        demo.shutdown();
        demo.shutdown();
        assert!(matches!(demo.host(), Err(CoreError::Closed)));
        assert!(matches!(
            demo.select(AndroidDemoChat::Veil),
            Err(CoreError::Closed)
        ));
        assert!(matches!(demo.choices(), Err(CoreError::Closed)));
        assert!(owner.snapshot().unwrap().composer.is_none());
    }

    #[test]
    fn an_existing_account_folder_is_never_opened_or_changed() {
        let scratch = tempfile::tempdir().unwrap();
        let account = scratch.path().join("core");
        std::fs::create_dir(&account).unwrap();
        std::fs::write(account.join("identity"), b"synthetic-existing-account").unwrap();
        assert!(CoreAndroidDemo::new(account.display().to_string(), Arc::new(Quiet)).is_err());
        assert_eq!(
            std::fs::read(account.join("identity")).unwrap(),
            b"synthetic-existing-account"
        );
        assert!(
            CoreAndroidDemo::new(scratch.path().display().to_string(), Arc::new(Quiet)).is_err()
        );
        assert!(CoreAndroidDemo::new("relative-demo".into(), Arc::new(Quiet)).is_err());
    }

    #[test]
    fn concurrent_shutdown_leaves_no_selected_composer_or_future_admission() {
        let scratch = tempfile::tempdir().unwrap();
        let demo = CoreAndroidDemo::new(
            scratch.path().join("demo-run").display().to_string(),
            Arc::new(Quiet),
        )
        .unwrap();
        let owner = demo.host().unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let selecting = demo.clone();
        let ready = barrier.clone();
        let task = std::thread::spawn(move || {
            ready.wait();
            selecting.select(AndroidDemoChat::Picker)
        });
        barrier.wait();
        demo.shutdown();
        // Selection may win admission before shutdown; shutdown must retire its result too.
        let _ = task.join().unwrap();
        assert!(demo.select(AndroidDemoChat::Cjk).is_err());
        let state = owner.state.lock().unwrap();
        assert!(!state.foreground);
        assert!(state.composer.is_none() && state.client.is_none());
    }
}
