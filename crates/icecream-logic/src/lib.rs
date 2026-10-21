//! The Ice Cream Man's rules as a swappable WebAssembly module.
//!
//! The hosted zone (`docs/verse/ice-cream-man-live.md`) loads this crate,
//! built for `wasm32-unknown-unknown`, by its content hash from the
//! tenant's manifest. It is a plain core module: no imports, no JavaScript
//! glue, and five exports. The page keeps the game state and passes it in
//! on every step, so the page can drop this module for a newer one, or roll
//! back to an older one, between two steps without losing the run.
//!
//! # The ABI
//!
//! | Export | Does |
//! | --- | --- |
//! | `ll_abi() -> u32` | This ABI's version, [`ABI`] |
//! | `ll_state_version() -> u32` | The newest state it reads ([`icecream_rules::live::STATE_VERSION`]) |
//! | `ll_alloc(len) -> ptr` | A buffer for a request |
//! | `ll_call(ptr, len) -> ptr` | Runs a request; the reply is at `ptr` |
//! | `ll_reply_len() -> u32` | The reply's length in bytes |
//!
//! A request is UTF-8 text: an operation, a newline, its arguments as JSON,
//! a newline, and the state (empty for `new`). A reply is `ok` or `err`, a
//! newline, a JSON note, a newline, and the new state. [`call`] holds the
//! operations, so native tests run the same code the module does.

use std::cell::RefCell;

use icecream_rules::live::{HouseCache, Retune};
use icecream_rules::{Game, Input, Options, Tuning};
use serde::Deserialize;

/// The version of the export table and the request format.
pub const ABI: u32 = 1;

#[derive(Deserialize)]
struct New {
    seed: u64,
    #[serde(default)]
    tuning: Tuning,
    #[serde(default)]
    options: Option<Options>,
}

#[derive(Deserialize)]
struct Step {
    input: Input,
    /// Steps to take with the same input.
    #[serde(default = "one")]
    steps: u32,
}

fn one() -> u32 {
    1
}

#[derive(Deserialize)]
struct Retuned {
    tuning: Tuning,
}

/// Runs one request against `cache`: `new`, `step`, or `retune`.
#[must_use]
pub fn call(request: &str, cache: &mut HouseCache) -> String {
    match run(request, cache) {
        Ok((note, state)) => format!("ok\n{note}\n{state}"),
        Err(error) => format!("err\n{}\n", serde_json::Value::String(error)),
    }
}

fn run(request: &str, cache: &mut HouseCache) -> Result<(String, String), String> {
    let mut parts = request.splitn(3, '\n');
    let op = parts.next().unwrap_or_default();
    let args = parts.next().unwrap_or("{}");
    let state = parts.next().unwrap_or_default();
    let bad = |e: serde_json::Error| format!("bad {op} arguments: {e}");
    match op {
        "new" => {
            let new: New = serde_json::from_str(args).map_err(bad)?;
            let options = new.options.unwrap_or_default();
            let game = Game::tuned(new.seed, options, new.tuning);
            Ok(("{}".into(), game.to_state()))
        }
        "step" => {
            let step: Step = serde_json::from_str(args).map_err(bad)?;
            let mut game = Game::from_state(state, cache)?;
            for _ in 0..step.steps.min(8) {
                game.step(step.input);
            }
            Ok(("{}".into(), game.to_state()))
        }
        "retune" => {
            let retuned: Retuned = serde_json::from_str(args).map_err(bad)?;
            let mut game = Game::from_state(state, cache)?;
            let note = match game.retune(retuned.tuning) {
                Retune::Applied { house_changed } => {
                    format!("{{\"applied\":true,\"house_changed\":{house_changed}}}")
                }
                Retune::NextRun(why) => {
                    format!(
                        "{{\"applied\":false,\"why\":{}}}",
                        serde_json::Value::from(why)
                    )
                }
            };
            Ok((note, game.to_state()))
        }
        other => Err(format!("unknown operation {other:?}")),
    }
}

thread_local! {
    static CACHE: RefCell<HouseCache> = RefCell::new(HouseCache::default());
    static REQUEST: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static REPLY: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// This ABI's version.
#[unsafe(no_mangle)]
pub extern "C" fn ll_abi() -> u32 {
    ABI
}

/// The newest state version this module reads.
#[unsafe(no_mangle)]
pub extern "C" fn ll_state_version() -> u32 {
    icecream_rules::live::STATE_VERSION
}

/// A buffer of `len` bytes for the next request, valid until the next call.
#[unsafe(no_mangle)]
pub extern "C" fn ll_alloc(len: u32) -> *mut u8 {
    REQUEST.with_borrow_mut(|request| {
        request.clear();
        request.resize(len as usize, 0);
        request.as_mut_ptr()
    })
}

/// Runs the request in the buffer from [`ll_alloc`] and returns where its
/// reply is; [`ll_reply_len`] gives its length.
#[unsafe(no_mangle)]
pub extern "C" fn ll_call(_ptr: *const u8, len: u32) -> *const u8 {
    let text = REQUEST.with_borrow(|request| {
        let len = (len as usize).min(request.len());
        String::from_utf8_lossy(&request[..len]).into_owned()
    });
    let reply = CACHE.with_borrow_mut(|cache| call(&text, cache));
    REPLY.with_borrow_mut(|out| {
        *out = reply.into_bytes();
        out.as_ptr()
    })
}

/// The length of the last reply.
#[unsafe(no_mangle)]
pub extern "C" fn ll_reply_len() -> u32 {
    REPLY.with_borrow(|out| u32::try_from(out.len()).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(text: &str) -> (bool, serde_json::Value, String) {
        let mut parts = text.splitn(3, '\n');
        let ok = parts.next() == Some("ok");
        let note = serde_json::from_str(parts.next().unwrap()).unwrap();
        (ok, note, parts.next().unwrap_or_default().to_owned())
    }

    #[test]
    fn new_step_and_retune_carry_one_run() {
        let mut cache = HouseCache::default();
        let (ok, _, state) = reply(&call("new\n{\"seed\":7}\n", &mut cache));
        assert!(ok);
        let step = "step\n{\"input\":{\"walk\":{\"x\":0.0,\"z\":-1.0},\"sprint\":true,\"jump\":false,\"fire\":false,\"aim\":{\"x\":0.0,\"z\":-1.0},\"interact\":false,\"select\":null},\"steps\":4}\n";
        let (ok, _, stepped) = reply(&call(&format!("{step}{state}"), &mut cache));
        assert!(ok);
        let game = Game::from_state(&stepped, &mut cache).unwrap();
        assert_eq!(game.tick, 4);

        // A fresh module (a new build, or a rollback) carries on from it.
        let mut fresh = HouseCache::default();
        let (ok, note, retuned) = reply(&call(
            &format!("retune\n{{\"tuning\":{{\"extra_closets\":2}}}}\n{stepped}"),
            &mut fresh,
        ));
        assert!(ok);
        assert_eq!(note["applied"], true);
        let after = Game::from_state(&retuned, &mut fresh).unwrap();
        assert_eq!(after.tick, 4);
        assert_eq!(after.player.pos, game.player.pos);
        assert_eq!(after.tuning.extra_closets, 2);
    }

    #[test]
    fn a_bad_request_is_an_error_reply_not_a_trap() {
        let mut cache = HouseCache::default();
        let (ok, note, _) = reply(&call("step\n{}\nnot a state", &mut cache));
        assert!(!ok);
        assert!(note.as_str().unwrap().contains("bad step"));
        let (ok, _, _) = reply(&call("fly\n{}\n", &mut cache));
        assert!(!ok);
    }
}
