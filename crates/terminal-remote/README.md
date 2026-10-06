# Terminal Remote

`Remote` adapts the existing paired-host shell client to `terminal-core::pty::Transport`. Inject the platform's admitted `Links` supervisor with `Remote::injected`, or use an explicit paired-device store with `Remote::paired`. Native windows and Verse mount the same adapter through `Overlay::on_host`.

Verse accepts `--terminal-host HOST --terminal-store DIR --terminal-reference GENERATION/TERMINAL` to continue that same terminal. The standalone window accepts `--host HOST --paired-store DIR`, with `--terminal GENERATION/TERMINAL` to reattach an exact retained reference. Its control status reports the host generation, route, attachment status, input availability, terminal reference, and hosted command journal. Closing the mount detaches; closing a pane explicitly closes its terminal. A restarted host reports the old reference lost.

Output and authoritative snapshots follow the existing client. The bounded projection queue reconciles by snapshot after overflow. Offline input and proposal decisions are refused; uncertain acknowledgments are never retried automatically. Hosted journal directories are advisory proposal inputs, and the host still checks its actual OS directory and current shell context before admission.
