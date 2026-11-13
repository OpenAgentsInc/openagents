# Crew

An unlisted Metaverse Engine page at `/worlds/crew`, written in Coax: character creation for the
cel realms. A player picks and tunes a body, then dresses it from one wardrobe of black work
clothes with white stitch lines. Every garment is fitted to the body's measurements, so every
garment fits every body. The spec is `docs/metaverse/2026-11-13-character-creation.md`.

## Views

- `/worlds/crew`: the creation screen. A turntable on your character, with five steps in the
  pane beside it: Body, Face and hair, Outfit, Carry, Name.
- `/worlds/crew?view=row`: six example characters beside a height scale, one per body preset,
  each in a different outfit.
- `/worlds/crew?view=fit&outfit=runner`: one outfit on all six bodies. The outfits are
  `flight-suit`, `deckhand`, `vac-suit`, `runner`, `shroud` and `fitter`.
- `/worlds/crew?view=outfits&body=fighter`: the six outfits on one body. The bodies are
  `bruiser`, `mechanic`, `scout`, `elder`, `fighter` and `kid`.

`&turn=DEGREES` holds the turntable at an angle. `&doc=BASE64URL` loads a character document.

## The character document

A character is one small text document, `crew.character.v1`
(`docs/protocol/crew-character-v1.md`). The page saves it in this browser, and an agent can write
one through the page's address. A document with a value out of range is refused with the reason,
never clamped.
