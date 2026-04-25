# OpenAgents GitBook — Deployment Guide

**Target domain:** `book.openagents.com`
**Source path:** `/gitbook` at the repo root of [OpenAgentsInc/openagents](https://github.com/OpenAgentsInc/openagents)
**Branch:** `gitbook/vegas-btc-2026`

This guide walks through everything required to get the book live at `book.openagents.com`. It is split into **three phases**:

1. **Phase A — You (user):** push the branch, open a PR, share with Chris
2. **Phase B — Chris:** review PR, merge, create GitBook space, connect GitHub, add custom domain
3. **Phase C — DNS & verification:** CNAME cutover, SSL issuance, go-live check

Total wall-clock estimate: **60–90 minutes**, most of it waiting on GitBook's build and DNS propagation.

---

## Prerequisites checklist

Before starting, confirm:

- [ ] You have write access to your fork `OV1-Kenobi/openagents` (or whichever fork you're working from)
- [ ] Chris has admin on `OpenAgentsInc/openagents` **and** on the `openagents.com` DNS zone
- [ ] Chris has (or will create) a [GitBook.com](https://www.gitbook.com/) workspace with billing sufficient for one public space
- [ ] The drafts folder at `/home/user/workspace/openagents-gitbook-drafts/gitbook/` is the version you want to ship

---

# Phase A — User actions

## A1. Copy the drafts into the repo

From your local clone of the fork:

```bash
# 1. Clone your fork if you haven't already
git clone https://github.com/OV1-Kenobi/openagents.git
cd openagents

# 2. Make sure you're up to date with upstream
git remote add upstream https://github.com/OpenAgentsInc/openagents.git 2>/dev/null || true
git fetch upstream
git checkout main
git merge upstream/main --ff-only

# 3. Create the branch
git checkout -b gitbook/vegas-btc-2026

# 4. Copy the drafts into /gitbook
mkdir -p gitbook
cp -R /path/to/openagents-gitbook-drafts/gitbook/. ./gitbook/

# 5. Verify the tree
ls gitbook/
# expected: README.md SUMMARY.md .gitbook.yaml assets/ developers/ investors/ shared/ users/
```

> **Replace** `/path/to/openagents-gitbook-drafts/gitbook/` with the actual path where you unzipped the review bundle.

## A2. Commit and push

```bash
git add gitbook/
git commit -m "docs(gitbook): Vegas BTC 2026 book — 3 pathway architecture

- Investor path: 10 chapters (complete)
- Developer path: landing + 4 stubs (coming soon)
- User path: landing + 6 stubs (coming soon)
- Shared: glossary, about, changelog
- Amboss-style: breadcrumbs, hint blocks, prev/next nav"

git push origin gitbook/vegas-btc-2026
```

## A3. Open the PR

Open a PR from your fork's branch into the upstream repo's `main`:

```bash
# Using GitHub CLI:
gh pr create \
  --repo OpenAgentsInc/openagents \
  --base main \
  --head OV1-Kenobi:gitbook/vegas-btc-2026 \
  --title "docs(gitbook): Vegas BTC 2026 investor book" \
  --body "Draft GitBook tree for book.openagents.com — 3 pathway architecture. Investor path complete, developer/user paths stubbed. See /gitbook/README.md for overview."
```

Or via the web: [github.com/OpenAgentsInc/openagents/compare/main...OV1-Kenobi:gitbook/vegas-btc-2026](https://github.com/OpenAgentsInc/openagents/compare/main...OV1-Kenobi:gitbook/vegas-btc-2026).

## A4. Smoke-test the live pieces

Before handing off, confirm these claims from the book are real:

```bash
# Pylon ships on npm
npx -y @openagentsinc/pylon@0.1.13 --help

# Current pinned commit exists upstream
git -C /tmp/oa-smoke clone --depth 50 https://github.com/OpenAgentsInc/openagents.git 2>/dev/null || true
git -C /tmp/oa-smoke show 8590d04a --stat | head -20
```

If any of these fail, flag it in the PR — book claims must stay honest.

## A5. Hand off to Chris

Send Chris:

- Link to the PR
- Link to this DEPLOYMENT_GUIDE.md
- Reminder of the target domain: `book.openagents.com`

Suggested message (copy/paste):

> Hey Chris — I've opened [PR #___] with the Vegas BTC 2026 investor book at `/gitbook`. Three reader pathways (investor/dev/user), investor path is complete, dev + user are stubbed. Deployment steps for you are in [DEPLOYMENT_GUIDE.md](./DEPLOYMENT_GUIDE.md) Phase B onward — mostly GitBook space creation + a CNAME on `book.openagents.com`. Happy to jump on a call to walk through it.

---

# Phase B — Chris actions

## B1. Review & merge the PR

1. Open the PR on GitHub
2. Review the `/gitbook` tree — focus on:
   - `/gitbook/README.md` (landing)
   - `/gitbook/investors/*.md` (the complete investor path)
   - `/gitbook/SUMMARY.md` (sidebar structure)
3. Request changes or approve
4. **Merge to `main`** (or keep on the feature branch if you want a soft launch — GitBook can point at either)

## B2. Create the GitBook space

1. Sign in at [app.gitbook.com](https://app.gitbook.com/)
2. In your OpenAgents organization, click **New → Space**
3. Name: `OpenAgents Book`
4. Visibility: **Public** (required for `book.openagents.com`)
5. Leave the default content empty — we're importing from GitHub

## B3. Connect the GitHub integration

In the new space:

1. **Integrations → GitHub Sync → Configure**
2. Authorize GitBook to access `OpenAgentsInc/openagents`
3. Settings:
   - **Repository:** `OpenAgentsInc/openagents`
   - **Branch:** `main` (or `gitbook/vegas-btc-2026` if you haven't merged yet)
   - **Project directory:** `/gitbook`
   - **Sync direction:** GitHub → GitBook (one-way; we edit in git, GitBook renders)
4. Click **Sync**

GitBook will read `/gitbook/.gitbook.yaml` and `/gitbook/SUMMARY.md` and build the tree. First sync takes 1–3 minutes.

## B4. Publish the space

1. Top-right → **Publish → Publish to the web**
2. Confirm content looks right at the temporary `*.gitbook.io/*` URL GitBook assigns
3. Check:
   - [ ] Landing page renders with 3 pathway cards
   - [ ] Sidebar shows Investors / Developers / Users / Reference sections
   - [ ] Investor chapters 1–10 all load
   - [ ] Hint blocks render as colored callouts
   - [ ] Images load from `/assets/images/hero-banner.png`

If any images 404, check that `/gitbook/assets/` was committed (git sometimes excludes via .gitignore).

## B5. Add the custom domain

1. In the space: **Settings → Domains → Add custom domain**
2. Enter: `book.openagents.com`
3. GitBook displays a **CNAME target** — usually `hosting.gitbook.io` or a subdomain like `custom.gitbook.io`. **Copy the exact value GitBook shows you** — it can differ by plan/region.

Leave this tab open. Proceed to Phase C.

---

# Phase C — DNS & verification

## C1. Add the CNAME record

In the `openagents.com` DNS provider (Cloudflare / Route 53 / Namecheap / etc.):

| Field | Value |
|---|---|
| Type | `CNAME` |
| Name / Host | `book` |
| Target / Value | *(paste the target GitBook showed you in B5)* |
| TTL | `300` (5 min) is fine; lower propagation is faster |
| Proxy (Cloudflare only) | **DNS-only** (grey cloud) — GitBook handles SSL |

Save.

## C2. Verify propagation

From any machine:

```bash
# Should return a CNAME pointing at gitbook.io infrastructure
dig +short book.openagents.com CNAME
# or
nslookup book.openagents.com
```

Propagation usually takes 1–10 minutes on a 300s TTL. Can take up to an hour on older TTLs.

## C3. Wait for SSL issuance

Back in GitBook's domain panel:

- Status will move: **Pending DNS → Verifying → Issuing SSL → Active**
- This is automatic; GitBook uses Let's Encrypt
- Typical time: 2–15 minutes after the CNAME resolves

## C4. Final go-live check

Open `https://book.openagents.com` in a fresh browser (or incognito to avoid cache). Verify:

- [ ] Loads over HTTPS with a valid cert
- [ ] Landing shows the 3 pathway cards
- [ ] Click through to Investor chapter 1 — renders correctly
- [ ] Sidebar navigation works on mobile
- [ ] `https://book.openagents.com/investors/01-why-openagents` resolves (pretty URLs)
- [ ] Legacy redirects work: `https://book.openagents.com/chapters/01-why-openagents` → investors path

---

# Ongoing maintenance

## Edit → Publish flow

Because GitBook is set to **one-way GitHub → GitBook sync**, every content change goes through git:

1. Edit files in `/gitbook/...` on a feature branch
2. Open PR, review, merge to `main`
3. GitBook auto-syncs (usually within 60 seconds of merge)
4. Changes appear at `book.openagents.com`

**Do not edit content in the GitBook web editor** unless you also re-enable two-way sync — otherwise edits will be overwritten on the next git sync.

## Adding a new page

1. Create the `.md` file in the correct pathway directory
2. Add its entry to `/gitbook/SUMMARY.md` so it appears in the sidebar
3. Add breadcrumb + "You will learn" + prev/next per the existing page template
4. PR → merge → auto-publish

## Promoting developer or user pathways from "coming soon"

Each stub page has a `{% hint style="warning" %}` block at the top flagging it as a placeholder. When content is ready:

1. Remove the warning hint
2. Fill in real content following the same page template
3. Update the pathway's `README.md` to remove the "coming soon" badge
4. PR → merge

---

# Rollback plan

If something breaks after merge:

```bash
# Revert the merge commit
git revert -m 1 <merge-sha>
git push origin main
```

GitBook re-syncs within ~60s and serves the previous version. DNS and custom domain remain intact — only the content reverts.

For a full takedown: in GitBook space settings, **Unpublish** the space. The custom domain will 503 until you re-publish or remove the CNAME.

---

# Troubleshooting

**Problem:** GitBook sync shows "No pages found"
**Fix:** Verify `/gitbook/.gitbook.yaml` exists at the configured project directory, and `SUMMARY.md` references files with correct relative paths.

**Problem:** `book.openagents.com` shows GitBook's 404 page
**Fix:** The space isn't published yet, or the CNAME points at the wrong target. Re-check B4 and C1.

**Problem:** SSL certificate error
**Fix:** Cloudflare proxy is on (orange cloud). Switch to DNS-only (grey cloud). GitBook handles its own SSL.

**Problem:** Images broken
**Fix:** Git LFS or `.gitignore` stripped them. Check `git log --stat gitbook/assets/` and re-add if missing.

**Problem:** Sidebar order looks wrong
**Fix:** Edit `/gitbook/SUMMARY.md` — GitBook uses the order in that file, not alphabetical.

---

# Contacts

- **Book content questions:** (user) — opens PRs on `OpenAgentsInc/openagents`
- **Infrastructure (GitBook, DNS, domain):** Chris
- **Emergencies during Vegas BTC 2026 event:** book direct-messages — go/no-go call is Chris's

---

*Last updated: 2026-04-24. This guide lives at `/DEPLOYMENT_GUIDE.md` in the review bundle and is not published to the book itself.*
