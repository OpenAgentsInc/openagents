//! The owner's private assets in Everglade (`docs/verse/private-assets.md`).
//!
//! The desktop names Verse's home ([`WorldRuntime::configure_private_assets`]).
//! On each Everglade entry, the placements file there
//! (`private-assets.json`) names the broker, the profile whose key signs, and
//! where each private character stands; the packs load in the background and
//! each character joins the zone as it arrives. Without the file, a key the
//! broker admits, or the network, nothing is drawn and the zone is unchanged.
//! Committed code names no private asset, and a browser build compiles none
//! of this.

use std::path::PathBuf;

use super::everglade::guests::{self, Stand};
use super::everglade_pack::private_assets::{PrivateEvent, PrivateLoader};
use crate::runtime::WorldRuntime;

impl WorldRuntime {
    /// Lets Everglade load the owner's private placements from `home`,
    /// Verse's home directory. This reads nothing until Everglade is entered.
    pub fn configure_private_assets(&mut self, home: PathBuf) {
        self.zone_state.private_home = Some(home);
    }

    /// Starts loading the private placements for Everglade, when configured.
    pub(super) fn start_private_assets(&mut self) {
        self.zone_state.private_loader = None;
        let Some(home) = self.zone_state.private_home.clone() else {
            return;
        };
        let placements = match verse_private::placements::load(&home) {
            Ok(Some(placements)) => placements,
            Ok(None) => return,
            Err(error) => {
                eprintln!("verse: private assets are off: {error}");
                return;
            }
        };
        // Read only: a missing key signs with a throwaway one, which no
        // manifest names, so the broker refuses it.
        let identity = match crate::identity::load_or_ephemeral(&home, &placements.profile) {
            Ok(identity) => identity,
            Err(error) => {
                eprintln!("verse: private assets are off: {error}");
                return;
            }
        };
        self.zone_state.private_loader = PrivateLoader::start(
            &placements,
            "everglade",
            identity.signer,
            home.join(verse_private::placements::CACHE),
        );
    }

    /// Stands each private character that has arrived in Everglade.
    pub(super) fn poll_private_assets(&mut self) {
        let Some(loader) = &mut self.zone_state.private_loader else {
            return;
        };
        // Every event is sent before the worker finishes, so after a finish
        // seen here, one drain takes them all.
        let done = loader.finished();
        while let Some(event) = loader.poll() {
            match event {
                PrivateEvent::Ready { placement, pack } => {
                    let Some(everglade) = &mut self.zone_state.everglade else {
                        continue;
                    };
                    // A seat gives the place, the floor, and the facing.
                    let stand = match placement.seat.as_deref() {
                        Some(name) => match guests::seat(name, placement.scale) {
                            Some(stand) => stand,
                            None => {
                                eprintln!(
                                    "verse: {} names a seat Everglade doesn't have",
                                    placement.asset
                                );
                                continue;
                            }
                        },
                        None => Stand {
                            at: placement.at,
                            yaw: placement.yaw,
                            scale: placement.scale,
                            floor: None,
                        },
                    };
                    match everglade.add_guest(&pack, stand) {
                        Ok(()) => eprintln!("verse: {} stands in Everglade", placement.asset),
                        Err(error) => eprintln!("verse: {} can't stand: {error}", placement.asset),
                    }
                }
                PrivateEvent::Failed { asset, reason } => {
                    eprintln!("verse: private asset {asset} didn't load: {reason}");
                }
            }
        }
        if done {
            self.zone_state.private_loader = None;
        }
    }

    /// How many private characters stand in Everglade.
    #[must_use]
    pub fn private_guests(&self) -> usize {
        self.zone_state
            .everglade
            .as_ref()
            .map_or(0, super::Everglade::guest_count)
    }
}
