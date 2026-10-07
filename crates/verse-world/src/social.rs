//! Social worlds: zones such as Everglade hosted without the chamber's
//! combat (`docs/verse/networking.md`, "Shared Everglade").
//!
//! The renderer-free pieces live here so Verse's zone and a hosted
//! instance read one copy: the player [`controller`], ground
//! [`nav`]igation, Everglade's heightfield and studio places
//! ([`everglade`]) and its ponds and stream ([`everglade_water`]), the
//! zone's [`solids`] and what the third-person camera sees through
//! ([`sight`]), the studio [`seats`]' walk, and
//! the social rules profile's authority ([`world`]). [`hosted`] builds the
//! closed profile a chamber host serves Everglade under and feeds it the
//! studio's seats. With the `studio`
//! feature, [`studio`] places a NIP-HOST studio snapshot's seats, drives
//! them for every viewer, and checks the studio rights a panel needs.

pub mod columns;
pub mod controller;
pub mod everglade;
pub mod everglade_water;
pub mod hosted;
pub mod nav;
pub mod seats;
pub mod sight;
pub mod solids;
#[cfg(feature = "studio")]
pub mod studio;
pub mod world;

#[cfg(test)]
mod tests;
