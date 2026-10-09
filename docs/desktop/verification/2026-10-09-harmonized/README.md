# Desktop harmonized with the web, October 9, 2026

[#11120](https://github.com/OpenAgentsInc/openagents/issues/11120). The
1.0 window's screens in Coder Light and Coder Noir, painted by
`openagents-desktop --capture-kept DIR` from a debug build (1200 by 840
points at 2x, an in-process host, example chats).

| Screen | Light | Dark |
| --- | --- | --- |
| New chat | [chat-new-light](chat-new-light.png) | [chat-new-dark](chat-new-dark.png) |
| A conversation | [chat-conversation-light](chat-conversation-light.png) | [chat-conversation-dark](chat-conversation-dark.png) |
| Account menu | [account-menu-light](account-menu-light.png) | [account-menu-dark](account-menu-dark.png) |
| Connect a phone | [connect-phone-light](connect-phone-light.png) | [connect-phone-dark](connect-phone-dark.png) |
| Settings, Appearance | [settings-appearance-light](settings-appearance-light.png) | [settings-appearance-dark](settings-appearance-dark.png) |
| Settings, Coder on this computer | [settings-coder-light](settings-coder-light.png) | [settings-coder-dark](settings-coder-dark.png) |

Next to the web: the desktop's new chat (left) and the web's home page
from the same commit (right, `openagents-web` run locally, headless
Chrome at 1200 by 840):
[light](side-by-side-chat-new-light.png),
[dark](side-by-side-chat-new-dark.png); the web pages alone:
[light](web-chat-new-light.png), [dark](web-chat-new-dark.png). The web's
Settings needs a signed-in account and the web has no Connect a phone
page, so those screens have no web counterpart here.

What the captures show:

- Text in the system face the web's `--font-sans` stack names (SF Pro on a
  Mac) and code in its `--font-mono` (SF Mono); the type scale is the
  web's (sidebar rows text-sm 14/20, titles text-xs 12/18, the wordmark
  text-md, headings heading-lg).
- The sidebar as on the web: the wordmark, then labelled rows (New chat,
  Connect a phone, Settings), the chats, then projects, and the account
  row with the theme toggle (`Light` or `Dark`) in its corner. The
  titlebar's sidebar toggle and Chat actions carry their labels.
- Dark is Coder Noir from the shared token table (`oa-tokens`), light is
  Coder Light.
- No Verse, Map, Gym runs, or Give feedback: those return in a build made
  with `OPENAGENTS_DESKTOP_PREVIEW=on`.

Second pass (same day): the conversation is on the web's chat scale
(text-md 16/24 body, headings heading-lg to heading-xs, code text-sm),
shared with the phone through `oa_tokens::typography::conversation`; the
command palette and account menu rows are on `--menu-font-size` (text-sm
14/20, second lines text-xs); the composer says "Ask OpenAgents anything"
as the web does; the account menu is headed by the computer's name (the
captures answer as a host named "Studio Mac"); cards, the Connect a phone
page's buttons, and the slide viewer's chrome paint from the shared token
roles in both looks.
