# Verse OS on NixOS

The NixOS side of Verse OS ([spec](../../docs/os/2026-11-02-verse-os.md)).
The owner put it in this repository on 2026-11-02. It is the one place Nix
lives here, and it stays small: modules, and later the image. Everything a
module runs is Coax (`os/`, `verse/`).

| Path | What |
| --- | --- |
| `flake.nix` | `nixosModules.verse-os`, an example host, and the check that evaluates it. nixpkgs is pinned to `coderos-4080`'s revision |
| `modules/verse-os.nix` | `verseos.session`: a VT whose login runs `verse-session` (`os/verse-session.coax`), so the VT shows the Verse; the realms from a directory (`dir`) or a package (`package`) |
| `modules/verse-os-image.nix` | `verseos.image` (V7): the disk image, the Verse on tty1, a read-only root in slot A with slot B reserved, `systemd-sysupdate` filling the slot not in use, `/home` made and TPM2-encrypted by `systemd-repart` on first boot |
| `pkgs/verse-realms.nix` | The realms and `verse-session` built by Nix from `verse/gpu/realm.sh local ship DIR` output, as realm.sh builds them |
| `tests/ship-stub/` | A stand-in for that output, so the image evaluates without one; `ship/` (git-ignored) holds a real one |

## The session on a VT (V0)

1. Build the realms into a kept directory, from a checkout on the machine:

       verse/gpu/realm.sh local install ~/.local/share/verse-os/current

   `REALM="newland verse everglade ..."` picks the realms (the first is where
   travel starts). The directory holds a folder per realm, the travel file,
   and `verse-session`.

2. Turn the module on in the host, with the VT's own autologin:

   ```nix
   inputs.verse-os.url = "git+file:///home/alice/openagents?dir=os/nix";
   # ...
   imports = [ verse-os.nixosModules.verse-os ];
   services.getty.autologinUser = "alice";
   verseos.session = {
     enable = true;
     user = "alice";
     tty = 3;
     dir = "/home/alice/.local/share/verse-os/current";
     stateDir = "/home/alice/.local/state/verse";
   };
   ```

3. Ctrl+Alt+F3. The Grid is on the screen. Escape held a second shows VT 1;
   Ctrl+Alt+F3 again resumes. The log is
   `$XDG_RUNTIME_DIR/verse-session.log`.

`tty = 1` boots the machine into the Verse; that is V0's last step, and on
`coderos-4080` it waits for the owner (NEEDS_OWNER.md).

## The image (V7)

    REALM="newland verse everglade" verse/gpu/realm.sh local ship os/nix/ship
    nix build ./os/nix#verse-realms                     # the realms, from os/nix/ship
    nix build ./os/nix#nixosConfigurations.verse-os-image.config.system.build.image

The last builds the whole disk (several GB, a long build); run it on a
machine that isn't doing anything else. Publishing versions for
`systemd-sysupdate` (`verseos.image.updateUrl`) is not set up yet.

## Checks

    nix flake check ./os/nix                # the example host and the image evaluate
    coax/bin/coax test os/verse-session.coax
