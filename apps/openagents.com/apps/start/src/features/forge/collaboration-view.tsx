import { PublicHeader } from '@/components/public-header'
import type {
  ForgeCollaborationProjection,
  ForgeCollaborationRequest,
  ForgeCollaborationResult,
} from '@/features/forge/collaboration-read'
import {
  AlertTriangle,
  CircleDot,
  FileWarning,
  GitCompareArrows,
  ListChecks,
  MessageSquareText,
  ShieldCheck,
  UserRoundCheck,
  XCircle,
} from 'lucide-react'

import './repository-view.css'

type Props = Readonly<{
  request: ForgeCollaborationRequest
  result: ForgeCollaborationResult
}>

const time = (value: string): string => {
  const date = new Date(value)
  return Number.isNaN(date.valueOf()) ? value : date.toLocaleString()
}

const statusClass = (state: string): string =>
  `forge-collaboration-status forge-collaboration-status-${state}`

const sourceLabel = (source: {
  author: string
  kind: number
  freshness: string
  observedAt: string
}) =>
  `${source.author} · kind ${source.kind} · ${source.freshness} · ${time(source.observedAt)}`

function Source({
  source,
}: {
  source: {
    eventId: string
    author: string
    kind: number
    freshness: 'fresh' | 'stale' | 'unknown'
    observedAt: string
  }
}) {
  return (
    <small className="forge-collaboration-source" title={source.eventId}>
      {sourceLabel(source)}
    </small>
  )
}

function Empty({ text }: { text: string }) {
  return <p className="forge-collaboration-empty">{text}</p>
}

function ChangeInspector({
  projection,
}: {
  projection: ForgeCollaborationProjection
}) {
  const change = projection.change
  if (change === null)
    return (
      <Empty text="This change is not available from the owned Forge record." />
    )
  return (
    <section
      aria-labelledby="forge-change-title"
      className="forge-collaboration-panel"
    >
      <header>
        <div>
          <span className="forge-eyebrow">Change</span>
          <h1 id="forge-change-title">{change.title}</h1>
          <p>{change.changeRef}</p>
        </div>
        <span className={statusClass(change.state.state)}>
          {change.state.label}
        </span>
      </header>
      <dl className="forge-collaboration-facts">
        <div>
          <dt>Base</dt>
          <dd>
            <code>{change.base.value}</code>
            <Source source={change.base.sources[0]} />
          </dd>
        </div>
        <div>
          <dt>Head</dt>
          <dd>
            <code>{change.head.value}</code>
            <Source source={change.head.sources[0]} />
          </dd>
        </div>
        <div>
          <dt>Proposal</dt>
          <dd>
            {change.proposalDialect.replaceAll('_', ' ')}
            <Source source={change.state.source} />
          </dd>
        </div>
        <div>
          <dt>Objects</dt>
          <dd>
            {change.proposalResolution === 'resolved'
              ? 'Resolved'
              : change.proposalResolution === 'disagreement'
                ? 'Disagreement'
                : 'Unresolved'}
          </dd>
        </div>
      </dl>
      {change.proposalResolution !== 'resolved' ? (
        <div className="forge-collaboration-warning" role="status">
          <AlertTriangle aria-hidden="true" />
          <p>
            {change.proposalResolution === 'disagreement'
              ? 'Sources disagree about this proposal. It is not actionable.'
              : 'The proposal objects are not resolved. It is not actionable.'}
          </p>
        </div>
      ) : null}
      <div className="forge-collaboration-grid">
        <section aria-labelledby="forge-review-heading">
          <h2 id="forge-review-heading">
            <UserRoundCheck aria-hidden="true" /> Review
          </h2>
          {change.reviews.length === 0 ? (
            <Empty text="No review record is available." />
          ) : (
            <ul className="forge-collaboration-list">
              {change.reviews.map(review => (
                <li key={`${review.label}-${review.source.eventId}`}>
                  <span className={statusClass(review.state)}>
                    {review.label}
                  </span>
                  <p>{review.detail}</p>
                  <Source source={review.source} />
                </li>
              ))}
            </ul>
          )}
        </section>
        <section aria-labelledby="forge-check-heading">
          <h2 id="forge-check-heading">
            <ListChecks aria-hidden="true" /> Checks and receipts
          </h2>
          {change.checks.length === 0 ? (
            <Empty text="No check record is available." />
          ) : (
            <ul className="forge-collaboration-list">
              {change.checks.map(check => (
                <li key={check.checkRef}>
                  <span className={statusClass(check.state)}>{check.name}</span>
                  <p>
                    {check.receiptRef ?? 'No receipt reference'}
                    {check.completedAt === undefined
                      ? ''
                      : ` · ${time(check.completedAt)}`}
                  </p>
                  <Source source={check.source} />
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
      <section aria-labelledby="forge-conversation-heading">
        <h2 id="forge-conversation-heading">
          <MessageSquareText aria-hidden="true" /> Conversation
        </h2>
        {change.comments.length === 0 ? (
          <Empty text="No signed NIP-22 comments are available." />
        ) : (
          <ol className="forge-collaboration-comments">
            {change.comments.map(comment => (
              <li key={comment.commentRef}>
                <strong>{comment.author}</strong>
                <time dateTime={comment.createdAt}>
                  {time(comment.createdAt)}
                </time>
                <p>{comment.body}</p>
                <Source source={comment.source} />
              </li>
            ))}
          </ol>
        )}
      </section>
      <section aria-labelledby="forge-merge-heading">
        <h2 id="forge-merge-heading">
          <GitCompareArrows aria-hidden="true" /> Merge outcome
        </h2>
        {change.merge === null ? (
          <Empty text="No merge decision has been recorded." />
        ) : (
          <div className="forge-collaboration-merge">
            <span
              className={statusClass(
                change.merge.outcome === 'merged'
                  ? 'applied'
                  : change.merge.outcome === 'blocked'
                    ? 'blocked'
                    : 'open',
              )}
            >
              {change.merge.outcome}
            </span>
            <Source source={change.merge.source} />
            {change.merge.signedReceipt === undefined ? (
              <p>No signed merge receipt is available.</p>
            ) : (
              <p>
                <ShieldCheck aria-hidden="true" />{' '}
                {change.merge.signedReceipt.summary} ·{' '}
                <code>{change.merge.signedReceipt.receiptRef}</code>
              </p>
            )}
          </div>
        )}
      </section>
    </section>
  )
}

function WorkView({
  projection,
}: {
  projection: ForgeCollaborationProjection
}) {
  const work = projection.work
  if (work === null)
    return (
      <Empty text="This work record is not available from the owned Forge record." />
    )
  return (
    <section
      aria-labelledby="forge-work-title"
      className="forge-collaboration-panel"
    >
      <header>
        <div>
          <span className="forge-eyebrow">Work</span>
          <h1 id="forge-work-title">{work.title}</h1>
          <p>{work.workRef}</p>
        </div>
        <span className={statusClass(work.state.state)}>
          {work.state.label}
        </span>
      </header>
      <dl className="forge-collaboration-facts">
        <div>
          <dt>Objective</dt>
          <dd>
            {work.objective.value}
            <Source source={work.objective.sources[0]} />
          </dd>
        </div>
        <div>
          <dt>Actor</dt>
          <dd>
            {work.actor.value}
            <Source source={work.actor.sources[0]} />
          </dd>
        </div>
        <div>
          <dt>Target change</dt>
          <dd>{work.targetChangeRef ?? 'No target change'}</dd>
        </div>
      </dl>
      <section aria-labelledby="forge-blocker-heading">
        <h2 id="forge-blocker-heading">
          <FileWarning aria-hidden="true" /> Blockers
        </h2>
        {work.blockers.length === 0 ? (
          <Empty text="No blocker record is available." />
        ) : (
          <ul className="forge-collaboration-list">
            {work.blockers.map(blocker => (
              <li key={blocker.value}>
                <p>{blocker.value}</p>
                <Source source={blocker.sources[0]} />
              </li>
            ))}
          </ul>
        )}
      </section>
    </section>
  )
}

function AttentionQueue({
  projection,
}: {
  projection: ForgeCollaborationProjection
}) {
  return (
    <section
      aria-labelledby="forge-attention-title"
      className="forge-collaboration-panel"
    >
      <header>
        <div>
          <span className="forge-eyebrow">For me</span>
          <h1 id="forge-attention-title">Attention queue</h1>
          <p>Every unresolved item stays visible.</p>
        </div>
      </header>
      {projection.attention.length === 0 ? (
        <Empty text="No attention item is available." />
      ) : (
        <ul aria-label="Forge attention queue" className="forge-attention-list">
          {projection.attention.map(item => (
            <li key={item.attentionRef}>
              <div>
                <span
                  className={statusClass(
                    item.kind === 'check_failed' ||
                      item.kind === 'work_blocked' ||
                      item.kind === 'disagreement'
                      ? 'blocked'
                      : item.kind === 'check_stale'
                        ? 'stale'
                        : 'open',
                  )}
                >
                  {item.kind.replaceAll('_', ' ')}
                </span>
                <h2>{item.title}</h2>
                <p>{item.detail}</p>
                <code>{item.target}</code>
                {item.actorRequired === undefined ? null : (
                  <p className="forge-attention-actor">
                    <CircleDot aria-hidden="true" /> Decision required from{' '}
                    {item.actorRequired}
                  </p>
                )}
                <Source source={item.source} />
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function Failure({ result }: { result: ForgeCollaborationResult }) {
  if (result._tag !== 'failed') return null
  const login = result.failure._tag === 'authentication_required'
  return (
    <section className="forge-collaboration-failure" role="status">
      <XCircle aria-hidden="true" />
      <h1>{login ? 'Invitation required' : 'Forge record unavailable'}</h1>
      <p>{result.failure.detail}</p>
      {login ? <a href="/login">Log In</a> : null}
      <p>This page never uses a GitHub fallback.</p>
    </section>
  )
}

export function ForgeCollaborationPage({ request, result }: Props) {
  const projection = result._tag === 'loaded' ? result.projection : undefined
  return (
    <div className="forge-page">
      <PublicHeader position="static" />
      <main className="forge-shell forge-collaboration-shell">
        <a className="forge-skip-link" href="#forge-collaboration-main">
          Skip to Forge record
        </a>
        <nav aria-label="Forge navigation" className="forge-tabs">
          <a
            href={`/forge/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}`}
          >
            Code
          </a>
          <a
            aria-current={request.view === 'change' ? 'page' : undefined}
            href={`/forge/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}/changes/${encodeURIComponent(request.changeRef ?? '')}`}
          >
            Changes
          </a>
          <a
            aria-current={request.view === 'work' ? 'page' : undefined}
            href={`/forge/${encodeURIComponent(request.owner)}/${encodeURIComponent(request.repo)}/work/${encodeURIComponent(request.workRef ?? '')}`}
          >
            Work
          </a>
          <a
            aria-current={request.view === 'attention' ? 'page' : undefined}
            href="/forge/attention"
          >
            For me
          </a>
        </nav>
        <main id="forge-collaboration-main">
          {projection === undefined ? (
            <Failure result={result} />
          ) : request.view === 'change' ? (
            <ChangeInspector projection={projection} />
          ) : request.view === 'work' ? (
            <WorkView projection={projection} />
          ) : (
            <AttentionQueue projection={projection} />
          )}
        </main>
      </main>
    </div>
  )
}

export function ForgeCollaborationSkeleton() {
  return (
    <div className="forge-page" aria-busy="true">
      <PublicHeader position="static" />
      <main className="forge-shell forge-collaboration-shell">
        <div className="forge-skeleton forge-collaboration-skeleton" />
        <div className="forge-skeleton forge-collaboration-skeleton" />
        <div className="forge-skeleton forge-collaboration-skeleton" />
      </main>
    </div>
  )
}
