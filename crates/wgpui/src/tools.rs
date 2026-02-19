use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn is_browser() -> bool {
    cfg!(target_arch = "wasm32")
}

pub fn cx<I, S>(values: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<String>>()
        .join(" ")
}

pub fn cx_optional<I, S>(values: I) -> String
where
    I: IntoIterator<Item = Option<S>>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .filter_map(|value| value.map(|inner| inner.as_ref().trim().to_string()))
        .filter(|value| !value.is_empty())
        .collect::<Vec<String>>()
        .join(" ")
}

pub fn filter_props<K: Ord, V>(props: BTreeMap<K, Option<V>>) -> BTreeMap<K, V> {
    props
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect()
}

pub fn randomize_list<T: Clone>(values: &[T], seed: Option<u64>) -> Vec<T> {
    let mut output = values.to_vec();
    let mut state = seed.unwrap_or_else(default_seed);

    for i in (1..output.len()).rev() {
        let j = (next_u32(&mut state) as usize) % (i + 1);
        output.swap(i, j);
    }

    output
}

fn default_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn next_u32(state: &mut u64) -> u32 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) as u32
}

pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub enum ImageLoadError {
    DecodeFailed,
    IoFailed,
}

pub fn load_image_from_bytes(bytes: &[u8]) -> Result<ImageData, ImageLoadError> {
    let image = image::load_from_memory(bytes).map_err(|_| ImageLoadError::DecodeFailed)?;
    let image = image.to_rgba8();
    let (width, height) = image.dimensions();
    Ok(ImageData {
        width,
        height,
        pixels: image.into_raw(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_image_from_path(
    path: impl AsRef<std::path::Path>,
) -> Result<ImageData, ImageLoadError> {
    let bytes = std::fs::read(path).map_err(|_| ImageLoadError::IoFailed)?;
    load_image_from_bytes(&bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_image_from_url(url: &str) -> Result<ImageData, ImageLoadError> {
    let response = reqwest::blocking::get(url).map_err(|_| ImageLoadError::IoFailed)?;
    let bytes = response.bytes().map_err(|_| ImageLoadError::IoFailed)?;
    load_image_from_bytes(&bytes)
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
pub async fn load_image_from_url(url: &str) -> Result<ImageData, ImageLoadError> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or(ImageLoadError::IoFailed)?;
    let response_value = JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|_| ImageLoadError::IoFailed)?;
    let response: web_sys::Response = response_value
        .dyn_into()
        .map_err(|_| ImageLoadError::IoFailed)?;
    let buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|_| ImageLoadError::IoFailed)?,
    )
    .await
    .map_err(|_| ImageLoadError::IoFailed)?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    load_image_from_bytes(&bytes)
}

pub type TOSchedulerId = String;

#[cfg(not(target_arch = "wasm32"))]
mod scheduler {
    use super::TOSchedulerId;
    use std::collections::HashMap;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;
    use std::time::Duration;

    const DEFAULT_ID: &str = "";

    pub struct TOScheduler {
        ledger: Arc<Mutex<HashMap<TOSchedulerId, Arc<AtomicBool>>>>,
    }

    impl Default for TOScheduler {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TOScheduler {
        pub fn new() -> Self {
            Self {
                ledger: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn is_pending(&self, id: Option<&str>) -> bool {
            let id = id.unwrap_or(DEFAULT_ID);
            self.ledger
                .lock()
                .map(|m| m.contains_key(id))
                .unwrap_or(false)
        }

        pub fn stop(&self, id: Option<&str>) {
            let id = id.unwrap_or(DEFAULT_ID);
            if let Ok(mut ledger) = self.ledger.lock()
                && let Some(flag) = ledger.remove(id)
            {
                flag.store(true, Ordering::SeqCst);
            }
        }

        pub fn stop_all(&self) {
            if let Ok(mut ledger) = self.ledger.lock() {
                for (_, flag) in ledger.drain() {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }

        pub fn start<F>(&self, delay: Duration, callback: F)
        where
            F: FnOnce() + Send + 'static,
        {
            self.start_with_id(DEFAULT_ID.to_string(), delay, callback);
        }

        pub fn start_with_id<F>(&self, id: TOSchedulerId, delay: Duration, callback: F)
        where
            F: FnOnce() + Send + 'static,
        {
            self.stop(Some(&id));

            let cancelled = Arc::new(AtomicBool::new(false));
            if let Ok(mut ledger) = self.ledger.lock() {
                ledger.insert(id.clone(), cancelled.clone());
            }

            let ledger = self.ledger.clone();
            thread::spawn(move || {
                thread::sleep(delay);
                if cancelled.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(mut ledger) = ledger.lock() {
                    ledger.remove(&id);
                }
                callback();
            });
        }
    }

    pub fn create_to_scheduler() -> TOScheduler {
        TOScheduler::new()
    }

    pub use TOScheduler as Scheduler;
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
mod scheduler {
    use super::TOSchedulerId;
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    use std::time::Duration;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    const DEFAULT_ID: &str = "";

    struct Scheduled {
        timeout_id: i32,
        _closure: Closure<dyn FnMut()>,
    }

    pub struct TOScheduler {
        ledger: Rc<RefCell<BTreeMap<TOSchedulerId, Scheduled>>>,
    }

    impl TOScheduler {
        pub fn new() -> Self {
            Self {
                ledger: Rc::new(RefCell::new(BTreeMap::new())),
            }
        }

        pub fn is_pending(&self, id: Option<&str>) -> bool {
            let id = id.unwrap_or(DEFAULT_ID);
            self.ledger.borrow().contains_key(id)
        }

        pub fn stop(&self, id: Option<&str>) {
            let id = id.unwrap_or(DEFAULT_ID);
            if let Some(scheduled) = self.ledger.borrow_mut().remove(id) {
                if let Some(window) = web_sys::window() {
                    let _ = window.clear_timeout_with_handle(scheduled.timeout_id);
                }
            }
        }

        pub fn stop_all(&self) {
            let ids: Vec<String> = self.ledger.borrow().keys().cloned().collect();
            for id in ids {
                self.stop(Some(&id));
            }
        }

        pub fn start<F>(&self, delay: Duration, callback: F)
        where
            F: FnOnce() + 'static,
        {
            self.start_with_id(DEFAULT_ID.to_string(), delay, callback);
        }

        pub fn start_with_id<F>(&self, id: TOSchedulerId, delay: Duration, callback: F)
        where
            F: FnOnce() + 'static,
        {
            self.stop(Some(&id));

            let ledger_for_closure = self.ledger.clone();
            let ledger_for_insert = self.ledger.clone();
            let id_clone = id.clone();
            let mut callback = Some(callback);

            let closure = Closure::wrap(Box::new(move || {
                if let Some(cb) = callback.take() {
                    cb();
                }
                ledger_for_closure.borrow_mut().remove(&id_clone);
            }) as Box<dyn FnMut()>);

            if let Some(window) = web_sys::window() {
                let delay_ms = delay.as_millis() as i32;
                if let Ok(timeout_id) = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        closure.as_ref().unchecked_ref(),
                        delay_ms,
                    )
                {
                    ledger_for_insert.borrow_mut().insert(
                        id,
                        Scheduled {
                            timeout_id,
                            _closure: closure,
                        },
                    );
                }
            }
        }
    }

    pub fn create_to_scheduler() -> TOScheduler {
        TOScheduler::new()
    }

    pub use TOScheduler as Scheduler;
}

#[cfg(all(target_arch = "wasm32", not(feature = "web")))]
compile_error!("tools::TOScheduler requires the `web` feature on wasm32 targets.");

pub use scheduler::{Scheduler as TOScheduler, create_to_scheduler};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cx() {
        let joined = cx(["alpha", "beta", ""]);
        assert_eq!(joined, "alpha beta");
    }

    #[test]
    fn test_cx_optional() {
        let joined = cx_optional([Some("alpha"), None, Some("beta")]);
        assert_eq!(joined, "alpha beta");
    }

    #[test]
    fn test_filter_props() {
        let mut input = BTreeMap::new();
        input.insert("a", Some(1));
        input.insert("b", None);
        let filtered = filter_props(input);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered.get("a"), Some(&1));
    }

    #[test]
    fn test_randomize_list_deterministic() {
        let values = vec![1, 2, 3, 4, 5];
        let shuffled = randomize_list(&values, Some(42));
        let shuffled_again = randomize_list(&values, Some(42));
        assert_eq!(shuffled, shuffled_again);
        assert_eq!(shuffled.len(), values.len());
    }
}
