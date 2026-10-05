//! Social worlds: zones such as Everglade hosted without the chamber's
//! combat (`docs/verse/networking.md`, "Shared Everglade").
//!
//! The renderer-free pieces live here so Verse's zone and a hosted
//! instance read one copy: the player [`controller`], ground
//! [`nav`]igation, Everglade's heightfield and studio places
//! ([`everglade`]), the zone's [`solids`], the studio [`seats`]' walk, and
//! the social rules profile's authority ([`world`]). With the `studio`
//! feature, [`studio`] places a NIP-HOST studio snapshot's seats, drives
//! them for every viewer, and checks the studio rights a panel needs.

pub mod controller;
pub mod everglade;
pub mod nav;
pub mod seats;
pub mod solids;
#[cfg(feature = "studio")]
pub mod studio;
pub mod world;

#[cfg(test)]
mod tests;
