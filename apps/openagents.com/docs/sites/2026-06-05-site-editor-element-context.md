# Site Editor Element Context

Issue: `#169`

The Site editor now has a bounded element-context contract for targeted
follow-up comments. The first UI slice exposes safe target actions in the Site
editor sidebar and inserts the selected element reference into the feedback
composer.

The selected context contains:

- a bounded selector;
- a normalized tag;
- bounded visible text;
- allowlisted attributes only; and
- a bounded HTML snippet such as
  `<a class="button" href="#returns">Investment case</a>`.

Safety rules:

- Secret-shaped selectors, text, and attribute values are rejected or dropped.
- Unsafe attributes such as inline event handlers are not captured.
- `href` values are limited to hash, relative, `http://`, or `https://` URLs.
- The inserted composer text includes only the snippet and selector, not
  runner payloads, source archives, build logs, private run IDs, provider refs,
  or secrets.

Important bridge note:

The order page cannot inspect DOM inside `sites.openagents.com` directly when
it is cross-origin. The sanitized payload implemented here is the contract that
a future Site runtime preview bridge should send through `postMessage` when a
customer clicks an element in the live preview. Until that bridge is installed,
the sidebar exposes safe target actions that exercise the same insertion path.
