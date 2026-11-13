# Components

The components the OpenAgents chat is being rebuilt from. It is a small set, and each one is a frame around content, in black, white and the greys between. The browser draws them, so the text on the page is real text: you can select it, search it, zoom it and have it read aloud. The page needs no WebGPU.

## What is on the page

Every component, working:

- **Button**: an octagon and a label. The primary action is the inverse (white, black text). A disabled one is dashed.
- **Field and composer**: real text fields. Enter sends; Shift and Enter makes a line.
- **Conversation**: your message in a dim frame, a tool call as a row that opens onto what was sent and what came back, and the agent's answer as plain Markdown with no frame.
- **Code block**: its language and Copy in a strip, then the code.
- **Code field**: code to edit, in the code face, with no wrapping. Enter makes a line and Tab indents.
- **Drop and file**: a place to drop a file or pick one, which says what it takes and how large before anything is sent, and a file in a list by its name and what it is.
- **Notice**: an error, or a request for approval with Allow and Deny.
- **Row**: a conversation in a list. The current one is bright over an underline.
- **Menu**: rows in a panel.
- **World card**: a world as a conversation shows it: its name, what it holds, and a link that opens its editor.
- **Dock**: panes round something that still shows: a strip of actions, a list, the picked thing's fields and facts, and a foot. A wide right pane holds a file of code. A stepper is a value with a button to lower it and one to raise it. The editor that `?edit` opens over a world is built from it.
- **Header and text**: the title types in.

Under a finger every control is at least 44 px either way. With a mouse they keep their smaller desktop sizes.

**Replay the entrance** enters everything again. If your system asks for reduced motion, nothing animates.

## How it is built

The components are written in Coax (`components/kit/frames.coax`). A component builds nodes in a tree (`components/core/tree.coax`): each node has a stable id, a tag, its text or children, and the frame behind it. The browser backend (`components/backend/dom.coax`) compares each tree with the last one it sent and writes only the changes. A small host (`coax/web/dom-host.js`) applies them and holds no logic. A page that is still sends nothing and does no work.

The look is one skin, `mono` (`components/skin/mono.coax`). It writes the page's whole stylesheet from Coax. There is no colour in it: state is shown by brightness, fill, line style and words.

## The frames

The frames and their motion are [ARWES](https://github.com/arwes/arwes/tree/bdbaa0324900ee978d42036d1304a053c1fe54b5), copyright 2014–2024 Romel Perez, under the [MIT license](https://github.com/arwes/arwes/blob/bdbaa0324900ee978d42036d1304a053c1fe54b5/LICENSE), ported to Coax (`components/core/arwes/`). A frame is an SVG behind its content, with three layers: the ground, the lines and the decorations. The page reports each framed node's size, Coax computes the frame's points for that size, and the browser draws them as paths. A frame assembles when its node first appears: the ground fades in, the lines draw, the decorations arrive.

## The earlier components

The components drawn on the GPU, in four skins, are at [/components-gpu](/components-gpu/index.md). OpenAgents for desktop still uses them.

The source is in the [OpenAgents repository](https://github.com/OpenAgentsInc/openagents) under `components/`. The reasons for this set are in `docs/audits/2026-11-11-chat-zero-base-audit.md`, sections 10 and 11.
