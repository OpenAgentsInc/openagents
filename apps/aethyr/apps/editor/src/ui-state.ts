import * as Atom from "effect/reactivity/Atom";
export const uiAtoms = {
  assetSearch: Atom.make("").pipe(Atom.keepAlive),
  assetChoice: Atom.make("").pipe(Atom.keepAlive),
  count: Atom.make(1).pipe(Atom.keepAlive),
  placement: Atom.make<"ahead" | "left" | "right">("ahead").pipe(
    Atom.keepAlive,
  ),
  commandText: Atom.make("").pipe(Atom.keepAlive),
  settingsTab: Atom.make<"light" | "sky" | "surface">("light").pipe(
    Atom.keepAlive,
  ),
};
