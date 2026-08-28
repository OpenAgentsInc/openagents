# Plugin A/B disposition

Machine-readable table: [`plugin-ab-disposition.json`](plugin-ab-disposition.json).
Issue [#120](https://openagents.com/OpenAgentsInc/openagents/issues/120).

Each row is one installed plugin × its oracle tasks × a status a metric
function can read:

| status | meaning |
| --- | --- |
| `delta` | WITH-plugin row beat WITHOUT on the named suite; ATIF shows the plugin ran |
| `no_delta` | both rows graded; score did not move; plugin may still have been invoked |
| `not_invoked` | WITH catalog was installed; ATIF has no `tool.ran` for this digest on its oracles |
| `no_oracle_yet` | no Terminal-Bench task is an honest oracle |
| `no_ab_yet` | A/B rows are not on the store yet |
| `abandoned` | a run could not finish; the blocker is in `notes` |

Recipe for scored rows: native CLI, `glm-5.3-flash`, proxy lane,
`https://openagents.com`. Suites `plugin-ab-git`, `plugin-ab-orient`,
and `plugin-ab-test` pin the same Harbor tasks as `tb2-cross-section`.

Update the JSON when a pair lands. Do not silently retain a plugin that
records `no_delta` with no rationale — flag it in `notes` for the review
loop.
