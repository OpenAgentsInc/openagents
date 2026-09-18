//! Settings resolution: explicit, then environment, then default — the same
//! order both official SDKs resolve theirs in. The whole run happens in one
//! test so the environment it mutates is this process's alone.

use std::time::Duration;

use jev::{Client, Config, Error};

type Outcome = Result<(), Box<dyn std::error::Error>>;

const KEY: &str = "TYPESAFE_API_KEY";
const BASE: &str = "TYPESAFE_BASE_URL";
const MODEL: &str = "TYPESAFE_DEFAULT_MODEL";

/// Set or clear one variable, unsafely because the environment is shared. The
/// file runs one test, so nothing else in the process reads it at the same
/// time.
fn set(name: &str, value: Option<&str>) {
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

/// The environment as it stood, put back when the guard drops.
struct Saved([(&'static str, Option<String>); 3]);

impl Saved {
    fn take() -> Self {
        Self([
            (KEY, std::env::var(KEY).ok()),
            (BASE, std::env::var(BASE).ok()),
            (MODEL, std::env::var(MODEL).ok()),
        ])
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            set(name, value.as_deref());
        }
    }
}

#[test]
fn resolution_is_explicit_then_env_then_default() -> Outcome {
    let _saved = Saved::take();
    for name in [KEY, BASE, MODEL] {
        set(name, None);
    }

    // With the environment empty, the defaults hold and the missing key names
    // the variable that sets it.
    let Err(Error::Config(message)) = Client::new(Config::new()) else {
        unreachable!("no key anywhere is a config error");
    };
    assert!(message.contains(KEY), "{message}");

    let client = Client::new(Config::new().api_key("explicit"))?;
    assert_eq!(client.base_url(), "https://api.typesafe.ai");
    assert_eq!(client.default_model(), "jev-latest");
    assert_eq!(client.timeout(), Duration::from_secs(10));

    // The environment fills in what the caller leaves out.
    set(KEY, Some("env-key"));
    set(BASE, Some("http://127.0.0.1:9000/v1///"));
    set(MODEL, Some("jev-env"));
    let client = Client::new(Config::new())?;
    assert_eq!(
        client.base_url(),
        "http://127.0.0.1:9000/v1",
        "trailing slashes come off"
    );
    assert_eq!(client.default_model(), "jev-env");

    // An explicit setting wins over the environment.
    let client = Client::new(
        Config::new()
            .api_key("explicit")
            .base_url("http://127.0.0.1:8000")
            .default_model("jev-explicit"),
    )?;
    assert_eq!(client.base_url(), "http://127.0.0.1:8000");
    assert_eq!(client.default_model(), "jev-explicit");

    // A blank or whitespace value is no value.
    set(KEY, Some("   "));
    set(BASE, Some(""));
    set(MODEL, Some("\t"));
    let Err(Error::Config(message)) = Client::new(Config::new()) else {
        unreachable!("a whitespace key is no key");
    };
    assert!(message.contains(KEY), "{message}");

    set(KEY, Some("env-key"));
    let client = Client::new(Config::new())?;
    assert_eq!(client.base_url(), "https://api.typesafe.ai");
    assert_eq!(client.default_model(), "jev-latest");
    Ok(())
}
