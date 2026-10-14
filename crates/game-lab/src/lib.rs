//! Game Lab: change a game by talking (docs/game-lab/README.md).
//!
//! A game's editable surface is a typed `game.v1` document ([`doc`]);
//! every change is a version with a one-line summary and a digest, kept
//! privately on this computer ([`lab`]). An edit is a JSON Patch
//! ([`patch`]) checked against the schema and the game's rules, and the
//! rules crate's bot plays seeded runs with it ([`sim`]): an edit nobody
//! can win is refused. Versions play as data on one build of the game.
//! `game-lab serve` is the local service the web chat's `game_*` tools,
//! its inline card and `openagents game` call.

pub mod doc;
pub mod lab;
pub mod patch;
pub mod sim;
