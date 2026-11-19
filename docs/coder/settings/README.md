# Coder settings: pixel check (#13726)

Coder's settings screen against Zeron's desktop settings
(`zeron/apps/zgx/ui/screens/shell/settings.luau`, `ds/settings.luau`,
`ds/account_row.luau`, `ds/tokens.luau`, `ds/themes/zeron-dark.json`).

Captures, dark, a 1486 by 986 point window at 2x, the usual text size (100%):

- `coder-general.png`: General.
- `coder-account.png`: Account.

There is no Zeron capture beside them: Zeron has no build on this Mac and no
reference screenshots, and building it was out of scope. The comparison is
against Zeron's numbers as its source sets them, measured from the captures.

## What matches (measured from the captures, in points)

- List rows 28 high, the search field 28, the Back row's top 46 from the
  window's top (Zeron's 38 strip plus its 8 padding). Coder's window has its
  own title bar, so the host passes its height (31 here) and the list starts
  under it at the same window position.
- The chosen row's wash `#ebebeb24`, hover `#ffffff0d`, the list headings
  Medium 11, row labels 13 Medium, icons 16 (the magnifier and chevron 14).
- The page column at most 640 with 32 side padding, centred; the title
  SemiBold 20, the description 13 on 20 lines, 32 between blocks.
- Cards: radius 12, `#ebebeb0b` tint, 1 edge and hairline rules; a one-line
  row 42 high; controls drop under the words when the words would get less
  than 180.
- Buttons 32 (primary `#ececee` on `#0b0b0c`), small buttons 28.
- Account: the 180 by 252 member card beside the rows, over them when the
  page is narrower than 620; the caption 12 under it.
- The account row at the list's foot: a 30 slot with a 20 avatar and the
  gear toggle 30 by 30. The chats' sidebar has the same row at the same place.

## Differences

- Coder's 13-point text is its 11-unit face: 13.2 points (0.2 over).
- Small buttons use Geist 12 Regular; Zeron's are 12 Medium.
- The list is the chats' sidebar's width (280 by default, 180 to 480 as
  dragged), not Zeron's own list width.
- The Command key on the search's key cap is an icon (`icons/command.svg`):
  the display faces hold ASCII only.
- Account: Coder has no account of its own, so the page says it works
  without one and offers Log in (the web page) instead of Zeron's
  signed-in profile.
- Pages: General, Models, Foreman, Account and About & updates. Zeron's
  other pages are for features Coder does not have, so Coder leaves them out.
