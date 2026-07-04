# Foldkit-Era UI Velocity Baseline

Date: 2026-07-04
Issue: [#8340](https://github.com/OpenAgentsInc/openagents/issues/8340)
Epic: [#8339](https://github.com/OpenAgentsInc/openagents/issues/8339)
Metric contract: [`docs/fable/2026-07-03-bf-7-2-locked-business-factory-metrics.md`](../fable/2026-07-03-bf-7-2-locked-business-factory-metrics.md)

This is the TS-10a baseline for the ONE-UI React/Tailwind transition. TS-10b
must rerun the same method against the React-era window and compare against
this artifact.

## Cutoff

The intended ordering was "baseline before the first React-surface merge."
The TanStack Start staging scaffold landed first as commit
`f6ba6e547e565da1e084e29d650a1621a61dd9c2`
(`2026-07-04T20:06:51Z`). To avoid contaminating the Foldkit-era baseline,
this report uses the parent commit
`1b9063bd30697c50adb466df566c1473cff7dbd3` as the measured tree and uses
`2026-07-04T20:06:51Z` as an exclusive upper bound.

No external Foldkit repository history, new Foldkit commits, or non-repo
Foldkit data were queried for this baseline. Inputs were limited to this
repository's historical Git graph and GitHub PR metadata already attached to
`OpenAgentsInc/openagents`.

## Scope

UI-surface work is defined as merged PRs that touched at least one file under:

- `apps/openagents.com/apps/web/`
- `clients/khala-code-desktop/`

Those are the pre-React web Foldkit app and Khala Code desktop UI surfaces
named by the epic. Direct `main` commits touching those paths are counted
separately because they do not carry PR create/merge timestamps and therefore
cannot produce a PR cycle-time measurement.

## Headline Rows

| Window | UI PRs | Web-path PRs | Desktop-path PRs | Direct/no-PR UI commits | Median cycle time | Average cycle time | P75 cycle time | Ledgered review-minutes per PR |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| trailing 30 days (`2026-06-04T20:06:51Z` to cutoff) | 131 | 60 | 72 | 586 | 12.4 min | 43.5 min | 39.0 min | `not_measured` |
| trailing 60 days (`2026-05-05T20:06:51Z` to cutoff) | 131 | 60 | 72 | 586 | 12.4 min | 43.5 min | 39.0 min | `not_measured` |

The 30-day and 60-day rows match because no resolvable merged PR in the
measured UI-surface set landed before `2026-06-18T22:19:46Z`. Older numeric
references in squashed commit subjects were issue refs or stale rewritten refs,
not resolvable PRs in `OpenAgentsInc/openagents`.

Cycle-time definition: `mergedAt - createdAt` in minutes for the resolved UI
PR set. For this baseline the measured sample has:

| Window | Count | Min | Median | P75 | Max |
| --- | ---: | ---: | ---: | ---: | ---: |
| trailing 30 days | 131 | 0.2 min | 12.4 min | 39.0 min | 338.1 min |
| trailing 60 days | 131 | 0.2 min | 12.4 min | 39.0 min | 338.1 min |

## Review Minutes

BF-7.2 defines `business_factory.review_minutes.v1` as ledgered human review
minutes from `omni_accepted_outcome_economics.review_minutes`. That ledger is
the audit receipt; GitHub review comments, issue comments, or elapsed wall time
are not review minutes.

For this Foldkit-era UI PR baseline, review minutes are:

| Window | Metric | Measurement state | Value | Caveat |
| --- | --- | --- | ---: | --- |
| trailing 30 days | `business_factory.review_minutes.v1` per UI PR | `not_measured` | n/a | No checked-in or public-safe accepted-outcome economics rows tie `review_minutes` to the UI PR refs at this cutoff. |
| trailing 60 days | `business_factory.review_minutes.v1` per UI PR | `not_measured` | n/a | Same caveat. |

GitHub review activity was inspected only as context, not as BF-7.2 review
minutes. The resolved UI PR set had 3 submitted GitHub reviews across 3 PRs;
first-review latency median was 11.7 minutes, P75 was 23.8 minutes, and average
was 16.0 minutes. Those values are not used in the headline metric because they
measure elapsed review latency, not ledgered human review effort.

## PR Set

Resolved UI PR numbers:

`5391`, `5473`, `5951`, `5989`, `6050`, `6069`, `6073`, `6074`, `6079`,
`6099`, `6102`, `6103`, `6121`, `6122`, `6133`, `6135`, `6136`, `6142`,
`6145`, `6147`, `6150`, `6151`, `6168`, `6170`, `6171`, `6172`, `6448`,
`6473`, `6487`, `6495`, `6511`, `6554`, `6725`, `6773`, `6792`, `6805`,
`6806`, `6807`, `6809`, `6962`, `6969`, `6996`, `7001`, `7004`, `7130`,
`7228`, `7233`, `7269`, `7543`, `7663`, `7674`, `7683`, `7751`, `7763`,
`7766`, `7770`, `7771`, `7819`, `7820`, `7909`, `7916`, `7918`, `7920`,
`7922`, `7928`, `7929`, `7930`, `7932`, `7933`, `7936`, `7937`, `7938`,
`7939`, `7941`, `7943`, `7944`, `7945`, `7946`, `7947`, `7948`, `7949`,
`7951`, `7954`, `7957`, `7958`, `7960`, `7962`, `7963`, `7964`, `7970`,
`7972`, `7974`, `7976`, `7977`, `7978`, `7979`, `7981`, `7984`, `7988`,
`7989`, `7992`, `8009`, `8123`, `8125`, `8128`, `8129`, `8130`, `8131`,
`8132`, `8133`, `8135`, `8136`, `8140`, `8143`, `8148`, `8149`, `8151`,
`8153`, `8158`, `8159`, `8166`, `8171`, `8215`, `8221`, `8236`, `8237`,
`8240`, `8241`, `8242`, `8243`, `8258`.

The first resolved UI PR in the set was
[#5391](https://github.com/OpenAgentsInc/openagents/pull/5391), merged
`2026-06-18T22:19:46Z`. The last was
[#8258](https://github.com/OpenAgentsInc/openagents/pull/8258), merged
`2026-07-03T11:34:30Z`.

## Reproduction Method

Run from the repository root with GitHub CLI authenticated for
`OpenAgentsInc/openagents`. The command intentionally inspects the parent of
the first React scaffold commit and excludes the scaffold commit itself.

```sh
CUTOFF_COMMIT=f6ba6e547e565da1e084e29d650a1621a61dd9c2
CUTOFF_PARENT=1b9063bd30697c50adb466df566c1473cff7dbd3
node <<'NODE'
const { execFileSync } = require('node:child_process')
const cutoff = new Date('2026-07-04T20:06:51Z')
const start30 = new Date(cutoff.getTime() - 30 * 24 * 60 * 60 * 1000)
const start60 = new Date(cutoff.getTime() - 60 * 24 * 60 * 60 * 1000)
const baselineRef = '1b9063bd30697c50adb466df566c1473cff7dbd3'
const paths = ['apps/openagents.com/apps/web', 'clients/khala-code-desktop']
const log = execFileSync('git', [
  'log',
  '--first-parent',
  `--since=${start60.toISOString()}`,
  `--until=${cutoff.toISOString()}`,
  '--format=%H%x09%cI%x09%s',
  baselineRef,
  '--',
  ...paths,
], { encoding: 'utf8', maxBuffer: 50 * 1024 * 1024 })

const entries = log.trim().split(/\n/).filter(Boolean).map(line => {
  const [hash, date, ...rest] = line.split('\t')
  const subject = rest.join('\t')
  const prs = [...subject.matchAll(/#(\d+)|origin\/pr\/(\d+)/g)]
    .map(match => Number(match[1] || match[2]))
  return { hash, date: new Date(date), subject, prs }
})

const prNumbers = [...new Set(entries.flatMap(entry => entry.prs))]
  .sort((a, b) => a - b)
const isUiFile = file =>
  file.startsWith('apps/openagents.com/apps/web/') ||
  file.startsWith('clients/khala-code-desktop/')

const prDetails = []
for (const number of prNumbers) {
  try {
    const json = execFileSync('gh', [
      'pr',
      'view',
      String(number),
      '--repo',
      'OpenAgentsInc/openagents',
      '--json',
      'number,title,createdAt,mergedAt,url,reviews,files',
    ], { encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 })
    const pr = JSON.parse(json)
    const files = (pr.files || []).map(file => file.path)
    if (files.some(isUiFile)) {
      prDetails.push({ ...pr, files: files.filter(isUiFile) })
    }
  } catch {
    // Numeric refs in historical squash subjects can be issue refs.
  }
}

const minutes = (a, b) => (new Date(b) - new Date(a)) / 60000
const quantile = (values, q) => {
  if (!values.length) return null
  const sorted = values.slice().sort((a, b) => a - b)
  const pos = (sorted.length - 1) * q
  const base = Math.floor(pos)
  const rest = pos - base
  return sorted[base + 1] === undefined
    ? sorted[base]
    : sorted[base] + rest * (sorted[base + 1] - sorted[base])
}
const round = value => value == null ? null : Math.round(value * 10) / 10

function summarize(start) {
  const prs = prDetails.filter(pr => {
    const merged = new Date(pr.mergedAt)
    return merged >= start && merged < cutoff
  })
  const allEntries = entries.filter(entry => entry.date >= start && entry.date < cutoff)
  const direct = allEntries.filter(entry => entry.prs.length === 0)
  const cycles = prs.map(pr => minutes(pr.createdAt, pr.mergedAt))
  return {
    start: start.toISOString(),
    end: cutoff.toISOString(),
    gitUiFirstParentCommitCount: allEntries.length,
    uiPrCount: prs.length,
    uiPrsWithWebPath: prs.filter(pr =>
      pr.files.some(file => file.startsWith('apps/openagents.com/apps/web/')),
    ).length,
    uiPrsWithDesktopPath: prs.filter(pr =>
      pr.files.some(file => file.startsWith('clients/khala-code-desktop/')),
    ).length,
    directOrNoPrUiCommitCount: direct.length,
    cycleMinutes: {
      count: cycles.length,
      average: round(cycles.reduce((total, value) => total + value, 0) / cycles.length),
      median: round(quantile(cycles, 0.5)),
      p75: round(quantile(cycles, 0.75)),
      min: round(Math.min(...cycles)),
      max: round(Math.max(...cycles)),
    },
  }
}

console.log(JSON.stringify({
  cutoff: cutoff.toISOString(),
  pathFilters: paths,
  windows: [summarize(start30), summarize(start60)],
}, null, 2))
NODE
```

## TS-10b Rerun Notes

TS-10b should keep the same path-filter shape but switch the UI prefixes to
the React-era surfaces it is measuring. If ledgered review-minute rows have
landed by then, TS-10b may replace the `not_measured` review-minute cells only
by joining the BF-7.2 economics receipt rows to the relevant PR refs. If that
join is still absent, TS-10b must keep the review-minute comparison
`not_measured` rather than filling in elapsed GitHub review latency.

The reusable extraction of the method is:

```sh
bun run perf:ui-velocity -- \
  --ref 1b9063bd30697c50adb466df566c1473cff7dbd3 \
  --cutoff 2026-07-04T20:06:51Z \
  --paths apps/openagents.com/apps/web,clients/khala-code-desktop \
  --window-days 30,60
```

On 2026-07-04 this command reproduced the headline TS-10a values exactly:
131 UI PRs, 60 web-path PRs, 72 desktop-path PRs, 586 direct/no-PR UI commits,
median cycle time 12.4 minutes, average 43.5 minutes, and P75 39.0 minutes.
