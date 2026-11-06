# Unused quick-match wrapper (#12982)

The room-context mutation run kills all 33 named-hold policy mutants, but
its baseline initially exits 1 because `rm_quick` is not reached by any
check. Reference search finds only that procedure's definition. It has no
`#export_c` and no generated room C-header entry.

The actual join path already calls `rm_quick_profile` for both legacy and
authoritative quick matches. Removing the unused three-line wrapper keeps
that route and every existing function body. Both existing QUICK MATCH
checks pass, including bot physics. The fresh room-context mutation run
kills all 33 mutants and exits 0 without the coverage warning.

The generated room C header remains byte-identical across this cleanup and
the named-target fixture integration. The ordinary native room also builds
and reports zero fields off their declared defaults in `--start-check`.
