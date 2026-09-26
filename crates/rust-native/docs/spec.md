# View and adapter specification

Rust Native separates semantic presentation from application state and native
widget ownership. The implemented core validates and serializes views; it does
not mount them or provide an application runtime.

## View and interaction contract

`View<I>` uses schema `rust-native.view.v1`, a surface `instance`, a positive
`revision`, and a root `Node<I>`. Each node contains a stable `key`, a resolved
`Style`, and an element:

| Element | Meaning |
| --- | --- |
| `Stack` | Ordered children on a horizontal or vertical axis. |
| `List` | A labeled bounded window of stable rows. Paging and access to original data remain application responsibilities. |
| `Text` | Unicode text with a body, heading, code, status, or Markdown role. A Markdown role conveys selectable document meaning; links and embedded content remain inert unless separately admitted by the application. It does not itself parse or render Markdown. |
| `Button` | A nonempty visible label, an enabled state, and the application's typed intent. |

`ValidatedView<I>` exposes an immutable tree and its checked serialization and
activation paths. Unknown core fields and variants are rejected. The
application supplies a closed serializable intent type and validates domain
identifiers within it. Compiled application serialization code is trusted host
code, not sandboxed plugin code.

| Bound | Limit |
| --- | --- |
| Surface, node, and style identifiers | 1–96 ASCII bytes: letters, digits, `_`, `-`, `.`, and `:` |
| Encoded view | 512 KiB, including escaping and intent data |
| Nodes | 1,024 |
| Node depth | 16, counting the root as one |
| JSON nesting | 96 containers, including intent payloads |
| Text or control label | 64 KiB in UTF-8 bytes |

Input byte and nesting limits apply before decoding. Constructed trees also
pass structural and encoded limits. These bounds apply to one view, not an
application's backing data store. Use bounded windows for long content while
retaining access to every original record. The v1 schema refuses earlier
versions rather than silently converting them.

## Revision and lifetime

The application allocates a fresh instance when it mounts a new surface
lifetime and increases its revision for each new immutable view. It must never
reuse one revision for different contents. The core validates individual views;
it does not maintain a global revision ledger.

An `Activation` carries `instance`, `revision`, and `node`. Resolving it against
the current validated view rejects stale instance or revision values, missing
or noninteractive nodes, and disabled buttons. It returns the intent already
stored in that view, never a caller-supplied replacement action.

Resolution is not authentication, proof of a physical gesture, authorization,
or idempotency. The application checks the event source, current domain state,
and permission before performing any effect. Repeated activations can resolve
the same intent; deduplication belongs to the application's operation contract.
A concurrent refresh can cause a visible stale-view refusal.

## Adapter responsibilities

Native adapters and their mounting protocol remain separate implementation
work. Each adapter must specify its supported elements and style properties,
faithful fallbacks, and refusals. Validation of a tree does not prove that a
particular adapter supports it.

The intended boundary is:

1. Application state produces a validated immutable view.
2. The adapter checks support, prepares changes, and applies them on the
   platform's UI thread.
3. The adapter acknowledges the revision it actually applied. Receiving a
   revision is distinct from displaying it.
4. Native callbacks name the instance, applied revision, and stable node key.
5. The application resolves the intent and checks current authority.

Reconciliation should preserve native control identity, focus, scroll position,
and selection when keys remain stable. Detached or recycled controls must lose
their old callbacks and private contents. A late mount must not replace a newer
one. Disposal releases native resources and presentation subscriptions; it does
not implicitly cancel durable application work.

Use native controls for selection, scrolling, system menus, input composition,
and accessibility. A text editor requires an additional explicit contract for
edit acknowledgments, selection units, marked text, composition ownership, and
stale updates. Those semantics are not implemented by serializing a string.

The shared core owns neither network access, persistence, a palette, clocks,
credentials, nor task execution. Platform objects belong to adapters;
application-specific components and effects belong to their application.
