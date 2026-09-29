+++
type = "regex"
target = { file = "lib/index.js" }
+++

module\.exports\s*=\s*\{[^}]*\btruncate\b[^}]*\}
