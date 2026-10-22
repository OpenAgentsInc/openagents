# The Haunted House in Game Lab (#12476)

The Haunted House is Game Lab's second game. The chat's model changes it
only with the `haunt_*` tools (`game_lab::haunt`), each one typed edit that
the house's rules check; the model's choice of tool is the route, and no
words are matched.

## Live, locally

`werewolf-live.jpg`: a house page standing at the south end of the long
hall, before and after `haunt_place` with "Add a werewolf in the long hall
that howls when you get near". Game Lab saved version 2 with the
`werewolf` card and its default trigger (`howl-wolf` within 3 m), its
on-save hook baked and deployed the `haunt` tenant in 26 ms, and the open
page showed the werewolf 0.94 s after the tool call, without a reload.
`haunt_audio` with the music down by a quarter reached the same page's
music bus 184 ms after the call. `capture.json` has the readings.

## On the website

`/games/the-haunted-house` follows the signed-in account's Game Lab copy
(`/labs/game/haunt/house`), asking every second, so a change shows in an
open house without a reload. A tester account saves one change; a refused
edit costs nothing; site admins and the accounts in
`OPENAGENTS_GAME_LAB_UNLIMITED` have no limit. The account tests
(`a_tester_makes_one_change_to_their_own_house_and_an_admin_has_no_limit`
in `crates/openagents-web/src/game_lab/hosted/tests.rs`) cover the
werewolf, the refused second change, another tester's separate house, and
an admin's three changes, with synthetic accounts.

## Not checked here

- A spoken request: dictation into the Game Lab chat (the phone's, or the
  paired browser's) reaches `haunt_audio` through the chat's model. That
  needs a deployed site and a test account; it's an after-close check.
- The in-zone mic button isn't built.
