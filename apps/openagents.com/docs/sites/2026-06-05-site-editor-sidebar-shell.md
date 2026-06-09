# Site Editor Sidebar Shell

Issue: `#167`

The customer order page now has a first Site editor shell around the existing
revision and feedback surface. For Site orders, the page renders the current
revision loop beside a collapsible `Site editor` sidebar on desktop and stacks
the same surfaces on narrow screens.

This slice intentionally does not add history controls, targeted element
selection, or source viewing yet. It creates the layout contract those features
will attach to:

- `data-component="site-editor-shell"` marks the responsive editor region.
- `data-component="site-editor-sidebar"` marks the collapsible sidebar.
- `data-sidebar-width-px="336"` records the bounded first-width value for the
  editor sidebar.
- Native `<details open>` behavior provides the initial expanded/collapsed
  state without adding private session state or a fragile custom toggle.

Privacy boundary:

- The sidebar only shows public-safe Site/revision labels already available on
  the customer order page.
- It does not expose runner payloads, source archives, provider references,
  secrets, private paths, or generated code.

Follow-up issues in the same epic attach behavior to this shell:

- `#168` adds version and prompt history.
- `#169` adds element-targeted edit context.
- `#170` adds the read-only sidebar code viewer.
