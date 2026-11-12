# Chat

<<<<<<< 43b3d06e25ee73090f8916d5e7481d6c5a2a9fdd
The OpenAgents chat, being rebuilt from nothing. For now the page is a box to type in, at the foot of the window, with Send, under the messages you have sent. The box grows as you type, to eight lines, and then scrolls. Enter or Send takes what you typed and clears the box; Shift and Enter makes a new line. Your message appears above the box, and an answer comes under it. The answer is a placeholder the page writes itself: nothing is sent anywhere yet and no model answers. The messages are gone when you leave the page.
=======
A direct chat with the OpenAgents agent for general questions. It reads a fresh copy of the public [OpenAgents repository](https://github.com/OpenAgentsInc/openagents) (main) and the web, says which files it read, and can't change anything. Pick Claude Haiku 5.5, Sonnet 5.5 or Opus 5.5 for each conversation. Under Key you can use your own Anthropic API key or Claude subscription token. Theme switches the page between the skins on [/components](/components): Zeron (the default), Everglade, Verse and Terminal. Your browser remembers the one you pick.
>>>>>>> c4040e643dba7610e09b4afa78b506b398ccb0ef^

Admins only for now. An admin logs in with their Nostr key: `coax/web/api/admin.sh chat` in the OpenAgents repo, or with the OpenAgents phone app at [/login](/login). Anyone else sees only Log in.
