# The app shell and the side

**Since 2026-11-13** (GitHub #13471). Someone logged in sees one side down the
left of the site: a head with the mark, the name and New chat, and a button that
puts the side away; the places, Chats (`/chat`), Projects (`/projects`) and
Worlds (`/worlds`), with the worlds they may edit under Worlds, Splice first, each
opening its editor (`/worlds/splice?edit`); the page's own list (on the chat, its sessions under section
heads); and at the foot who is logged in, a link to Settings and
Log out. It is 300 px wide. Below 760 px it is hidden, as the chat's sidebar was.

## The pieces

- **Drawing:** the kit's side pieces in `components/kit/frames.coax`: `k_side`,
  `k_side_head`, `k_side_places`, `k_side_list`, `k_side_mark`,
  `k_sub_place`, `k_who`, `k_side_open`, styled by `mono_css` (`components/skin/mono.coax`).
  `/components` shows them.
- **Rules:** `apps/openagents-web/site-side.coax`: which place an address is,
  which addresses the shell draws itself, the name at the foot (the first email
  from `/api/auth/account?as=lines`, else a short npub), the worlds under
  Worlds and the sections. Plain functions with their checks.
- **Worlds:** `GET /api/worlds` (`coax/web/api/worlds.c`) lists the worlds the
  asker may edit, one line each, `world ID NAME`, Splice first. Who may is the
  rule a write to the world is held to (`wr_may`: a site admin, or a realm's
  owner and admins). Someone logged out gets an empty list.
- **The shell:** `apps/openagents-web/chat-web.coax`. It draws the side and,
  beside it, either the chat or, at `/`, nothing.

## Why one module, and how navigation works

Each page of the site is its own document with its own module, so a link
rebuilds whatever is on the page, the side included. To keep the side still,
the pages it holds have to be drawn by one module that stays loaded while the
address changes.

The side and the chat's session list are one thing (the list is the side's
middle), so the shell is the chat's own module rather than a second module
that would have to be told the chat's sessions. At `/` it draws the side and
nothing else; at `/chat` (or any other address it is opened on, such as the
editor's foot) it draws the chat.

- The browser host (`coax/web/dom-host.js`) treats a module that imports
  `dom_go` as an app shell. It tells the module the address as it starts and
  after Back or Forward (event 13). A press on a link in the module is
  reported as a press; when the module answers with a pushed address, the host
  puts it in the address bar with the History API and the browser does not
  follow the link. Any other link is followed as usual, so Projects, Worlds and
  Settings are ordinary page loads.
- Moving between `/` and `/chat` never rebuilds the side. A direct load of
  either address works: `/` is `index.html`, `/chat` is `chat/index.html`, and
  both load the same module.
- For links that still leave the shell, the side is the same module's drawing
  with its state kept in the browser (whether it is put away, with the agents
  setting, through `dom_keep`), and the skin turns on cross-document view
  transitions (`@view-transition { navigation: auto }`, with
  `view-transition-name: side`), so the side stays still between two pages
  that both have it.

## The home page and its cost

`index.html` holds the public page's engine
(`<script type="module" src="/coaxial/host.js" data-app="newland">`) inside a
`<template>`, so none of it loads until one request to `/api/auth/account`
answers. Someone logged in gets the shell (dom-host and `chat-web.wasm`, no
engine); anyone else gets the template's script, which the release build still
finds and rewrites as before. A logged-out visitor pays that one small request
(at most 3 seconds when the API is slow) before the engine starts.

## What each remaining page needs

- **Projects (`/projects`) and Worlds (`/worlds`)** are drawn by the GPU on a
  canvas over the whole window (`projects.coax`, the galaxy). To sit in the
  shell they need the canvas confined to the room right of the side, as the
  editor already confines a world's canvas to what its panes leave free
  (`coax/web/host.js`, the editor frame: the canvas set to that rectangle and
  the pointer offset), and the shell has to open their module beside the side
  instead of a page load. Until then they are page loads, and they show no side.
- **Settings and the account pages (`/settings/*`, `/login`)** are GPU pages
  too; the same applies.
- **A new page drawn by the browser** joins the shell by adding a view to the
  shell module (as the home page is) or, when it is large, by being opened into
  a slot beside the side with `dom-host.js?app=NAME&root=ID`, as the editor
  opens the chat in its foot.
- **Resizing the side by dragging** is not built; the width is fixed at 300 px.
