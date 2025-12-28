# Autopilot Demo Gallery Deployment Guide

> **DEPRECATED (2025-12-28):** Demo gallery has been moved to ~/code/backroom.
> This document is kept for historical reference only.

## Overview

This guide covers deploying the autopilot demo gallery to openagents.com, completing d-027 (Autopilot Demo + Dogfooding Funnel).

## Current Status

✅ **Complete:**
- 10 well-scoped GitHub issues created (#1525-#1534)
- Demo generation workflow documented
- 287 sessions analyzed, top 5 selected
- Replay bundles created (121KB total)
- Gallery index with metadata
- Quality scoring system (85-91/100 range)

🚧 **In Progress:**
- Deployment infrastructure setup
- CDN configuration
- Domain routing

⛔ **Blocked:**
- GitHub → Local DB sync (for processing created issues)
- Daemon merge from agent/001 branch

## Demo Gallery Components

### 1. Replay Bundles (`demos/`)

5 curated demos packaged as `.replay.tar.gz` files:

```
demos/
├── index.json                              # Gallery catalog
├── 20251219-2138-start-working.replay.tar.gz    # 75KB, score: 91
├── 20251219-2122-start-working.replay.tar.gz    # 10KB, score: 91
├── 20251223-235126-process-issues-*.tar.gz      # 13KB, score: 85
├── 20251223-231615-process-issues-*.tar.gz      # 10KB, score: 85
└── 20251222-162040-call-issue-ready-*.tar.gz    # 13KB, score: 85
```

Each bundle contains:
- `session.rlog` - Complete session transcript
- `metadata.json` - Metrics, tools, tokens
- `changes.diff` - Git modifications
- `README.md` - Viewing instructions

### 2. Web Viewer

The existing demo viewer at `demo-viewer/` needs deployment:

```
demo-viewer/
├── index.html          # Viewer UI
├── player.js           # Replay engine
├── styles.css          # UI styling
└── [static assets]
```

**Features:**
- Plays .rlog files in browser
- Shows tool calls, outputs, agent messages
- Syntax highlighting for code
- Timestamp navigation

**Needed Enhancements** (from issue #1527):
- Play/pause button
- Playback speed controls (0.5x, 1x, 2x, 5x)
- Keyboard shortcuts
- Progress indicator
- Speed preference persistence

### 3. Gallery Landing Page

**Required:** Create `demos/gallery.html`

Should include:
- Hero section explaining autopilot
- Demo cards with:
  - Title and description
  - Quality score badge
  - Difficulty level
  - Key highlights
  - Download/view buttons
- Filtering by difficulty
- Search/sort functionality
- Stats dashboard

## Deployment Architecture

### Option A: Static Site (Recommended for MVP)

**Stack:**
- GitHub Pages or Netlify/Vercel
- Static HTML/CSS/JS
- No backend required

**Setup:**
```bash
# Build static site
cd demo-viewer
npm install
npm run build

# Deploy to hosting
netlify deploy --prod --dir=dist
# OR
vercel --prod
```

**Pros:**
- Zero infrastructure cost
- Fast CDN delivery
- Simple deployment
- HTTPS automatic

**Cons:**
- No server-side logic
- Bundle uploads must be manual
- No analytics backend

### Option B: Full Stack (Future)

**Stack:**
- Frontend: React/Next.js
- Backend: Rust (Axum/Actix)
- Database: PostgreSQL
- Storage: S3/Cloudflare R2
- CDN: Cloudflare

**Features:**
- User uploads
- Session analytics
- Rating system
- Comments
- Auto-refresh from CI

## Deployment Steps

### Phase 1: Static MVP (Immediate)

1. **Prepare Viewer**

```bash
cd demo-viewer

# Install dependencies
npm install

# Build for production
npm run build

# Test locally
npm run serve
# Visit http://localhost:8080
```

2. **Create Gallery Page**

```bash
# Generate gallery HTML from index.json
python3 scripts/generate_gallery.py > demos/gallery.html
```

3. **Deploy to Netlify**

```bash
# Install Netlify CLI
npm install -g netlify-cli

# Deploy
netlify deploy --prod --dir=demo-viewer/dist

# Configure custom domain
netlify domains:add demos.openagents.com
```

4. **Configure DNS**

Add CNAME record:
```
demos.openagents.com -> [netlify-site].netlify.app
```

5. **Verify**

```bash
curl https://demos.openagents.com/
curl https://demos.openagents.com/demos/index.json
```

### Phase 2: Gallery Page (Week 1)

1. Create gallery landing page
2. Add demo cards with download buttons
3. Implement filtering/search
4. Add stats dashboard
5. Link to viewer for each demo

### Phase 3: Enhanced Viewer (Week 2)

1. Implement issue #1527 (playback controls)
2. Add metrics sidebar (issue #1532)
3. Improve mobile responsiveness
4. Add share functionality
5. Implement deep linking (jump to timestamp)

### Phase 4: Analytics & Feedback (Week 3)

1. Add basic analytics (Plausible/Fathom)
2. Implement feedback form
3. Create "Request Demo" flow
4. Add email capture for interested users
5. Track popular demos/features

### Phase 5: Automation (Week 4)

1. CI integration for auto-deployment
2. Scheduled autopilot runs
3. Automatic bundle generation
4. Quality filtering in CI
5. Gallery auto-update

## Domain Strategy

### Recommended Structure

- `openagents.com` - Main site (marketing)
- `demos.openagents.com` - Demo gallery
- `app.openagents.com` - Actual app (future)
- `docs.openagents.com` - Documentation

### SEO Considerations

- Sitemap generation
- Meta tags for social sharing
- Structured data (JSON-LD)
- Robots.txt configuration
- Analytics integration

## Metrics & Success Criteria

### Launch Metrics

- [ ] Gallery loads < 2s (mobile)
- [ ] All demos playable without errors
- [ ] Mobile-responsive design
- [ ] 90+ Lighthouse score
- [ ] HTTPS enabled

### Engagement Metrics (Week 1)

- Unique visitors
- Demos viewed
- Average watch time
- Bounce rate
- Conversion to GitHub stars

### Growth Metrics (Month 1)

- Organic search traffic
- Social shares
- Backlinks
- Email signups
- Feature requests

## Post-Launch Tasks

### Content

- [ ] Write blog post announcing gallery
- [ ] Create Twitter thread with demo highlights
- [ ] Post to r/programming, r/rust, r/LocalLLaMA
- [ ] Submit to Hacker News
- [ ] Add to Product Hunt

### Monitoring

- [ ] Set up uptime monitoring (UptimeRobot)
- [ ] Configure error tracking (Sentry)
- [ ] Enable analytics (Plausible)
- [ ] Create status page

### Iteration

- [ ] Collect user feedback
- [ ] Identify most popular demos
- [ ] Add more demos based on feedback
- [ ] Improve viewer based on usage data

## Budget Estimate

### MVP (Static Site)

- Domain: $12/year (if not owned)
- Netlify: $0 (free tier)
- **Total: ~$0/month**

### Growth Phase

- Netlify Pro: $19/month (if needed)
- Analytics: $9/month (Plausible)
- CDN: $5/month (Cloudflare R2)
- **Total: ~$33/month**

### Full Stack (Future)

- Hosting: $20/month (VPS)
- Database: $15/month (managed PG)
- Storage: $5/month (S3/R2)
- CDN: $10/month
- **Total: ~$50/month**

## Rollback Plan

If deployment issues occur:

1. **DNS**: Revert CNAME to previous target
2. **Netlify**: Use rollback feature to previous deploy
3. **Static files**: Keep backup of working build
4. **Database**: Not applicable for MVP

## Security Considerations

- **HTTPS**: Automatic via Netlify
- **CSP**: Configure Content Security Policy
- **CORS**: Restrict if adding API later
- **Rate limiting**: Add if abuse detected
- **Input validation**: For feedback forms

## Next Steps

### Immediate (Today)

1. ✅ Create deployment documentation (this file)
2. Create gallery page generator script
3. Test demo viewer with all 5 bundles
4. Set up Netlify account

### This Week

1. Deploy MVP to demos.openagents.com
2. Verify all demos load correctly
3. Share with OpenAgents community
4. Collect initial feedback

### Next Week

1. Implement playback controls (issue #1527)
2. Add metrics display (issue #1532)
3. Process GitHub issues via local DB
4. Run autopilot on new issues
5. Add new demos to gallery

## Resources

- **Demo bundles**: `./demos/`
- **Viewer source**: `./demo-viewer/`
- **Selection script**: `./scripts/select_best_demos.py`
- **Bundling script**: `./scripts/bundle_demo.sh`
- **Gallery index**: `./demos/index.json`
- **GitHub issues**: https://github.com/OpenAgentsInc/openagents/issues/1525-1534

## Troubleshooting

### Viewer won't load .rlog files

- Check CORS headers
- Verify file is valid .rlog format
- Check browser console for errors
- Ensure file size < 10MB

### Demos won't play

- Clear browser cache
- Check JavaScript console
- Verify metadata.json is valid
- Test with minimal .rlog first

### Slow load times

- Enable gzip compression
- Optimize bundle sizes
- Use CDN for static assets
- Lazy load demo content

---

**Last Updated:** 2025-12-27
**Status:** Ready for MVP deployment
**Owner:** OpenAgents Team
**Next Review:** After first deployment
