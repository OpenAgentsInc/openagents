//! Android process initialization required before a network client can start.

#[cfg(any(target_os = "android", test))]
use std::sync::Mutex;

#[cfg(target_os = "android")]
use jni::{
    EnvUnowned,
    errors::ThrowRuntimeExAndDefault,
    jni_mangle,
    objects::{Global, JClass, JObject},
    refs::Reference,
};

use crate::client_ffi::CoreError;

#[cfg(target_os = "android")]
static ANDROID_CONTEXT: InitializationState<Global<JObject<'static>>> = InitializationState {
    value: Mutex::new(None),
};

#[cfg(any(target_os = "android", test))]
#[derive(Default)]
struct InitializationState<T> {
    value: Mutex<Option<T>>,
}

#[cfg(any(target_os = "android", test))]
impl<T> InitializationState<T> {
    fn initialize<E>(&self, initialize: impl FnOnce() -> Result<T, E>) -> Result<(), E> {
        let mut value = self
            .value
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if value.is_some() {
            return Ok(());
        }
        *value = Some(initialize()?);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.value
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_some()
    }
}

/// Refuses to construct a live client until Android has installed its JVM context and verifier.
pub(crate) fn ensure_initialized() -> Result<(), CoreError> {
    #[cfg(target_os = "android")]
    if !ANDROID_CONTEXT.is_initialized() {
        return Err(initialization_error());
    }

    Ok(())
}

#[cfg(target_os = "android")]
fn initialization_error() -> CoreError {
    CoreError::Internal {
        message: "App startup did not finish. Restart the app and try again.".to_owned(),
    }
}

#[cfg(target_os = "android")]
#[jni_mangle("com.openagents.app.AndroidCoreBootstrap")]
pub fn native_initialize<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
) {
    unowned_env
        .with_env(|env| {
            if context.is_null() {
                return Err(jni::errors::Error::NullPtr("application context"));
            }

            ANDROID_CONTEXT.initialize(|| {
                let java_vm = env.get_java_vm()?;
                let application_context = env.new_global_ref(&context)?;
                rustls_platform_verifier::android::init_with_env(env, context)?;

                // iroh-dns and ndk-context use this pointer until the process exits.
                // Keep the global reference alive in ANDROID_CONTEXT.
                unsafe {
                    ndk_context::initialize_android_context(
                        java_vm.get_raw().cast(),
                        application_context.as_raw().cast(),
                    );
                }
                Ok(application_context)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

#[cfg(test)]
mod tests {
    use super::InitializationState;
    use std::sync::Mutex;

    #[test]
    fn repeated_startup_callback_initializes_once() {
        let state = InitializationState::<()>::default();
        let mut calls = 0;

        for _ in 0..3 {
            state
                .initialize(|| {
                    calls += 1;
                    Ok::<_, ()>(())
                })
                .unwrap();
        }

        assert_eq!(calls, 1);
        assert!(state.is_initialized());
    }

    #[test]
    fn failed_initialization_can_retry_and_does_not_mark_ready() {
        let state = InitializationState::<()>::default();

        assert_eq!(
            state.initialize(|| Err("invalid context")),
            Err("invalid context")
        );
        assert!(!state.is_initialized());
        state.initialize(|| Ok::<_, &str>(())).unwrap();
        assert!(state.is_initialized());
    }

    #[test]
    fn recreated_process_starts_uninitialized() {
        let first_process = InitializationState::<()>::default();
        first_process.initialize(|| Ok::<_, ()>(())).unwrap();

        let recreated_process = InitializationState::<()>::default();
        assert!(!recreated_process.is_initialized());
        recreated_process.initialize(|| Ok::<_, ()>(())).unwrap();
        assert!(recreated_process.is_initialized());
    }
}
