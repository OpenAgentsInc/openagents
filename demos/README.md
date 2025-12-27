# OpenAgents Autopilot Demo Gallery

This directory contains the production-ready demo gallery showcasing OpenAgents Autopilot capabilities through real session replays.

## Contents

- **gallery.html** - Demo gallery landing page (529 lines)
- **index.json** - Demo catalog with metadata (5 curated demos)
- **5 replay bundles** - Packaged session replays (121KB total)

## Demo Quality Scores

All demos scored 85-91/100 based on:
- Successful completion
- Code quality
- Tool diversity
- Narrative clarity
- Real-world applicability

**Top 5 Demos:**
1. Multi-issue processing (91.0/100) - 21 files, multiple issues
2. Autonomous issue creation (91.0/100) - Shows initiative
3. Comprehensive refactoring (85.0/100) - 26 files, 8 commits
4. Systematic resolution (85.0/100) - Methodical approach
5. Massive multi-file (85.0/100) - 56 files, 10 commits

## Viewing Demos Locally

```bash
cd demos
python3 -m http.server 8000
# Open http://localhost:8000/gallery.html
```

## Bundle Format

Each `.replay.tar.gz` contains:
- `session.rlog` - Complete session transcript
- `metadata.json` - Metrics, tools used, token counts
- `changes.diff` - Git modifications made
- `README.md` - Instructions for viewing

## Deployment

### Quick Deploy to Netlify

```bash
# Install Netlify CLI
npm install -g netlify-cli

# Deploy from repo root (netlify.toml configured)
netlify deploy --prod

# Or connect GitHub repo for auto-deploys
netlify init
```

### Alternative: GitHub Pages

```bash
# Create gh-pages branch with demos/ content
git checkout --orphan gh-pages
git reset --hard
git checkout main -- demos/
mv demos/* .
rmdir demos
git add .
git commit -m "Deploy demo gallery"
git push origin gh-pages

# Configure in repo settings:
# Settings → Pages → Source: gh-pages branch
```

### Custom Domain (demos.openagents.com)

**Netlify:**
```bash
netlify domains:add demos.openagents.com
```

**DNS Configuration:**
```
Type: CNAME
Name: demos
Value: [your-netlify-site].netlify.app
```

## Adding New Demos

1. **Generate from session logs**
   ```bash
   cd scripts
   ./bundle_demo.sh /path/to/session.rlog demos/new-demo.replay.tar.gz
   ```

2. **Update index.json**
   - Add new demo entry with metadata
   - Update stats (total count, average score, etc.)

3. **Re-generate gallery**
   ```bash
   python3 scripts/generate_gallery.py > demos/gallery.html
   ```

4. **Deploy**
   ```bash
   git add demos/
   git commit -m "Add new demo: [description]"
   git push origin main
   ```

## File Structure

```
demos/
├── README.md                                          # This file
├── gallery.html                                       # Landing page
├── index.json                                         # Demo catalog
├── 20251219-2138-start-working.replay.tar.gz         # Demo 1 (75KB)
├── 20251219-2122-start-working.replay.tar.gz         # Demo 2 (10KB)
├── 20251223-235126-process-issues-*.replay.tar.gz    # Demo 3 (13KB)
├── 20251223-231615-process-issues-*.replay.tar.gz    # Demo 4 (10KB)
└── 20251222-162040-call-issue-ready-*.replay.tar.gz  # Demo 5 (13KB)
```

## Related Documentation

- **Status Report:** `docs/autopilot/D-027-STATUS.md`
- **Generation Workflow:** `docs/autopilot/DEMO_GENERATION.md`
- **Deployment Guide:** `docs/autopilot/DEPLOYMENT.md`
- **Selection Script:** `scripts/select_best_demos.py`
- **Bundling Script:** `scripts/bundle_demo.sh`
- **Gallery Generator:** `scripts/generate_gallery.py`

## Success Metrics

### Launch Criteria
- [ ] Gallery live at demos.openagents.com
- [ ] All 5 demos downloadable
- [ ] Page load < 2s on mobile
- [ ] 90+ Lighthouse score
- [ ] Zero console errors

### Week 1 Targets
- 1000+ unique visitors
- 50+ demo downloads
- 10+ GitHub stars (attribution)

## Security

All bundles have been manually reviewed for:
- ✅ No API keys or tokens
- ✅ No sensitive file paths
- ✅ No proprietary business logic
- ✅ Public-safe content only

## License

Part of OpenAgents - MIT License
