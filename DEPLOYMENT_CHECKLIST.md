# OpenAgents Demo Gallery - Deployment Checklist

> **DEPRECATED (2025-12-28):** Demo gallery has been moved to ~/code/backroom.
> This document is kept for historical reference only.

**Objective:** Deploy the autopilot demo gallery to demos.openagents.com

**Estimated Time:** 1.5 hours hands-on work

**Status:** Archived (was 95% complete, infrastructure only remaining)

---

## Pre-Deployment Verification

### ✅ Content Ready
- [x] 5 demo bundles created and tested (121KB total)
- [x] Quality scores validated (85-91/100 range)
- [x] gallery.html generated (529 lines)
- [x] index.json catalog complete
- [x] All bundles manually reviewed for secrets
- [x] README.md documentation created

### ✅ Code Ready
- [x] netlify.toml configuration created
- [x] Cache headers configured
- [x] Redirects configured (/ → /gallery.html)
- [x] No build step required (static site)

### ✅ Documentation Ready
- [x] D-027 status report complete
- [x] Demo generation workflow documented
- [x] Deployment guide created
- [x] Success metrics defined

---

## Deployment Steps

### Phase 1: Netlify Setup (10 minutes)

**Prerequisites:**
- GitHub repository pushed to remote
- Netlify account created (free tier sufficient)

**Steps:**

1. **Install Netlify CLI**
   ```bash
   npm install -g netlify-cli
   ```

2. **Login to Netlify**
   ```bash
   netlify login
   # Opens browser for authentication
   ```

3. **Initialize site**
   ```bash
   cd /home/christopherdavid/code/openagents
   netlify init
   # Select: Create & configure a new site
   # Team: (your team)
   # Site name: openagents-demos (or choose another)
   # Build command: (leave empty)
   # Directory to deploy: demos
   ```

4. **Verify configuration**
   ```bash
   cat netlify.toml
   # Should show publish = "demos/"
   ```

**Expected Result:** Site created on Netlify with auto-deploy enabled

**Verification:**
```bash
netlify open
# Opens Netlify dashboard in browser
```

---

### Phase 2: DNS Configuration (10 minutes)

**Prerequisites:**
- Access to openagents.com DNS settings
- Domain registrar account (Namecheap, Cloudflare, etc.)

**Steps:**

1. **Add custom domain in Netlify**
   ```bash
   netlify domains:add demos.openagents.com
   ```

2. **Configure DNS at registrar**

   **If using Cloudflare:**
   - Login to Cloudflare dashboard
   - Select openagents.com domain
   - Add CNAME record:
     ```
     Type: CNAME
     Name: demos
     Target: [your-netlify-site].netlify.app
     Proxy: Enabled (orange cloud)
     TTL: Auto
     ```

   **If using Namecheap/other:**
   - Login to registrar control panel
   - Manage DNS for openagents.com
   - Add CNAME record:
     ```
     Type: CNAME
     Host: demos
     Value: [your-netlify-site].netlify.app
     TTL: 300 (5 minutes)
     ```

3. **Verify DNS propagation**
   ```bash
   # Wait 1-5 minutes, then check
   dig demos.openagents.com
   # Should show CNAME pointing to Netlify

   # Or use online tool
   # https://dnschecker.org/#CNAME/demos.openagents.com
   ```

**Expected Result:** DNS resolves demos.openagents.com → Netlify

**Verification:**
```bash
curl -I https://demos.openagents.com
# Should return 200 OK (may take 5-10 minutes)
```

---

### Phase 3: Deploy Site (5 minutes)

**Steps:**

1. **Final content review**
   ```bash
   cd demos
   ls -lh
   # Verify all 5 .replay.tar.gz files present
   # Verify gallery.html and index.json present
   ```

2. **Test locally one last time**
   ```bash
   python3 -m http.server 8000 &
   curl -s http://localhost:8000/gallery.html | grep -i "OpenAgents"
   # Should show page title
   pkill -f "python3 -m http.server"
   ```

3. **Commit all changes**
   ```bash
   cd /home/christopherdavid/code/openagents
   git status
   # Should show: netlify.toml, demos/README.md, DEPLOYMENT_CHECKLIST.md

   git add netlify.toml demos/README.md DEPLOYMENT_CHECKLIST.md
   git commit -m "$(cat <<'EOF'
   Add deployment configuration for demo gallery

   - Netlify configuration with cache headers
   - Demos directory README with deployment guide
   - Deployment checklist for production launch

   All 5 demo bundles ready (121KB total, scores 85-91/100)
   Gallery page generated and tested locally
   Ready for deployment to demos.openagents.com

   🤖 Generated with [Claude Code](https://claude.com/claude-code)

   Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
   Co-Authored-By: Autopilot <autopilot@openagents.com>
   EOF
   )"
   ```

4. **Push to GitHub**
   ```bash
   git push origin main
   ```

5. **Deploy to production**
   ```bash
   netlify deploy --prod
   # Confirm deployment when prompted
   ```

**Expected Result:** Site deployed to Netlify and accessible via HTTPS

**Verification:**
```bash
# Check site is live
curl -I https://demos.openagents.com
# Should return 200 OK

# Check gallery loads
curl -s https://demos.openagents.com/gallery.html | grep "OpenAgents Autopilot"

# Check demo bundles downloadable
curl -I https://demos.openagents.com/20251219-2138-start-working.replay.tar.gz
# Should return 200 OK

# Check JSON catalog
curl -s https://demos.openagents.com/index.json | jq .stats.total_demos
# Should return: 5
```

---

### Phase 4: Post-Deployment Verification (15 minutes)

**Manual Testing:**

1. **Desktop browsers:**
   - [ ] Chrome: Open https://demos.openagents.com
   - [ ] Firefox: Open https://demos.openagents.com
   - [ ] Safari: Open https://demos.openagents.com (if available)

2. **Mobile browsers:**
   - [ ] Mobile Chrome: Visit site on phone
   - [ ] Mobile Safari: Visit site on iPhone (if available)

3. **Functionality checks:**
   - [ ] Gallery page loads < 2 seconds
   - [ ] All 5 demo cards visible
   - [ ] Download buttons work
   - [ ] Quality score badges display correctly
   - [ ] Filtering works (difficulty levels)
   - [ ] No console errors (check DevTools)

4. **Performance checks:**
   - [ ] Run Lighthouse audit (https://web.dev/measure/)
   - [ ] Target: 90+ performance score
   - [ ] Target: 100 accessibility score
   - [ ] Target: 100 SEO score

**Automated Checks:**

```bash
# Verify all critical URLs
URLS=(
    "https://demos.openagents.com"
    "https://demos.openagents.com/gallery.html"
    "https://demos.openagents.com/index.json"
    "https://demos.openagents.com/20251219-2138-start-working.replay.tar.gz"
    "https://demos.openagents.com/20251219-2122-start-working.replay.tar.gz"
    "https://demos.openagents.com/20251223-235126-process-issues-from-database.replay.tar.gz"
    "https://demos.openagents.com/20251223-231615-process-issues-from-database.replay.tar.gz"
    "https://demos.openagents.com/20251222-162040-call-issue-ready-now-to.replay.tar.gz"
)

for url in "${URLS[@]}"; do
    echo "Testing: $url"
    curl -sf -o /dev/null "$url" && echo "✅ OK" || echo "❌ FAILED"
done
```

**Expected Result:** All URLs return 200 OK

---

### Phase 5: Launch Announcement (1 hour)

**Prerequisites:**
- All verification checks passed
- Site fully functional
- No console errors

**Steps:**

1. **GitHub Release**
   ```bash
   # Create annotated tag
   git tag -a demo-gallery-v1.0 -m "Demo Gallery v1.0 - Initial Launch

   Features:
   - 5 curated autopilot session replays
   - Quality scores 85-91/100
   - 121KB total bundle size
   - Production-ready gallery interface

   Live at: https://demos.openagents.com"

   git push origin demo-gallery-v1.0
   ```

2. **Social Media**

   **Twitter/X:**
   ```
   🚀 OpenAgents Autopilot Demo Gallery is LIVE!

   Watch real autopilot sessions:
   • 56 file refactorings
   • Multi-issue processing
   • Autonomous code generation
   • All with tests passing ✅

   👉 https://demos.openagents.com

   #AI #Coding #Automation #OpenSource
   ```

   **LinkedIn:**
   ```
   Excited to share the OpenAgents Autopilot Demo Gallery!

   We've open-sourced real session replays showing AI autonomously:
   - Creating and processing GitHub issues
   - Refactoring 26-56 files in single sessions
   - Maintaining d-012 compliance (no stubs, working code)
   - Running tests and creating PRs

   All demos scored 85-91/100 on our quality rubric.

   Check it out: https://demos.openagents.com
   ```

3. **Hacker News**

   **Title:** OpenAgents Autopilot Demo Gallery – Real AI Coding Session Replays

   **URL:** https://demos.openagents.com

   **Comment (optional):**
   ```
   Hi HN! I'm sharing our autopilot demo gallery - real, unedited sessions
   of our AI agent working on a production codebase.

   We analyzed 287 sessions and selected the top 5 based on quality metrics:
   successful completion, code quality, tool diversity, and narrative clarity.

   Each demo is a complete workflow: claim issue → analyze → plan → code →
   test → commit → PR. The largest session refactored 56 files across 10
   commits.

   All bundles are downloadable (121KB total) and include session logs,
   git diffs, and metadata.

   Would love feedback on the gallery UX and which demo types would be
   most valuable!
   ```

4. **Reddit**

   **Subreddits:**
   - r/programming
   - r/rust
   - r/LocalLLaMA
   - r/opensource

   **Title:** [Show] OpenAgents Autopilot Demo Gallery - Watch AI Code in Real-Time

   **Body:**
   ```
   I've launched a demo gallery showing real autopilot coding sessions:
   https://demos.openagents.com

   - 5 unedited session replays (quality scores 85-91/100)
   - Shows complete workflows from issue → PR
   - Largest session: 56 files, 10 commits
   - All tests passing, d-012 compliant (no stubs)

   Each demo is downloadable as a .replay bundle with full logs and diffs.

   Feedback welcome!
   ```

5. **Community Channels**
   - Post in OpenAgents Discord/Slack
   - Share in relevant developer communities
   - Email to interested beta testers
   - Update main site with link to demos

**Expected Result:** Initial traffic to site, early feedback collected

---

## Success Metrics

### Week 1 (Launch + 7 days)

**Traffic:**
- [ ] 1000+ unique visitors
- [ ] 500+ page views
- [ ] < 60% bounce rate

**Engagement:**
- [ ] 50+ demo downloads
- [ ] 10+ GitHub stars (attribution)
- [ ] 5+ community discussions

**Technical:**
- [ ] 99.9% uptime
- [ ] < 2s average load time
- [ ] Zero critical errors

### Month 1 (Launch + 30 days)

**Growth:**
- [ ] 10,000+ visitors
- [ ] 500+ downloads
- [ ] 100+ GitHub stars

**Feedback:**
- [ ] 3+ feature requests collected
- [ ] Conversion funnel defined (demo → trial → paid)
- [ ] Payment integration spec'd

---

## Rollback Plan

**If critical issues arise:**

1. **Immediate rollback:**
   ```bash
   # Revert to previous deployment
   netlify rollback
   ```

2. **DNS rollback:**
   ```bash
   # Remove CNAME record at DNS provider
   # Site becomes inaccessible within 5 minutes
   ```

3. **Emergency contact:**
   - Netlify support: support@netlify.com
   - DNS provider support: (depends on registrar)

**Criteria for rollback:**
- Site completely inaccessible (5xx errors)
- Security issue discovered (leaked credentials)
- Major UX breaking bug (demos won't play)
- Legal concern (copyright, content)

---

## Post-Launch Tasks

### Immediate (Within 24 hours)
- [ ] Monitor analytics for traffic
- [ ] Check error logs for issues
- [ ] Respond to early feedback
- [ ] Fix any critical bugs

### Week 1
- [ ] Collect user feedback
- [ ] Analyze most popular demos
- [ ] Plan next 5 demos based on feedback
- [ ] Implement quick-win improvements

### Week 2-4
- [ ] Add playback controls (issue #1527)
- [ ] Implement metrics sidebar (issue #1532)
- [ ] Generate 5 more demos
- [ ] Refresh gallery with new content
- [ ] Integrate gallery into main openagents.com site

### Month 2
- [ ] Add analytics dashboard
- [ ] Implement feedback form
- [ ] Create "Request Demo" workflow
- [ ] Define payment funnel
- [ ] Begin Stripe + Lightning integration

---

## Support & Monitoring

**Monitoring Tools:**
- Netlify Analytics (built-in)
- Plausible Analytics (add if needed)
- UptimeRobot (free tier)
- Sentry (error tracking, if needed)

**Support Channels:**
- GitHub Issues: https://github.com/OpenAgentsInc/openagents/issues
- Discord/Slack: (if available)
- Email: support@openagents.com (if configured)

---

## Conclusion

This checklist covers the complete deployment workflow from local verification to production launch and post-launch monitoring.

**Current Status:** All code and content ready. Deployment execution is the only remaining work.

**Estimated Total Time:** ~1.5 hours hands-on work
- Netlify setup: 10 min
- DNS config: 10 min
- Deploy: 5 min
- Verification: 15 min
- Launch comms: 1 hour

**Next Action:** Begin Phase 1 (Netlify Setup)

---

**Document Version:** 1.0
**Last Updated:** 2025-12-27
**Owner:** OpenAgents Team
**Status:** Ready for Execution
