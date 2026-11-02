# Verse OS on NixOS

The NixOS side of Verse OS ([spec](../../docs/os/2026-11-02-verse-os.md)).
The owner put it in this repository on 2026-11-02. It is the one place Nix
lives here, and it stays small: modules, and later the image. Everything a
module runs is Coax (`os/`, `verse/`).

| Path | What |
| --- | --- |
| `flake.nix` | `nixosModules.verse-os`, an example host, and the check that evaluates it. nixpkgs is pinned to `coderos-4080`'s revision |
| `modules/verse-os.nix` | `verseos.session`: a VT whose login runs `verse-session` (`os/verse-session.coax`), so the VT shows the Verse |

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

## Checks

    nix flake check ./os/nix                # the example host evaluates and builds
    coax/bin/coax test os/verse-session.coax
