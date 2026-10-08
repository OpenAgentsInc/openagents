# Rust Native web adapter

`render(&ValidatedView<I>)` returns escaped HTML. `CSS` supplies the generic
stylesheet. The renderer never serializes an application intent into markup,
loads a resource, opens a link, or executes a domain effect. Markdown links stay
inert. A registered `Surface` initially renders its accessible label; specialized
surface mounting remains application adapter work.

With the `browser` feature, `mount(&web_sys::Element, &ValidatedView<I>)` applies
a keyed DOM update and acknowledges `data-rn-instance` and `data-rn-revision` on
the mount root. It rejects older revisions and reused revisions with changed
content. Stable fields retain native identity, focus, selection, and local
masked drafts. Composer tokens start separate editing lifetimes.

For Fields, a new rendered value replaces only the previous acknowledged draft;
later typing remains intact, and an acknowledgment that matches the live value
preserves its selection. A fixture reset uses a new instance or disposes its
mount before remounting. Ordinary rerenders preserve masked drafts.
Existing scroll positions survive updates; transcripts follow at the bottom and retain
a visible row anchor while reading earlier content. `dispose` retires the DOM
and identity; the controller removes its own delegated listeners.
Retirement closes native dialogs to return focus and clears detached input
contents before a late callback can read them.

The renderer uses these event attributes:

| Element | Event contract |
| --- | --- |
| Button or Choice | `data-rn-action="activate"`; resolve the nearest `data-rn-node` with `ValidatedView::activate`. |
| Field | An input or textarea with `data-rn-action="change"`; collect its value and nearest node into `FieldChange`, then call `change_field`. Never inspect or log secret values. |
| Composer | An editable textarea with `data-rn-action="compose"` and a token; Send and choice buttons use `submit`, and Stop uses `activate`. |
| Dialog | A Close button uses `dismiss`; resolve the dialog node through `activate`. Capture native `cancel`, prevent its default, and resolve the same dismissal through the controller. |

Read event identity from the displayed mount. The controller defers input
changes during IME composition and processes the committed value once; it owns
the shared editor/mirror stamps and exact submitted-text acknowledgment. Native
input controls provide browser undo, selection, and clipboard behavior. Mounting
does not by itself establish an editor reconciliation or authorization policy.

V2 views remain unchanged. `View::new_v3` opts into Field, RichText, Choice, and
Dialog. Secret Field values must be empty in immutable views; only local input
and its owning controller hold the typed secret. RichText uses typed runs and a
TextRole; `Terminal` retains spaces without wrapping. Choice has an accessible
label, explicit selected state, and optional presentation children.

Transcript sources are process-local. Publish the rows in the rendering
process before rendering a view with a source; a server source ID cannot hydrate
a Wasm process. Missing sources refuse. Browser code highlighting remains plain
unless the application supplies typed highlighted runs. This adapter does not
claim desktop or phone support for new v3 elements.

The root can set `--rn-default-foreground`, `--rn-default-font`,
`--rn-mono-font`, and `--rn-dialog-background`. Node styles resolve to literal
numeric CSS properties; no application-provided CSS or HTML is accepted.
