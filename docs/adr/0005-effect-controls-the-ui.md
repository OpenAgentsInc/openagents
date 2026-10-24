# 0005: Effect controls the UI

Status: Accepted in part (owner, 2026-10-24: "I do like the idea of using
Effect as a UI layer, like controlling the UI layer of whatever it ends up
being, TypeScript on the web or something else on mobile").

## Decision

Split every Aethyr interface into a **controller** and a **renderer**, joined
by a platform-neutral protocol:

- **Controller (Effect).** Effect 4 services and Effect Atom state own UI
  state, navigation, intents, loading and error states, retries, and the calls
  to OpenAgents services. The controller never touches pixels or DOM nodes.
- **Protocol.** The controller publishes a typed view tree (semantic nodes
  with stable IDs, a revision number, and design-token references, never raw
  colors) and receives typed intents (`select`, `submit`, `dismiss`, and
  so on, each with the node ID). It also reads the device profile
  ([0007](0007-capability-and-detail.md)). The schema is defined once, in
  Rust, and generated for TypeScript, so both sides check it.
- **Renderers.** Anything that can draw the view tree: React DOM with the
  design system ([0006](0006-one-design-system.md)) today, Aethyr's GPU UI
  later, and on mobile whatever the shell uses. A renderer sends intents back
  and never changes state on its own.
- **Domain authority stays in Rust.** Accounts, permissions, publication,
  payments, and world authority are Rust services. The controller calls them.
  It does not reimplement them.

**Web (decided).** The controller is TypeScript with Effect 4 and Effect Atom,
as `apps/verse-ui` already is. The Kiki web surfaces move to this split first.
The engine's frame loop stays independent of controller updates: the
controller never blocks a frame, and the renderer reads the latest revision.

## Not yet decided: mobile

Where the controller runs on mobile is an owner decision. The protocol keeps
both options open:

1. **The same Effect bundle in an embedded JavaScript engine** (recommended).
   One controller codebase for web and mobile. It runs interpreted (Hermes
   bytecode, QuickJS, or JavaScriptCore without JIT), which app stores allow.
   It costs a JavaScript runtime in the app and a TypeScript exception for the
   mobile app.
2. **A Rust controller on mobile.** No new runtime, and it matches today's
   rule that new phone logic is Rust. It means two controllers for the same
   screens.

Until the owner decides, mobile screens use the same protocol from their
existing Rust code, so either option can replace it without changing
renderers.
