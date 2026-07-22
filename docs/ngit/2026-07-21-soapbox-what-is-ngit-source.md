# Source capture — Soapbox: "What is Ngit? The BUZZy New Git Protocol Taking On GitHub"

**Captured:** 2026-07-21
**Source:** Soapbox blog, published July 21, 2026, byline M. K. Fain.
**Provenance note:** This file preserves the article body as external source
material for the `docs/ngit/` reference lane. It is untrusted third-party
prose, not OpenAgents authority, and it is retained for citation stability.
Website navigation, donation, and footer chrome are omitted. Image content is
represented by the article's own alt/caption text. Claims in this article are
checked against source code in
[`2026-07-21-ngit-analysis.md`](2026-07-21-ngit-analysis.md).

---

> Ngit lets you host repos, patches, and issues on Nostr instead of GitHub.
> Built by DanConwayDev, used by Shakespeare, and now shipping inside Block's
> Buzz.

Ngit is a tool that lets developers collaborate on code over Nostr instead of
GitHub. Your repos, pull requests, and issues become signed events on an open
protocol, hosted by servers anyone can run. Today it took its biggest step
yet: Block shipped Buzz, a team workspace with git hosting built on Nostr,
announced by Jack Dorsey.

## The Problem: Open Source Lives on a Closed Platform

Git itself is decentralized. Linus Torvalds designed it in 2005 so that every
contributor holds a full copy of the project history and no central server is
required. Then the world put nearly all of it in one place anyway.

GitHub hosts the overwhelming majority of open source development, and GitHub
is a closed-source product owned by Microsoft, which acquired it for $7.5
billion in 2018. The result is a strange arrangement: the free software
movement, built on the idea that code should not be controlled by any single
company, coordinates almost all of its work inside a single company's
proprietary platform.

That control gets exercised. A few examples:

- In 2020, GitHub took down youtube-dl, one of the most popular open source
  projects in the world, after a DMCA notice from the RIAA. It was later
  restored, but the takedown itself required nothing more than a letter.
- In 2019, GitHub restricted accounts of developers in Iran, Syria, and
  Crimea to comply with US sanctions, cutting people off from their own code
  because of where they live.
- Microsoft trained GitHub Copilot on the public code hosted on the platform,
  a move that triggered a class-action lawsuit over open source license
  violations.
- When GitHub goes down, and it has had repeated major outages, a large
  fraction of the world's software development stops with it.

None of this means GitHub is uniquely bad. It means it is a single point of
failure and a single point of control, and code is speech. If your money runs
on Bitcoin and your identity runs on Nostr but your code lives on a platform
that can delist it with a policy change, the most important layer of your
stack is the least sovereign one.

## What Ngit Is

Ngit is a CLI tool and git plugin built by DanConwayDev, developed in
partnership with Soapbox. It implements NIP-34, the open specification for
code collaboration on Nostr. It does not replace git. It replaces the
platform around git:

**Repos as signed events** — A repository announcement on Nostr declares the
project, its maintainers, and where to clone it. Signed by your key, not
granted by a platform.

**nostr:// clone URLs** — With the git-remote-nostr helper installed, you can
run `git clone nostr://npub.../repo` like any other URL.

**PRs and issues over relays** — Patches, pull requests, and issues are Nostr
events sent to the repo's relays. No account on anyone's platform required,
just your key.

**GRASP servers** — Relays that double as git servers host the actual repo
data. Your repo can list several at once, so no single host can take it
offline.

The workflow stays familiar. `ngit init` announces an existing repo to Nostr.
After that, `git push` and `git pull` work against the `nostr://` remote like
any other. Contributors clone the same URL and send patches without asking
permission from a hosting company. If a maintainer disappears or a host
delists the project, the repo announcement, the history, and the discussion
all still exist, replicated across relays and every contributor's machine.

For a web interface, DanConwayDev also built gitworkshop.dev, which describes
itself as "git collaboration, without the platform." It gives you the
browsing, issues, and PR review experience you expect from GitHub, on top of
NIP-34 events. The project is open source and supported by OpenSats.

*Image caption: A repo on gitworkshop.dev: file tree, issues, PRs, and the
GRASP servers and relays that host it.*

## First Major Use: Shakespeare

Ngit's first wave of real-world adoption came from Shakespeare, Soapbox's AI
app builder. Shakespeare runs git entirely in the browser and supports
`nostr://` URLs natively, so every project built there can be pushed to Nostr
git with a click. Thousands of vibe-coded projects became NIP-34 repos this
way, which gave the protocol something every new protocol needs: actual
repositories with actual users.

*Image caption: One click, a repo identifier, and the project is on Nostr.*

Support has since spread across the ecosystem. NostrHub, Soapbox's developer
platform, lets you create a repo in the browser and push it to a GRASP
server, or mirror an existing GitHub repo onto Nostr. Gitworkshop.dev covers
the full maintainer workflow. And because repos, issues, and patches are just
Nostr events, general-purpose clients can link to and render them too.

*Image caption: One profile, 55 repos on Nostr. Most of these started as
Shakespeare projects.*

## Now Shipping Inside Buzz

Today Block released Buzz, an open-source, self-hostable workspace where
teams of humans and AI agents work together in channels. Buzz is built on
Nostr: every person and every agent has a keypair, and every message,
approval, and commit is signed. It ships with git hosting and a forge UI,
with repositories stored on object storage and coordinated through Nostr
events.

*Image caption: Repositories in Buzz, hosted on Nostr instead of GitHub.*

*Image caption: A Buzz project: AI agents open pull requests and file
issues, humans make the call.*

Block's reasoning, from their engineering post, reads like the case for
Nostr git in general: "If Buzz disappears, your identity and signed history
still verify. Git remains Git and can be rehosted. It means the host does
not own your name or your work. A protocol anyone can rebuild is a protocol
nobody can lock you into."

There is also a practical driver: AI agents. Block notes that a team of
agents can produce "human-months of commits and CI runs in an afternoon,"
and centralized forges were designed around human rate limits. Signed events
and self-hosted git storage handle machine-scale collaboration in a way a
platform account model does not. The code and protocol specs are at
github.com/block/buzz, and yes, the irony of that URL is exactly why this
movement exists.

*Image caption: A pull request in Buzz's forge: conversation, commits,
reviewers, and merge, with every action signed by a Nostr key.*

## Try It in 10 Minutes

1. Install ngit from gitworkshop.dev/ngit. This includes the
   git-remote-nostr helper.
2. Run `ngit init` inside an existing repo to announce it on Nostr with your
   key.
3. Push and pull with normal git against your new `nostr://` remote, and
   browse the repo on gitworkshop.dev.

No terminal? Create a repo directly in the browser on NostrHub, or build
something in Shakespeare and push it to Nostr from there.

## Frequently Asked Questions

**What is ngit?** Ngit is a command-line tool and git remote helper that
lets you use Nostr for code collaboration. Repos are announced as signed
Nostr events, patches and issues travel over relays, and you can clone with
a `nostr://` URL. It was built by DanConwayDev and implements NIP-34, the
open specification for git on Nostr.

**Is ngit a replacement for git?** No. Ngit is a replacement for GitHub, not
git. You keep using git exactly as before. Ngit replaces the centralized
platform layer: hosting, pull requests, issues, and discovery all move to
Nostr, where they are signed by your key and served by relays no single
company controls.

**What is a GRASP server?** A GRASP server is a Nostr relay that also acts
as a git server. It stores your repository data and serves clones, while the
Nostr events that announce the repo let anyone find it and verify who
maintains it. If one GRASP server disappears, your repo announcement points
to others.

**What happens to my repo if a relay goes down?** Nothing is lost. Your repo
announcement lists multiple clone URLs and relays, every contributor has a
full copy of the history (that is how git works), and the events defining
issues and patches are replicated across relays. There is no single server
whose failure takes the project offline.

**Who uses ngit today?** Shakespeare pushes projects to Nostr git and was
the first major source of adoption. NostrHub and gitworkshop.dev provide web
interfaces for NIP-34 repos. And as of July 2026, Block's Buzz ships git
hosting built on Nostr for teams of humans and AI agents.

---

*Article ends. Related-reading links, newsletter, navigation, and donation
footer omitted from this capture.*
