import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'

// `openagents.com/training/runs` (and the `$runId` detail alias at
// `openagents.com/training/runs/{runId}`) — the training-runs/gym feature is
// deprecated-for-now (owner decision, 2026-07-05): the feature is intended to
// be restored later, not a current priority, so rather than delete the
// migrated route or the real `TrainingRunsPage` component (kept dormant in
// `-training-runs-page.tsx` for restoration), both routes render this honest
// "temporarily unavailable" notice instead of the real idle-state UI. The
// legacy Foldkit page (`apps/web/src/page/loggedOut/page/trainingRuns.ts`)
// and its route registration are untouched — only the nav links that
// surfaced it on the live product surface were removed. See
// `docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md` for the
// full deprecation note.
export function TrainingRunsDeprecatedPage() {
  return (
    <main
      className="min-h-dvh bg-black text-khala-text"
      data-route="training-runs-deprecated"
    >
      <div className="mx-auto grid min-h-dvh w-full max-w-2xl content-center gap-3 px-6 py-10 text-center">
        <Card className="grid gap-3 p-6 text-left sm:p-8">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <p className="m-0 font-mono text-[0.72rem] font-semibold uppercase leading-none text-khala-text-faint">
              Training Runs
            </p>
            <Badge>Temporarily unavailable</Badge>
          </div>
          <h1 className="m-0 text-2xl font-semibold leading-tight text-khala-text">
            This page is temporarily unavailable
          </h1>
          <p className="m-0 text-sm/6 text-khala-text-muted">
            Public CS336 training run state, verification, and settlement
            projection are deprecated for now while this feature is reworked.
            It is not gone — the code stays in place and this page will
            return.
          </p>
        </Card>
      </div>
    </main>
  )
}
