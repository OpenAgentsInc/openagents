# Review disk cleanup

Use `openagents-disk-cleanup scan --age-hours 24 --plan FILE --json` for old
Cargo outputs and linked Git worktrees. Choose a new plan filename in the
session's scratch folder. Install the tool with
`scripts/dev/install-disk-janitor.sh` if it is missing.

Report the candidate sizes and paths, then the largest kept folders and their
reasons. Include uncovered target folders. Do not say anything was removed by
a scan. Models, results, package caches, and unrelated scratch are outside this
cleanup command.

When the person authorizes removal, apply the reviewed plan with
`openagents-disk-cleanup apply --plan FILE --receipt RECEIPT`. Use `--only`
with entry numbers when they approved a subset. Do not broaden the saved scope
or reduce its age. Read the receipt and scan again: changed or active entries
may have stayed. Stop once the remaining entries match the expected keep
reasons; do not force their removal.
