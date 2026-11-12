# Chat

The OpenAgents chat, being rebuilt from nothing. For now the page is a box to type in, at the foot of the window, with Send, under the messages you have sent. The box grows as you type, to eight lines, and then scrolls. Enter or Send takes what you typed and clears the box; Shift and Enter makes a new line. Your message appears above the box, and the answer comes in under it as it is written. While it comes, Send is Stop. Copy, under a finished answer, copies it.

One model answers, Gemini on Google's Vertex AI, with the conversation so far. It has no tools, reads no files and no web, and remembers nothing between visits: the agent that will do those things is not connected yet. The messages are gone when you leave the page. Admins only for now.

The page before this one is at [/chat-gpu](/chat-gpu/index.md). The parts this one is built from are on [/components](/components/index.md).
