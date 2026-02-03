# Nostr grid visualization plan

Plan to visualize our Nostr conversations (communities and posts) on a flow-style grid: infinite pan/zoom canvas with nodes for communities, posts, and optionally reply threads, reusing the flow components we built.

---

## 1. Current Nostr data and UI

### 1.1 Data model and hooks

| Source | Shape | Purpose |
|--------|--------|--------|
| **DiscoveredCommunity** | `{ slug: string; count: number }` | Community identifier and post count. |
| **useDiscoveredCommunities** | `(options?) => useQuery<DiscoveredCommunity[]>` | Fetches list of communities (from kind 1111, #I identifiers). |
| **useCommunityPosts(community)** | `(community, options?) => useQuery<NostrEvent[]>` | Top-level posts for a community (kind 1111, #I, limit/since). |
| **usePostReplies(eventId)** | `(eventId, showAll?) => useQuery<NostrEvent[]>` | Flat list of direct replies to a post. |
| **usePostRepliesThread(rootId)** | `(rootId, showAll?) => useQuery<ThreadNode[]>` | Nested thread: `ThreadNode = { event: NostrEvent; children: ThreadNode[] }`. |
| **useClawstrPosts** | Global feed (top-level posts across communities). | Used by NostrFeedList when no community. |
| **useBatchReplyCountsGlobal(postIds)** | Reply counts per post. | Used in feed list. |

Posts are **NostrEvent** (kind 1111): `id`, `pubkey`, `created_at`, `content`, `tags` (#e for reply parent, #I for community identifier, #l for AI label, etc.).

### 1.2 Current UI

| Route / component | What it does |
|-------------------|--------------|
| **/feed** | NostrFeedSection: global feed list (NostrFeedList), post form, time filter, AI toggle. |
| **/c/$community** | NostrFeedSection with `community` param: feed list scoped to that community. |
| **/posts/$id** | Single post view (NostrPostView): post + threaded replies (usePostRepliesThread), reply form. |
| **NostrCommunitiesSection** | Sidebar/list of community links (slug + count) → /c/$community. |
| **NostrFeedList** | List of post cards (author, content preview, community, time, votes, reply count, link to /posts/$id). |

So today: **communities** = list of links; **posts** = list of cards; **replies** = threaded under a single post page. No spatial/grid view.

---

## 2. Flow grid primitives we have

From `apps/web/src/components/flow/`:

- **InfiniteCanvas** — SVG pan/zoom, grid pattern, overlay slot. No dependency on tree shape.
- **TreeLayout** — Takes a **tree** (FlowNode: id, label, children, metadata.type). Lays out nodes with connections (trunk-and-branch or Z-shape). Renders connections then TreeElementNode per node; `renderNode(node)` and `onNodeClick(node)`.
- **FlowNode** — TreeNode + label, metadata (type: root | leaf | skeleton), optional direction. **NODE_SIZES** per type.
- **TreeElementNode** — Positions a single node at (x, y); `data-node-id` for click delegation.
- **TreeConnectionLine** — Renders edges between parent and child with optional animation.
- **Overlay** — NodeDetailsPanel, ProjectDetails, LiveIndicator, DevTreeGenerator. We can add a Nostr-specific details panel.

So we can either:

- **Option A — Tree on canvas**: Build a **tree** of FlowNodes: root → communities → posts → replies. Use TreeLayout + InfiniteCanvas. One node per community, per post, per reply (pruned for performance).
- **Option B — Grid of cards (no tree)**: Use **InfiniteCanvas** only; position community/post cards in a **grid** (e.g. row/col from index). No TreeLayout; custom placement. Still pan/zoom and overlay.
- **Option C — Hybrid**: TreeLayout for **communities → posts** (two levels); replies only in overlay or on post click (no reply nodes on canvas to keep size bounded).

---

## 3. Proposed graph model (recommended: tree, pruned)

- **Root** — Single node e.g. "Nostr" or "Communities".
- **Level 1 — Communities** — One node per discovered community (slug, count). Top N communities (e.g. 12–20) by count.
- **Level 2 — Posts** — For each community, top M posts (e.g. 3–5). Node: post id, content snippet, author, time, reply count.
- **Level 3 (optional) — Replies** — For each post, top K replies (e.g. 2–3) or none for MVP. Node: same shape as post, smaller.

Each node needs a **stable id** and **label** for FlowNode: e.g. community node id = `community:${slug}`, label = slug; post node id = `post:${event.id}`, label = content preview or title.

**Pruning** to keep the tree renderable:

- Cap communities (e.g. 15).
- Cap posts per community (e.g. 5).
- Cap replies per post (e.g. 3) or omit replies in MVP.

**Data loading**: useDiscoveredCommunities for level 1. For level 2, useCommunityPosts(slug) per community (could run in parallel for visible communities). For level 3, usePostReplies(eventId) or usePostRepliesThread when expanding a post (lazy) or fetch top replies for visible posts in batch if we have a batch API.

---

## 4. Layout and sizing

- **TreeLayout** expects one root and children; we have one root and many communities, then each community has many posts, then each post has many replies. So direction: e.g. root → **vertical** (communities stacked) and each community → **horizontal** (posts in a row) or **vertical** (posts stacked). Per-node `direction` in FlowNode is supported.
- **NODE_SIZES**: Define Nostr-specific types, e.g. `community`, `post`, `reply` with different widths/heights (e.g. community 160×40, post 200×56, reply 180×40). FlowNode today has root | leaf | skeleton; we can add `community` | `post` | `reply` or map them to generic types with different dimensions.
- **Connections**: Use default TreeConnectionLine (animated dashes). Optionally no connections for a “grid of cards” feel (would require a layout that doesn’t draw edges, or we still draw them for hierarchy).

---

## 5. Node types and rendering

- **CommunityNode** — Renders as card: slug (font-mono), count. Click → overlay with slug, count, link to /c/$community. Reuse flow node card style (rounded border, bg-card).
- **PostNode** — Renders as card: content preview (truncated), author npub (short), relative time, vote/reply counts. Click → overlay with summary + link to /posts/$id.
- **ReplyNode** — Same as post but smaller; optional in MVP.

Implementation options:

- **A)** Extend FlowNode metadata.type with `community` | `post` | `reply`; add NostrCommunityNode, NostrPostNode, NostrReplyNode in `flow/nodes/` or in a new `nostr-grid/` folder; `renderNode` dispatches by type.
- **B)** Keep FlowNode as-is; in the Nostr grid view, pass a custom `renderNode` that checks a **data** field (e.g. node.metadata.kind === 'community') and renders the right card. Node label could be slug or content preview; extra data (count, event, etc.) lives on the node object.

Recommendation: **B** for MVP (custom renderNode in the Nostr grid route); migrate to dedicated node components (A) if we want reuse and type safety.

---

## 6. Data → tree conversion

Add a **builder** that turns Nostr data into a FlowNode tree:

- **Input**: List of DiscoveredCommunity; optionally Map<slug, NostrEvent[]> (posts per community); optionally Map<eventId, NostrEvent[]> (replies per post).
- **Output**: Single FlowNode root with children = community nodes; each community node’s children = post nodes; each post node’s children = reply nodes (if any).

Ids: `community:${slug}`, `post:${event.id}`, `reply:${event.id}`. Labels: slug, content slice, content slice. Store on node: for post/reply, store event (or id + content + pubkey + created_at) for overlay and link.

Handle loading: show skeleton nodes (we have SkeletonNode) while useCommunityPosts loads; or show tree with only communities first, then add post children when data arrives (two-phase render).

---

## 7. Route and integration

- **New route** e.g. **/feed/grid** or **/nostr-grid** (or **/c/grid** for “all communities on a grid”).
- Page content: Wrap in **NostrProvider** and **RelayConfigProvider** and **QueryClientProvider** (same as NostrFeedSection). Render InfiniteCanvas → TreeLayout with tree from §6 and custom renderNode (§5). Overlay: NodeDetailsPanel (or a NostrNodeDetailsPanel) showing community slug + count + link, or post summary + link; optional ProjectDetails/LiveIndicator.
- **State**: selectedNode (FlowNode | null). When node is a community, overlay shows community details + link to /c/$community. When node is a post, overlay shows post details + link to /posts/$id.
- **Navigation**: From sidebar or feed page, add a “Grid” or “Map” link to this route so users can switch from list to grid view.

---

## 8. Phased implementation

### Phase 1 — Communities on a grid (MVP)

- New route `/feed/grid` (or `/nostr-grid`).
- Fetch communities (useDiscoveredCommunities). Build tree: root + community nodes only (no posts yet). Use TreeLayout with direction vertical (or horizontal) so communities appear in a row/column. Each node: id `community:${slug}`, label = slug; store slug and count on node (e.g. in metadata or a data field).
- renderNode: if node is community, render card with slug + count. onNodeClick: setSelectedNode. Overlay: show slug, count, link to /c/$community.
- Reuse InfiniteCanvas, TreeLayout, TreeElementNode, NodeDetailsPanel (or minimal custom panel). No new flow types if we use generic FlowNode with metadata or a single “community” type.

### Phase 2 — Add posts under each community

- For each community node, fetch top posts (useCommunityPosts(slug), limit 5). Build second level: community children = post nodes. Post node: id `post:${event.id}`, label = content preview; store event or summary.
- renderNode: dispatch by node id prefix (community: vs post:) or metadata; render community card or post card. Sizing: set NODE_SIZES or dimensions for “post” (e.g. wider than community). Connections: default TreeConnectionLine from community to each post.
- Overlay: if post selected, show summary + link to /posts/$id. Lazy load: optionally load posts only when community is expanded or when in view (simplest: load all top communities’ posts on mount).

### Phase 3 — Replies (optional)

- For each post node, optionally fetch top replies (usePostReplies(eventId), limit 3). Add third level: post children = reply nodes. Reply node: id `reply:${event.id}`, label = content preview.
- renderNode: reply card (smaller). Overlay: reply summary + link to /posts/$rootId (thread view). Prune aggressively (e.g. 2 replies per post) to keep node count low.

### Phase 4 — Polish

- Time filter (since 24h / 7d / 30d) that re-fetches communities/posts and rebuilds tree.
- AI filter (showAll) toggle.
- Loading states: skeleton nodes while posts load; or spinner on community node until posts loaded.
- Empty state: no communities → message + link to feed.
- Analytics: track grid_view, node_click (community vs post), overlay_open.

---

## 9. File and folder structure (suggested)

- **Route**: `routes/_app/feed.grid.tsx` or `routes/_app/nostr-grid.tsx` — page that composes Nostr providers + InfiniteCanvas + TreeLayout + overlay.
- **Tree builder**: `lib/nostrGridTree.ts` or `components/nostr-grid/buildNostrTree.ts` — function `buildNostrTree(communities, postsByCommunity?, repliesByPost?) => FlowNode`.
- **Node rendering**: Inline in the route with a `renderNostrNode(node)` that switches on node id or metadata; or `components/nostr-grid/NostrCommunityNode.tsx`, `NostrPostNode.tsx`, `NostrReplyNode.tsx` if we want components.
- **Overlay**: Extend NodeDetailsPanel or add `NostrNodeDetailsPanel.tsx` that shows community (slug, count, link) or post (summary, link) based on selected node.
- **Hooks**: Reuse useDiscoveredCommunities, useCommunityPosts, usePostReplies. Optional: `useNostrGridTree(communityLimit, postsPerCommunity, repliesPerPost)` that returns a FlowNode tree and loading state by composing those hooks and buildNostrTree.

---

## 10. Summary

| Item | Choice |
|------|--------|
| **Canvas** | InfiniteCanvas (pan/zoom, grid, overlay). |
| **Layout** | TreeLayout with one root → communities → posts → (optional) replies. |
| **Data** | useDiscoveredCommunities + useCommunityPosts (+ usePostReplies for Phase 3). |
| **Tree shape** | buildNostrTree(communities, postsByCommunity, repliesByPost?) → FlowNode. |
| **Node types** | Community, post, reply (id prefix or metadata). |
| **Rendering** | Custom renderNode in route (MVP) or dedicated Nostr*Node components. |
| **Overlay** | NodeDetailsPanel or NostrNodeDetailsPanel (slug/count or post summary + links). |
| **Route** | /feed/grid or /nostr-grid; NostrProvider + RelayConfig + QueryClient. |

This gives a single doc that ties our existing Nostr data and list UI to the flow grid: same canvas and tree layout, with a Nostr-specific tree builder and node rendering, and a clear phased path from communities-only to posts to replies.
