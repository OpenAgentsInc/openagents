import type { ReactNode } from 'react'

import { InternalLink } from '@/components/internal-link'
import { PublicFooter } from '@/components/public-footer'
import { PublicHeader } from '@/components/public-header'
import {
  DOCS_URL,
  DOWNLOAD_URL,
  GITHUB_REPOSITORY_URL,
  MAC_RELEASE,
  PRODUCT_BOUNDARIES,
} from '@/lib/public-site'

const workroomSteps = [
  {
    title: 'Start where you left off',
    body: 'Recent sessions appear before transcripts hydrate, with stable identity across reloads and restarts.',
  },
  {
    title: 'Keep the turn legible',
    body: 'Conversation, plans, tools, changes, decisions, and blockers stay in one causal timeline.',
  },
  {
    title: 'Review the exact change',
    body: 'Repository status and diffs are bounded to the active work context and remain read-only.',
  },
  {
    title: 'Return without guessing',
    body: 'Interrupted work reconciles explicitly. Nothing silently reruns or substitutes another session.',
  },
] as const

const capabilityItems = [
  ['Stable thread identity', 'Sessions remain the same sessions across navigation, reloads, and desktop restarts.'],
  ['Live turn state', 'Streaming, waiting, blocked, complete, and interrupted states stay visible while work happens.'],
  ['Tool evidence', 'Commands, file reads, approvals, and results appear where they belong in the conversation.'],
  ['Plans and steering', 'See the current plan, redirect the active turn, and understand what the agent will do next.'],
  ['Bounded change review', 'Inspect repository status and exact diffs without turning the review surface into a second editor.'],
  ['Explicit recovery', 'Interrupted work reconciles into an honest state instead of silently replaying side effects.'],
  ['Workspace context', 'The active repository, branch, and working directory remain attached to the work they explain.'],
  ['Local-first operation', 'Your ordinary Codex login stays authoritative. OpenAgents does not require another account.'],
] as const

const operatingFacts = [
  ['Local', 'Codex remains the runtime authority'],
  ['Live', 'Turn state stays visible as it changes'],
  ['Read-only', 'Repository review never mutates the tree'],
  ['Explicit', 'Recovery never guesses or silently reruns'],
] as const

const questions = [
  ['Does OpenAgents replace Codex?', 'No. Codex remains the engine and source of truth. OpenAgents Desktop adds a durable workroom around the session you already use.'],
  ['Do I need an OpenAgents account?', 'Not for the Desktop MVP. It uses your ordinary logged-in Codex session and keeps the core workroom local-first.'],
  ['Can the review UI change my files?', 'No. Repository status and diff views are deliberately read-only. Changes still happen through the active agent turn, where the cause and result remain visible.'],
  ['What happens after a restart or interrupted turn?', 'OpenAgents restores stable session identity, then reconciles the latest known turn state. It does not silently replay tools or pretend interrupted work completed.'],
  ['What is available today?', `The current ${MAC_RELEASE.version} release candidate is available for Apple silicon Macs. The product is still an MVP, so the download and docs describe the supported boundary precisely.`],
] as const

export function PublicSiteShell({ children }: { children: ReactNode }) {
  return (
    <div className="oa-public-site">
      <PublicHeader />
      <main>{children}</main>
      <PublicFooter />
    </div>
  )
}

export function DesktopLandingPage() {
  return (
    <PublicSiteShell>
      <section className="oa-hero">
        <div className="oa-container oa-hero-inner">
          <div className="oa-hero-copy">
            <p className="oa-kicker">Codex, made durable.</p>
            <h1>A serious place<br />for serious agent work.</h1>
            <p className="oa-hero-summary">OpenAgents Desktop is a local-first workroom around your ordinary Codex session—built to find work, follow the turn, review changes, and resume without losing the thread.</p>
            <div className="oa-actions"><InternalLink className="oa-button oa-button-primary" href={DOWNLOAD_URL} preload="render">Download for Mac</InternalLink><InternalLink className="oa-button oa-button-secondary" href={DOCS_URL} preload="render">Read the docs</InternalLink></div>
            <p className="oa-release-note">{MAC_RELEASE.version} · {MAC_RELEASE.architecture} release candidate</p>
          </div>
          <div className="oa-hero-signal" aria-hidden="true"><span>session.open</span><span>turn.streaming</span><span>change.reviewable</span><span>restart.reconciled</span></div>
        </div>
      </section>

      <section className="oa-preview-section" aria-labelledby="oa-preview-title">
        <div className="oa-container">
          <div className="oa-section-heading"><div><h2 id="oa-preview-title">Conversation first.</h2><p>The active turn stays central. Navigation and review stay close, but quiet.</p></div><span>Desktop preview</span></div>
          <figure className="oa-workbench" data-replace-with-screenshot>
            <div className="oa-window-bar"><div className="oa-window-dots" aria-hidden="true"><i /><i /><i /></div><span>OpenAgents</span><span className="oa-window-state"><i /> Codex connected</span></div>
            <div className="oa-workbench-body">
              <aside className="oa-session-rail">
                <button className="oa-new-session" type="button">＋ New chat</button>
                <div className="oa-session-group"><span>Today</span><div className="oa-session active"><strong>Unified public website</strong><small>Streaming · now</small></div><div className="oa-session"><strong>Release acceptance</strong><small>Complete · 42m</small></div><div className="oa-session"><strong>Composer recovery</strong><small>Blocked · 2h</small></div></div>
              </aside>
              <div className="oa-conversation">
                <header className="oa-conversation-header"><div><strong>Unified public website</strong><span>openagents · main</span></div><button type="button">Review changes <b>4</b></button></header>
                <div className="oa-timeline">
                  <div className="oa-message oa-user-message">Bring the public website into TanStack Start and keep the existing routes intact.</div>
                  <div className="oa-message oa-agent-message"><span>Codex</span><p>I’m consolidating the public pages, curated documentation, and authenticated application into one Start build.</p></div>
                  <div className="oa-tool-row"><span className="oa-tool-state" /><strong>Working</strong><span>apps/openagents.com/apps/start</span><time>00:38</time></div>
                  <div className="oa-change-row"><span>＋</span><div><strong>Public routes unified</strong><small>Start pages · Cloud Run mount · route tests</small></div><button type="button">Open review</button></div>
                </div>
                <div className="oa-composer"><span>Steer the current turn…</span><div><span>openagents / main</span><button type="button" aria-label="Stop current turn">■</button></div></div>
              </div>
            </div>
            <figcaption>Product frame for layout validation. Replace this figure with the current Desktop screenshot before homepage cutover.</figcaption>
          </figure>
        </div>
      </section>

      <section className="oa-capabilities" aria-labelledby="oa-capabilities-title">
        <div className="oa-container">
          <div className="oa-centered-heading"><p>Built for the whole agent loop</p><h2 id="oa-capabilities-title">Everything around the turn.<br />Nothing between you and Codex.</h2></div>
          <div className="oa-capability-grid">{capabilityItems.map(([title, body], index) => <article key={title}><span aria-hidden="true">{String(index + 1).padStart(2, '0')}</span><h3>{title}</h3><p>{body}</p></article>)}</div>
        </div>
      </section>

      <section className="oa-system-section" aria-labelledby="oa-system-title">
        <div className="oa-container oa-system-layout"><div className="oa-system-intro"><h2 id="oa-system-title">Codex stays the engine.<br />OpenAgents keeps the work legible.</h2><p>The workroom adds durable structure around the loop you already use. It does not replace your model account, invent another thread store, or turn review into hidden mutation.</p></div><ol className="oa-workroom-steps">{workroomSteps.map((step, index) => <li key={step.title}><span>{String(index + 1).padStart(2, '0')}</span><div><h3>{step.title}</h3><p>{step.body}</p></div></li>)}</ol></div>
      </section>

      <section className="oa-facts" aria-label="Operating principles"><div className="oa-container oa-facts-grid">{operatingFacts.map(([value, label]) => <div className="oa-fact" key={value}><strong>{value}</strong><p>{label}</p></div>)}</div></section>
      <section className="oa-boundaries" aria-label="Product boundaries"><div className="oa-container oa-boundary-grid">{PRODUCT_BOUNDARIES.map(boundary => <p key={boundary}><span aria-hidden="true">✓</span>{boundary}</p>)}</div></section>

      <section className="oa-faq" aria-labelledby="oa-faq-title">
        <div className="oa-container oa-faq-layout"><div className="oa-faq-intro"><p>Questions and answers</p><h2 id="oa-faq-title">The important boundaries, plainly.</h2><InternalLink href={DOCS_URL} preload="render">Read the full documentation <span aria-hidden="true">→</span></InternalLink></div><div className="oa-question-list">{questions.map(([question, answer], index) => <details key={question} open={index === 0}><summary>{question}<span aria-hidden="true">＋</span></summary><p>{answer}</p></details>)}</div></div>
      </section>

      <section className="oa-closing"><div className="oa-container oa-closing-layout"><div><p>Open source. Local first. Evidence backed.</p><h2>The work should survive the window.</h2></div><div className="oa-closing-actions"><InternalLink className="oa-button oa-button-primary" href={DOWNLOAD_URL} preload="render">Download for Mac</InternalLink><a className="oa-source-link" href={GITHUB_REPOSITORY_URL} target="_blank" rel="noreferrer">Explore the source <span aria-hidden="true">↗</span></a></div></div></section>
    </PublicSiteShell>
  )
}

export function DownloadPage() {
  return (
    <PublicSiteShell>
      <section className="oa-install-hero"><div className="oa-container oa-install-layout"><div><p className="oa-kicker">OpenAgents Desktop</p><h1>Bring your Codex work<br />into one durable place.</h1><p className="oa-install-summary">Download the latest OpenAgents Desktop candidate for Mac. It uses your ordinary Codex session and keeps your work local, legible, and ready to resume.</p><div className="oa-download-row"><a className="oa-download-button" href={MAC_RELEASE.downloadUrl} rel="noreferrer"><span>Download for Mac</span><small>{MAC_RELEASE.version} · {MAC_RELEASE.size}</small></a><p>{MAC_RELEASE.architecture}<br />macOS disk image</p></div><p className="oa-candidate-note">This is a release candidate for early use, not the stable release.</p></div><aside className="oa-release-panel" aria-label="Release details"><p className="oa-kicker">Current candidate</p><dl><div><dt>Version</dt><dd>{MAC_RELEASE.version}</dd></div><div><dt>Platform</dt><dd>{MAC_RELEASE.platform}</dd></div><div><dt>Chip</dt><dd>{MAC_RELEASE.architecture}</dd></div><div><dt>Package</dt><dd>.dmg</dd></div></dl><a href={MAC_RELEASE.releaseUrl} target="_blank" rel="noreferrer">View release notes <span aria-hidden="true">↗</span></a></aside></div></section>
      <section className="oa-install-steps"><div className="oa-container oa-install-steps-layout"><div><p className="oa-kicker">Install</p><h2>From download to workroom.</h2></div><ol>{[['Open the disk image', 'Double-click the downloaded DMG in Finder.'], ['Move OpenAgents to Applications', 'Drag the app into your Applications folder.'], ['Launch and start working', 'Open OpenAgents and continue with your existing Codex setup.']].map(([title, body], index) => <li key={title}><span>{String(index + 1).padStart(2, '0')}</span><div><h3>{title}</h3><p>{body}</p></div></li>)}</ol></div></section>
      <section className="oa-install-help"><div className="oa-container"><p>Need the setup details or product boundaries?</p><InternalLink href={DOCS_URL} preload="render">Read the docs <span aria-hidden="true">→</span></InternalLink></div></section>
    </PublicSiteShell>
  )
}

export function HoldingPage() {
  return <main className="oa-holding"><div className="oa-holding-bg" /><div className="oa-holding-copy"><h1>OpenAgents</h1><p>be right back</p></div></main>
}
