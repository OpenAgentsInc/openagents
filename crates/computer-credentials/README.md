# Computer credentials

Account- and computer-scoped credential authorization, fixed network profiles,
and the Google Secret Manager reader for the host-side credential proxy.
The broker checks current authority before and after provider access. Public
callers select registered connection IDs; they cannot provide cloud secret paths.

The initial network format accepts exact DNS names. Credential rules also bind
the destination port, path, HTTP methods, header, and source version. Native IPv6,
wildcard host selectors, and raw guest credentials require separate qualification.
