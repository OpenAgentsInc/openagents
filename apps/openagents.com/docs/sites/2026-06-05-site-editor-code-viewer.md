# Site Editor Code Viewer

Issue: `#170`

The Site editor sidebar now includes a read-only code viewer for selected
element source context. When a customer selects a safe element target, the code
viewer shows:

- a bounded source path such as `selected-element/a.html`;
- the current Site version reference;
- the detected language;
- the bounded read-only source snippet; and
- a copy affordance for the snippet.

This slice does not expose generated source archives or private build artifacts
to customers. It uses the selected element source context from issue `#169`.
Future generated-source export work can add file-backed source browsing once
there is a customer-safe source projection and export/clone-token policy.

Safety boundary:

- Secret-shaped path, version, or source values block the viewer context.
- Source snippets are length-bounded before rendering.
- The viewer is read-only and does not expose source archive keys, build logs,
  runner payloads, provider refs, private run IDs, or secrets.
