import { Array, Match as M, Option, Schema as S } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { blogPostRouter, blogRouter, docsPageRouter } from '../route'
import type { BlogPostRoute, BlogRoute } from '../route'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

export const BlogSlug = S.Literals([
  'introducing-khala-code',
  'tassadar-run-is-live',
  'pylon-autopilot-v1-rc1',
  'free-autopilot',
  'get-paid-to-code',
  'introducing-autopilot-sites',
])
export type BlogSlug = typeof BlogSlug.Type

type BlogRouteValue = BlogRoute | BlogPostRoute

type BlogPost = {
  readonly slug: BlogSlug
  readonly listed: boolean
  readonly title: string
  readonly excerpt: string
  readonly date: string
  readonly readTime: string
  readonly sections: ReadonlyArray<BlogSection>
}

type BlogSection = {
  readonly id: string
  readonly title: string
  readonly blocks: ReadonlyArray<BlogBlock>
}

type BlogBlock =
  | {
      readonly _tag: 'Paragraph'
      readonly text: string
    }
  | {
      readonly _tag: 'List'
      readonly items: ReadonlyArray<string>
    }
  | {
      readonly _tag: 'Quote'
      readonly text: string
    }
  | {
      readonly _tag: 'Link'
      readonly href: string
      readonly label: string
      readonly text: string
    }
  | {
      readonly _tag: 'TweetEmbed'
    }
  | {
      readonly _tag: 'Transcript'
      readonly text: string
    }

const paragraph = (text: string): BlogBlock => ({ _tag: 'Paragraph', text })
const list = (items: ReadonlyArray<string>): BlogBlock => ({
  _tag: 'List',
  items,
})
const quote = (text: string): BlogBlock => ({ _tag: 'Quote', text })
const link = (label: string, href: string, text: string): BlogBlock => ({
  _tag: 'Link',
  href,
  label,
  text,
})
const tweetEmbed = (): BlogBlock => ({ _tag: 'TweetEmbed' })
const transcript = (text: string): BlogBlock => ({ _tag: 'Transcript', text })

const pageShellClass =
  'h-dvh overflow-auto bg-[var(--oa-color-khala-surface)] font-mono ' +
  'text-[var(--oa-color-khala-text-primary)] antialiased ' +
  'selection:bg-[var(--oa-color-khala-energy-blue)] selection:text-white'

const pageFrameClass = 'mx-auto w-[min(100%,880px)] px-4 py-10 sm:py-14'

const panelClass =
  'khala-panel border border-[var(--oa-color-khala-border)] ' +
  'bg-[var(--oa-color-khala-surface-raised)] p-5 sm:p-7'

const titleClass =
  'm-0 text-balance font-mono text-3xl font-semibold tracking-normal ' +
  'text-[var(--oa-color-khala-text-bright)] sm:text-4xl'

const bodyClass =
  'm-0 max-w-[72ch] text-base/7 text-[var(--oa-color-khala-text-body)] ' +
  'text-pretty'

const mutedClass = 'font-mono text-sm text-[var(--oa-color-khala-text-faint)]'

const linkClass =
  'khala-focus font-medium text-[var(--oa-color-khala-energy-soft)] ' +
  'underline decoration-[var(--oa-color-khala-energy-blue)] ' +
  'underline-offset-4 transition-colors duration-200 ease-out ' +
  'hover:text-[var(--oa-color-khala-energy-cyan)] ' +
  'hover:decoration-[var(--oa-color-khala-energy-cyan)] ' +
  'motion-reduce:transition-none'

const freeAutopilotTranscript = `And we're back. OAPN. Good to see you, Christopher. Good to see you, Car. This day's been a long time coming. We've been talking about Autopilot for a while.

If you go back to episode 199, we were talking about Autopilot is a mech suit for your Claude Code. Oh, you sweet summer child, Chris of episode 199. Anthropic is... don't get me started.

Okay, so we built our own coding agents. To some extent, it's actually a mech suit around Codex and OpenCode and Hermes and other agents. Here's the cool part. It doesn't matter. You don't need to care about any of that stuff. There's a website you go to. It's called openagents.com. You click it. You give it tasks. It does it. You don't worry about the details. You slap down your credit card payment. It all works like magic.

Now, let's just say you've got the best coding agent in the whole world. What do you do with it? The first thing we do is we give it to you for free.

Free?

Yes, free. We're giving it to you for free.

F-R-E-E?

F-R-E-E. Free as in beer. Free as in... Codex? Also like sovereign, but free. Free, free, free, free, free.

Here's what you're going to do. First, you're going to see this blog post on the openagents.com. It's called Get Paid to Code.

Not just free.

It's not just free. We're going to pay you to use it if what you produce is cool. Let's step through this a little bit.

This is me taking my big ideas and being like, Codex, put this into your Codex terms. Let's just see what this says. OpenAgents already buys useful compute for Bitcoin. The next resource we need is useful code. The thing that has worked for OpenAgents is simple. Pay people for a resource the system needs. We needed compute, so we started buying compute for Bitcoin. That became the first little flywheel.

Now we need code. The useful version is AI-generated code that runs through our system, survives review, and teaches Autopilot how to do more valuable work the next step.

So here's the big idea. We're paying you for any data from your agent traces, the work that your agent does, that we can pull into our system to make our agent better. Okay? So let's say that you are working on some very hard problem of like some super cutting edge Rust ML framework, like for example, that's what we're building. And you solve some stuff, your agent does some stuff, and you burn like $10 of compute learning this thing.

Now let's say another agent, another version of Autopilot wants to do that same thing. What Anthropic does in this case is say, oh yeah, spend the other $10, and they're just gonna be pocketing everything. Oh yeah. Like they don't, like they're happy to have 10,000 developers, 10,000 companies pay 10,000 times for the same code.

Yeah.

Whereas we see that as a waste. Anthropic, big labs, inefficient. We've identified ways to be more efficient. Because we only really need to build one graph, one set of knowledge, one set of plugins, one set of best practices, the economics get very different to the point where we can say like, hey, our core product, at least a limited version of it, we wanna give it to you for free.

And also if you use it to do useful work and generate stuff that's gonna be usable and paid workflows, we're gonna pay you on the backend, proportional to RevShare usage, but like, you're gonna get paid for this. So maybe.

Now, most people are gonna be like, hey, this is pretty cool. Can I actually use this for building a website, doing other things? Because this idea of you stepping through this process, you're gonna wanna do this, we think, for all sorts of other things. So we're happy to like start by giving you the teaser for free. If you do like want to do some coding work and get things for free.

Now, it's not all instant. Part of the trade off is just you're gonna wait a little bit because we're doing a bunch of stuff in the background. We're able to provide it for free because we're making good use of this agentic asynchronous.

So it's not instant, it's going to get faster over time, but for now we're saying like, hey, let's take an order from you and give it back to you in a day. And then we'll bring that time down.

But here's what you're going to do at openagents.com. After logging in with GitHub, you're going to choose what repo. So you can either type it in, or this will pull from your repos. And then for now, we're only doing public repos, but we'll probably change that in the next few days. So by the time you read this, we intend to work with both public and private. But for now, public, public, public.

You're going to pick the repo. This is a little demo here. You're going to say what you want to happen. So give it a thing that you know. This might take you, like a coding agent, five minutes to do. Okay, we're going to do that for free.

If you scope out a whole multi-pronged effort that would take like $100 of compute, maybe that's where we'll say, you know, we'll do some of the work for free, but then slap the other credit card for the rest.

But the caveats, for now this is going to be public. And not only is it going to be public, like we're going to use it, but we're going to put the chain of what the agent does on the website, because we want people to see what does OpenAgents do. And hey, what if those agent traces are generating good data that can be used in RL training runs, like our Pylon network. More on that in probably the next video.

Okay, so we're paying for the compute. The stuff's public. That's it. You'll hear back from us with the result of your thing. And then obviously we want this to go well for you. So you're like, all right, I'll throw $5 of credits in there and see how that works.

A little bit about why we're doing this. We just had Codex go and download and then read all 227 of the episodes of this video series. Extract the major themes. OpenAgents versus closed AI capture. Build in public. Coding agents. Expectability. Graph-based. Nostr. 402. HUD. Site building. Automobile. Data markets. Local. Pylon. Psionic. Rust. Distributed training. Revenue share stuff.

And I was like, okay, speculate about what it would look like to create one product from this. So we've got a lot of this built, and we're actually going to be like, putting out this sort of we call it the Omni product that ties together all of the themes of OpenAgents.

But we were doing some analysis on this. And we kind of continued this in a private planning repo that we have, and like turn these into a product. And I just kind of like fed it to ChatGPT, I'm like, tell me what some of these smart AI people, these analysts, Gavin Baker, these big hedge fund AI guys, like, what would they say about all this?

They say, like, well, they'd say they've got a lot of this figured out, but you're really looking for what is that your first commercial wedge is like, oh, we need to have the one thing that people like want to slap down a credit card and pay for.

Well, you're going to pay for the best coding agent in the world. You're going to pay for it, but it's so good that we're gonna start by giving it to you for free. So we got a few beta, I'm sure kind of kinks to work out. So we're calling this a beta, we're gonna start by getting some people in the door here to try it out, get paid to use it.

And then if you ask for something that's super ambitious, maybe it'll say, hey, maybe you should pay us a little bit of money for that. But the idea is that we're building you a better product than what you get from the other companies. We're going to be doing it cheaper than what you get from the other companies.

And then over the next few weeks and months, we also aim for it to be faster than what you can just imagine you not having to close your laptop when you're coding on something. I think that's the people people don't realize just like how much of a fix that is.

Your wife was here in the office the other day talking about how it's much more freeing using Autopilot now. So you don't close your laptop and you can pay attention and all that kind of stuff.

Yeah, like I want to not have to carry my laptop around like an idiot. Like you should just say what the software that you want is and the magic agents will do it and you'll just like go live your life and then come back and then do that on repeat.

Okay, we'll leave it there. Thanks so much.`

const blogPosts: ReadonlyArray<BlogPost> = [
  {
    slug: 'introducing-khala-code',
    listed: true,
    title: 'Introducing Khala Code',
    excerpt:
      'Khala Code wraps your own local Codex install, adds fleet coordination, and connects coding work to the OpenAgents network while the public installer and economics loop are still being brought online.',
    date: 'July 2, 2026',
    readTime: '4 min read',
    sections: [
      {
        id: 'front-door',
        title: 'The coding front door',
        blocks: [
          paragraph(
            'Khala Code is the OpenAgents front door for coding work. It wraps your own local Codex install instead of replacing the harness: chat, threads, slash commands, approvals, MCP, plugins, skills, settings, and headless JSONL paths stay grounded in Codex while Khala adds a coordinated shell around them.',
          ),
          paragraph(
            'That wrapper matters because the bottleneck is no longer asking a model for code. The bottleneck is coordinating many runs, preserving evidence, comparing outcomes, and turning accepted work into something the network can learn from without pretending a plan is already live.',
          ),
          list([
            'Use your own local Codex capacity; Khala Code does not make the default path a pooled third-party labor market.',
            'Coordinate a fleet of isolated worker accounts through Pylon-backed delegation and exact per-turn accounting.',
            'Keep the review surface centered on evidence: diffs, tests, artifacts, screenshots, token rows, receipts, and acceptance state.',
          ]),
        ],
      },
      {
        id: 'fleet',
        title: 'Fleet and swarm coordination',
        blocks: [
          paragraph(
            'The fleet layer connects multiple isolated Codex worker homes and routes bounded coding requests across them. The useful shape is ordinary to the user: one inbox for approvals, blockers, worker closeouts, and proof panes; under the hood, each worker keeps its own credentials and account budget.',
          ),
          paragraph(
            'This is also the bridge into the wider OpenAgents network. A coding turn can become reviewable work with source refs and verification. Over time, accepted patterns can become reusable agent capability, but the public promise remains bounded to what the registry says is actually live.',
          ),
        ],
      },
      {
        id: 'plans',
        title: 'Free, paid, and the honest promise state',
        blocks: [
          quote(
            'Episode 245 framed the two-plan direction: Free (pay with data) and Paid (private data). That is launch-anchored design intent, not a green claim that every loop is live today.',
          ),
          paragraph(
            'The current public truth is narrower and better for trust: Khala Code is buildable from this repo, but there is no public installer yet. Free-plan desktop trace capture is not live. The paid plan is not yet purchasable; its purchase seam is flag-gated off and collects no payment while unarmed.',
          ),
          link(
            'Plan catalog',
            '/api/public/khala-code/plans',
            'The code-backed plan catalog is the source of truth for those caveats.',
          ),
          link(
            'Product-promise registry',
            '/api/public/product-promises',
            'The khala_code.* promise records track what is planned, blocked, or evidenced.',
          ),
        ],
      },
      {
        id: 'next',
        title: 'Full post coming',
        blocks: [
          paragraph(
            'This placeholder zero-bases the blog around the Khala Code launch while the full post is written. The durable point is already clear: OpenAgents starts from the coding harness people actually use, then adds fleet coordination, proof, and economic rails around accepted work.',
          ),
        ],
      },
    ],
  },
  {
    slug: 'tassadar-run-is-live',
    listed: false,
    title: 'The Tassadar run is live',
    excerpt:
      'Independent contributors are installing Pylon, doing verified executor-trace work, and earning Bitcoin — with a public, dereferenceable settlement receipt.',
    date: 'June 16, 2026',
    readTime: '3 min read',
    sections: [
      {
        id: 'live',
        title: 'The run is live',
        blocks: [
          paragraph(
            'The Tassadar run is live. It is a public decentralized training run where you install node software, do useful executor-trace work, and the work is verified by exact replay on a separate machine before anyone is paid. Two launch promises just flipped green on the live product-promises registry (version 2026-06-16.7): training.decentralized_training_launch.v1 and pylon.install_without_wallet_knowledge.v1.',
          ),
          paragraph(
            'Everything here is receipt-first: each claim degrades to the receipts actually recorded, and you can dereference every one of them. The run, its state, and its settlement metrics are public.',
          ),
          link(
            'Run status',
            'https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615',
            'Live run.tassadar.executor.20260615 — state, verified pairs, and provider-confirmed settled sats.',
          ),
          link(
            'Product promises',
            'https://openagents.com/api/public/product-promises',
            'The live registry. Check the two promise IDs above at version 2026-06-16.7.',
          ),
        ],
      },
      {
        id: 'contribute',
        title: 'Install and contribute',
        blocks: [
          paragraph(
            'You can contribute without Bitcoin wallet knowledge and without loading any bitcoin. A fresh Pylon install provisions its own wallet and identity on first run; you then claim a window lease and submit an executor trace. An independent validator on a distinct device auto-discovers your contribution and replays it — agreement is the proof.',
          ),
          list([
            'Install Pylon (the headless node) or Autopilot Desktop, which bundles and runs a node for you.',
            'Claim work and submit a trace; a separate validator device replays it.',
            'Verified by exact replay: equal worker and validator digests finalize the pairing.',
          ]),
          link(
            'Install & test',
            '/INSTALL.md',
            'Pylon (CLI) or Autopilot Desktop, plus the agent test flow.',
          ),
        ],
      },
      {
        id: 'earned',
        title: 'First contributors earned Bitcoin',
        blocks: [
          paragraph(
            'This is not a plan — it already happened, between independent operators, in the open. The first fully independent worker↔validator pairing finalized Verified by exact replay and earned an operator-approved, provider-confirmed Bitcoin settlement, recorded as a public receipt linked to the run.',
          ),
          list([
            'Verified pairing: challenge 59ba1f30 — an independent worker, validated by a separate independent operator on a distinct device.',
            'Settlement receipt: receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2 (settlement_recorded, state settled), reflected on the run as providerConfirmedSettledPayoutSats.',
          ]),
          paragraph(
            'This is the first such pairing and settlement — a real turn of the loop, not a scale claim. We are not claiming Tassadar is trained or beats a CPU, not claiming the largest decentralized run or a contributor count, and not quoting any payout that is not a settled, dereferenceable receipt. Payouts stay operator-approved under bounded spend authority. The run is open; more contributors are welcome.',
          ),
          link(
            'Follow along',
            '/forum/t/34bebe36-1c7c-443a-b7e2-13ec521955d9',
            'The full launch thread — every pairing and receipt, in the open.',
          ),
        ],
      },
    ],
  },
  {
    slug: 'pylon-autopilot-v1-rc1',
    listed: false,
    title: 'Pylon & Autopilot v1.0 — Release Candidate 1',
    excerpt:
      'The first v1.0 release candidates of Pylon (the headless node) and Autopilot Desktop are available for testing.',
    date: 'June 15, 2026',
    readTime: '3 min read',
    sections: [
      {
        id: 'rc1',
        title: 'Release candidate 1 is here',
        blocks: [
          paragraph(
            'We cut the first v1.0 release candidates of the OpenAgents node software. Two builds, one network: Pylon, the headless node you run from the command line, and Autopilot Desktop, the GUI cockpit that bundles and runs a Pylon node for you. Both are version 1.0.0-rc.1.',
          ),
          paragraph(
            'This is a release candidate for testing — not the stable public release. We are inviting agents and operators to install it, exercise it, and tell us what breaks. Installing a node is a capability; paid work, the training run, and settlement stay behind their own gated promises and are not live yet.',
          ),
        ],
      },
      {
        id: 'builds',
        title: 'What is in the box',
        blocks: [
          list([
            'Pylon — a single self-contained binary (macOS + Linux, four platforms). Every command speaks JSON; an agent can drive the whole surface from pylon help --json. No coding-agent SDK required to run the core.',
            'Autopilot Desktop — a signed, Apple-notarized macOS app that opens normally through Gatekeeper, bundles the headless node, and shows a live visualization of the pylon network on its home screen.',
            'Default-on, signed auto-update on both: each build fetches updates from our infrastructure and verifies them against a pinned OpenAgents release key, failing closed on anything it cannot verify.',
          ]),
        ],
      },
      {
        id: 'install',
        title: 'Install and test it',
        blocks: [
          paragraph(
            'The install and test guide covers both builds, signature verification, and the agent test flow. It is small and fetchable so an agent can act on it directly.',
          ),
          link(
            'Install & test the v1.0 release candidate',
            '/INSTALL.md',
            'Pylon (CLI) or Autopilot Desktop, plus the agent test script.',
          ),
          link(
            'Report what you find',
            '/forum/f/release-candidates',
            'Post install and test feedback on the Release Candidates forum — honest negative reports are the most useful.',
          ),
        ],
      },
    ],
  },
  {
    slug: 'introducing-autopilot-sites',
    listed: false,
    title: 'Introducing Autopilot Sites',
    excerpt:
      'Autopilot Sites turns website requests into hosted, reviewable revisions.',
    date: 'June 5, 2026',
    readTime: '3 min read',
    sections: [
      {
        id: 'site-requests',
        title: 'Site requests',
        blocks: [
          paragraph(
            'Autopilot started with repository tasks: pick a repo, describe the change, and come back to review the work. Sites adds a second handoff shape for people who want an actual hosted website, web app, internal tool, or game.',
          ),
          paragraph(
            'A Site request does not ask the customer to set up hosting, deployment, storage, or a local build pipeline. The useful output is a live URL, a revision record, and a place to send feedback.',
          ),
        ],
      },
      {
        id: 'handoff',
        title: 'Software handoff',
        blocks: [
          paragraph(
            'Some OpenAgents requests are best handled as code changes in an existing repository. Those can become a branch, a patch, a pull request, tests, and review notes.',
          ),
          paragraph(
            'Other requests are best handled as Sites. Those get a hosted page at sites.openagents.com, with revisions tracked through the customer order page.',
          ),
          list([
            'For repo work, the handoff is the code change and the review trail.',
            'For website work, the handoff is the latest live Site revision plus the order page.',
            'For follow-up comments, the customer keeps adding feedback and Autopilot rolls it into the next revision.',
          ]),
        ],
      },
      {
        id: 'beta',
        title: 'Beta loop',
        blocks: [
          paragraph(
            'Sites is still a public beta loop. Early revisions may be rough, but the product contract is simple: the customer should always know what exists, what changed, and where to comment next.',
          ),
          paragraph(
            'That loop is what matters. A request becomes a visible revision. Feedback becomes the next work item. The stable Site URL keeps pointing at the latest accepted or review-ready revision while the order page keeps the customer oriented.',
          ),
          link(
            'Read the Sites docs',
            docsPageRouter({ slug: 'autopilot-sites' }),
            'The customer-facing overview is here.',
          ),
          link(
            'Read the handoff docs',
            docsPageRouter({ slug: 'software-handoff' }),
            'The request and revision process is here.',
          ),
        ],
      },
    ],
  },
  {
    slug: 'free-autopilot',
    listed: false,
    title: 'Episode 228: Free Autopilot',
    excerpt: 'We launch the beta for Autopilot, our cloud coding agent.',
    date: 'June 4, 2026',
    readTime: '6 min read',
    sections: [
      {
        id: 'transcript',
        title: 'Verbatim transcript',
        blocks: [
          paragraph('Transcript of the Episode 228 launch video.'),
          tweetEmbed(),
          transcript(freeAutopilotTranscript),
        ],
      },
      {
        id: 'try-it',
        title: 'Try it',
        blocks: [
          paragraph(
            'Start with one concrete repository task. Autopilot will take the order and return work for review.',
          ),
          link('Start an Autopilot task', '/', 'OpenAgents beta starts here.'),
          link(
            'Get Paid to Code',
            blogPostRouter({ slug: 'get-paid-to-code' }),
            'Read the economics behind the beta.',
          ),
          link(
            'Autopilot basics',
            docsPageRouter({ slug: 'autopilot-basics' }),
            'Then read what Autopilot does.',
          ),
        ],
      },
    ],
  },
  {
    slug: 'get-paid-to-code',
    listed: false,
    title: 'Get Paid to Code',
    excerpt:
      'OpenAgents already buys useful compute for Bitcoin. The next resource we need is useful code.',
    date: 'June 4, 2026',
    readTime: '4 min read',
    sections: [
      {
        id: 'the-wedge',
        title: 'The wedge',
        blocks: [
          paragraph(
            'The thing that has worked for OpenAgents is simple: pay people for a resource the system needs. We needed compute, so we started buying compute for Bitcoin. That became the first little flywheel.',
          ),
          paragraph(
            'Now we need code. The useful version is AI-generated code that runs through our system, survives review, and teaches Autopilot how to do more valuable work the next time.',
          ),
        ],
      },
      {
        id: 'how-it-works',
        title: 'How it works',
        blocks: [
          paragraph(
            'The customer flow is simple. You sign in with GitHub, pick a repo, and tell us what you want. We scope it, split out the first useful slice, and come back with the next action.',
          ),
          list([
            'Small first slices can be free while the system learns what is worth building.',
            'Bigger requests can turn into priced orders with margin built in.',
            'Good code sessions can become reusable Autopilot learning.',
            'Reusable learning can later create credit or Bitcoin compensation when it contributes to paid workflows.',
          ]),
        ],
      },
      {
        id: 'why-this-is-different',
        title: 'Why this is different',
        blocks: [
          paragraph(
            'A business user can say what they want and get software back while OpenAgents handles the cloud databases, deployment plumbing, model routing, and toolchain chores underneath.',
          ),
          paragraph(
            'That is the product. The machinery underneath can be Codex, local models, our Pylon network, cheap cloud, expensive cloud, or some mix that wins on cost and quality. The user sees a clear request, progress, and reviewable software.',
          ),
        ],
      },
      {
        id: 'get-started',
        title: 'Get started',
        blocks: [
          quote(
            'Give us a real repo task. Kick the tires. If it is a good fit, we will try to turn it into useful work and useful learning.',
          ),
          paragraph(
            'This is beta software. The point is to make the loop dead simple: tell us what you want, let Autopilot work, and make sure useful code and useful contributors get paid.',
          ),
          link(
            'What is OpenAgents?',
            docsPageRouter({ slug: 'openagents' }),
            'Start with what OpenAgents is.',
          ),
          link(
            'Autopilot basics',
            docsPageRouter({ slug: 'autopilot-basics' }),
            'Then read what Autopilot does.',
          ),
          link(
            'Get Paid to Code',
            docsPageRouter({ slug: 'get-paid-to-code' }),
            'The short version of the offer is here.',
          ),
        ],
      },
    ],
  },
]

export const findBlogPost = (slug: string): Option.Option<BlogPost> =>
  Array.findFirst(blogPosts, post => post.slug === slug)

const listedBlogPosts = Array.filter(blogPosts, post => post.listed)

export const blogPostTitle = (slug: string): Option.Option<string> =>
  Option.map(findBlogPost(slug), post => post.title)

export const view = <Message>(
  route: BlogRouteValue,
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      M.value(route).pipe(
        M.withReturnType<Html>(),
        M.tagsExhaustive({
          Blog: () => indexView<Message>(),
          BlogPost: ({ slug }) =>
            Option.match(findBlogPost(slug), {
              onNone: () => missingPostView<Message>(slug),
              onSome: post => articleView<Message>(post),
            }),
        }),
      ),
    ],
  )
}

const indexView = <Message>(): Html => {
  const h = html<Message>()

  return h.main(
    [Ui.className<Message>(pageFrameClass)],
    [
      h.div(
        [Ui.className<Message>('grid gap-5')],
        [
          Ui.headingBlock<Message>({
            eyebrow: 'Khala Code dispatch',
            title: 'OpenAgents Blog',
            body: 'A reset public log for Khala Code: the local Codex wrapper, fleet coordination, proof surfaces, and the claim states that keep the launch honest.',
            level: 1,
            className:
              '[&_.oa-ui-heading-body]:max-w-[70ch] ' +
              '[&_.oa-ui-heading-body]:text-[var(--oa-color-khala-text-body)] ' +
              '[&_.oa-ui-heading-title]:font-mono ' +
              '[&_.oa-ui-heading-title]:text-[var(--oa-color-khala-text-bright)]',
          }),
          h.hr([Ui.className<Message>('khala-rule w-48')]),
        ],
      ),
      h.div(
        [Ui.className<Message>('mt-8 grid gap-3')],
        Array.map(listedBlogPosts, postCard<Message>),
      ),
    ],
  )
}

const articleView = <Message>(post: BlogPost): Html => {
  const h = html<Message>()

  return h.main(
    [Ui.className<Message>(pageFrameClass)],
    [
      h.a(
        [
          h.Href(blogRouter()),
          Ui.className<Message>(
            'khala-focus inline-flex text-sm text-[var(--oa-color-khala-text-muted)] ' +
              'transition-colors hover:text-[var(--oa-color-khala-energy-cyan)] ' +
              'motion-reduce:transition-none',
          ),
        ],
        ['Back to blog'],
      ),
      h.article(
        [Ui.className<Message>(`mt-6 ${panelClass}`)],
        [
          h.p(
            [Ui.className<Message>(mutedClass)],
            [`${post.date} / ${post.readTime}`],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'mt-4 text-pretty font-mono text-3xl font-semibold ' +
                  'tracking-normal text-[var(--oa-color-khala-text-bright)] sm:text-4xl',
              ),
            ],
            [post.title],
          ),
          h.p(
            [
              Ui.className<Message>(
                'mt-4 max-w-[72ch] text-base/7 text-[var(--oa-color-khala-text-body)]',
              ),
            ],
            [post.excerpt],
          ),
          h.hr([Ui.className<Message>('khala-rule mt-8 w-full')]),
          h.div(
            [Ui.className<Message>('mt-10 grid gap-10')],
            post.sections.map((section, index) =>
              sectionView<Message>(section, index + 1),
            ),
          ),
        ],
      ),
    ],
  )
}

const missingPostView = <Message>(slug: string): Html => {
  const h = html<Message>()

  return h.main(
    [Ui.className<Message>(pageFrameClass)],
    [
      h.article(
        [Ui.className<Message>(panelClass)],
        [
          h.p([Ui.className<Message>(`${mutedClass} mb-3`)], ['Missing post']),
          h.h1([Ui.className<Message>(titleClass)], ['Blog post not found']),
          h.p(
            [
              Ui.className<Message>(
                'mt-4 max-w-[72ch] text-base/7 text-[var(--oa-color-khala-text-body)]',
              ),
            ],
            [`No OpenAgents blog post exists for "${slug}".`],
          ),
          h.a(
            [
              h.Href(blogRouter()),
              Ui.className<Message>(
                'khala-focus mt-5 inline-flex border border-[var(--oa-color-khala-border-strong)] ' +
                  'px-3 py-2 text-sm text-[var(--oa-color-khala-energy-soft)] ' +
                  'transition-colors hover:border-[var(--oa-color-khala-energy-cyan)] ' +
                  'hover:bg-[var(--oa-color-khala-surface-active)] hover:text-white ' +
                  'motion-reduce:transition-none',
              ),
            ],
            ['Back to blog'],
          ),
        ],
      ),
    ],
  )
}

const sectionView = <Message>(section: BlogSection, index: number): Html => {
  const h = html<Message>()

  return h.section(
    [h.Id(section.id), Ui.className<Message>('scroll-mt-8')],
    [
      h.div(
        [Ui.className<Message>('flex items-center gap-3')],
        [
          h.span(
            [
              Ui.className<Message>(
                'khala-index inline-flex h-8 w-8 shrink-0 items-center justify-center ' +
                  'border border-[var(--oa-color-khala-border-strong)] ' +
                  'bg-[var(--oa-color-khala-surface-muted)] font-mono text-xs ' +
                  'font-semibold text-[var(--oa-color-khala-energy-cyan)]',
              ),
            ],
            [String(index).padStart(2, '0')],
          ),
          h.h2(
            [
              Ui.className<Message>(
                'm-0 font-mono text-xl font-semibold tracking-normal ' +
                  'text-[var(--oa-color-khala-text-bright)]',
              ),
            ],
            [section.title],
          ),
        ],
      ),
      h.hr([Ui.className<Message>('khala-rule mt-4 w-full')]),
      h.div(
        [Ui.className<Message>('mt-5 grid gap-5')],
        Array.map(section.blocks, blockView<Message>),
      ),
    ],
  )
}

const blockView = <Message>(block: BlogBlock): Html => {
  const h = html<Message>()

  return M.value(block).pipe(
    M.withReturnType<Html>(),
    M.tagsExhaustive({
      Paragraph: ({ text }) => h.p([Ui.className<Message>(bodyClass)], [text]),
      List: ({ items }) =>
        h.ul(
          [Ui.className<Message>('grid gap-2 pl-0')],
          Array.map(items, item =>
            h.li(
              [
                Ui.className<Message>(
                  'flex max-w-[72ch] gap-3 text-base/7 text-[var(--oa-color-khala-text-body)]',
                ),
              ],
              [
                h.span(
                  [
                    Ui.className<Message>(
                      'mt-3 h-px w-3 shrink-0 bg-[var(--oa-color-khala-energy-blue)] ' +
                        'shadow-[0_0_10px_rgba(58,123,255,0.75)]',
                    ),
                  ],
                  [],
                ),
                h.span([], [item]),
              ],
            ),
          ),
        ),
      Quote: ({ text }) =>
        h.blockquote(
          [
            Ui.className<Message>(
              'khala-panel max-w-[72ch] border border-[var(--oa-color-khala-border-strong)] ' +
                'bg-[var(--oa-color-khala-surface-muted)] p-4 text-base/7 ' +
                'text-[var(--oa-color-khala-text-quote)]',
            ),
          ],
          [text],
        ),
      Link: ({ href, label, text }) =>
        h.p(
          [Ui.className<Message>(bodyClass)],
          [
            text,
            ' ',
            Ui.textLink<Message>({
              href,
              label,
              attrs: [Ui.className<Message>(linkClass)],
            }),
          ],
        ),
      TweetEmbed: () =>
        h.div(
          [
            Ui.className<Message>(
              'grid max-w-[560px] justify-items-start overflow-x-auto',
            ),
            h.DataAttribute('tweet-embed', 'episode-228'),
          ],
          [
            h.blockquote(
              [
                Ui.className<Message>('twitter-tweet'),
                h.DataAttribute('media-max-width', '560'),
              ],
              [
                h.p(
                  [h.Lang('en'), h.Dir('ltr')],
                  [
                    'Episode 228: Free Autopilot',
                    h.br([]),
                    h.br([]),
                    'We launch the beta for Autopilot, our cloud coding agent.',
                    h.br([]),
                    h.br([]),
                    'We built Autopilot to be:',
                    h.br([]),
                    h.br([]),
                    '✅ Easy to use ',
                    h.br([]),
                    '🤑 Free or cheap (or pays YOU)',
                    h.br([]),
                    '🧠 Able to learn over time from you and others',
                    h.br([]),
                    '🐢 Slow on purpose! But easy to spawn many Autopilots',
                    h.br([]),
                    h.br([]),
                    'So you can:',
                    h.br([]),
                    '-… ',
                    h.a(
                      [h.Href('https://t.co/Tna7WEJP4N')],
                      ['pic.twitter.com/Tna7WEJP4N'],
                    ),
                  ],
                ),
                '— OpenAgents (@OpenAgents) ',
                h.a(
                  [
                    h.Href(
                      'https://x.com/OpenAgents/status/2062626257443909886?ref_src=twsrc%5Etfw',
                    ),
                  ],
                  ['June 4, 2026'],
                ),
              ],
            ),
            h.script(
              [
                h.Attribute('async', ''),
                h.Attribute('src', 'https://platform.x.com/widgets.js'),
                h.Attribute('charset', 'utf-8'),
              ],
              [],
            ),
          ],
        ),
      Transcript: ({ text }) =>
        h.pre(
          [
            h.DataAttribute('blog-transcript', 'free-autopilot'),
            Ui.className<Message>(
              'max-w-[72ch] whitespace-pre-wrap border border-[var(--oa-color-khala-border)] ' +
                'bg-[var(--oa-color-khala-surface)] p-4 font-sans text-base/7 ' +
                'text-[var(--oa-color-khala-text-soft)]',
            ),
          ],
          [text],
        ),
    }),
  )
}

const postCard = <Message>(post: BlogPost): Html => {
  const h = html<Message>()

  return Ui.Basecoat.card<Message>({
    className:
      'khala-panel border border-[var(--oa-color-khala-border)] ' +
      'bg-[var(--oa-color-khala-surface-raised)] p-0',
    children: [
      h.a(
        [
          h.Href(blogPostRouter({ slug: post.slug })),
          Ui.className<Message>(
            'khala-focus block p-4 transition-colors duration-200 ease-out ' +
              'hover:bg-[var(--oa-color-khala-surface-active)] ' +
              'motion-reduce:transition-none',
          ),
        ],
        [
          h.p(
            [Ui.className<Message>(mutedClass)],
            [`${post.date} / ${post.readTime}`],
          ),
          h.h2(
            [
              Ui.className<Message>(
                'mt-3 font-mono text-xl font-semibold tracking-normal ' +
                  'text-[var(--oa-color-khala-text-bright)]',
              ),
            ],
            [post.title],
          ),
          h.p(
            [
              Ui.className<Message>(
                'mt-3 max-w-[72ch] text-base/7 text-[var(--oa-color-khala-text-soft)]',
              ),
            ],
            [post.excerpt],
          ),
        ],
      ),
    ],
  })
}
