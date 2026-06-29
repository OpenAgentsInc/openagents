# Site Preview Element Bridge

Issue: `#171`

The Site editor now has a runtime bridge for live preview element targeting.
The order page installs an origin-scoped Foldkit mount on the Site editor shell
when a Site has an active URL. The mount listens for `message` events from that
Site origin only.

Accepted payload shape:

```json
{
  "type": "openagents.site.elementTarget",
  "selector": "main a[href=\"#returns\"]",
  "tag": "a",
  "text": "Investment case",
  "attributes": [
    { "name": "class", "value": "button" },
    { "name": "href", "value": "#returns" }
  ]
}
```

When the payload validates, it is converted through the same
`SiteElementContext` sanitizer used by the sidebar inspect actions and then
dispatches `SelectedCustomerSiteElementContext`. The existing composer
insertion, selected-context display, and code viewer paths handle the result.

Safety boundary:

- Messages from any origin other than the active Site origin are ignored.
- Wrong message types, missing selectors/tags, secret-shaped selectors, and
  unsafe attributes are rejected before dispatch.
- The bridge does not inspect cross-origin DOM itself; eligible Site runtimes
  must emit the payload.
- No runner payloads, source archives, build logs, private run IDs, provider
  refs, or secrets are accepted.

Runtime work still required for each generated Site that wants true click
capture:

1. Add an inspect-mode click listener inside the Site runtime or preview
   wrapper.
2. Build the bounded selector/tag/text/attribute payload locally in the Site.
3. Send it to the parent with `window.parent.postMessage(payload, parentOrigin)`.
4. Keep the Site-side payload generation aligned with the sanitizer contract
   above.
