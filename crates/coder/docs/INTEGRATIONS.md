# Integrations

Coder is Git-native by design. Everything results in branches, PRs, checks, and deploys.

---

## Version Control

### Git Integration

| Feature | Status | Description |
|---------|--------|-------------|
| Init Repository | MVP | Create new repo |
| Commit | MVP | Stage and commit |
| Push/Pull | MVP | Sync with remote |
| Branch | MVP | Create/switch branches |
| Merge | Phase 2 | Merge branches |
| Conflict Resolution | Phase 2 | Visual merge tool |
| History | MVP | View commit log |
| Diff | MVP | See changes |
| GitHub Integration | MVP | Connect to GitHub |
| GitLab Integration | Phase 2 | Connect to GitLab |

### Branch-per-Agent Model

Each agent works on its own branch:
```
coder/architect/design-auth-system
coder/implementer/add-login-form
coder/tester/auth-tests
```

Changes merge through normal PR flow with review gates.

---

## CI/CD Integration

| Feature | Status | Description |
|---------|--------|-------------|
| GitHub Actions | MVP | Trigger and monitor workflows |
| GitLab CI | Phase 2 | Pipeline integration |
| Status Checks | MVP | PR status updates |
| Test Results | MVP | Parse and display results |
| Build Logs | MVP | Stream build output |

---

## Deployment

### Supported Targets

| Target | Status | Description |
|--------|--------|-------------|
| Cloudflare Pages | MVP | Static sites |
| Cloudflare Workers | MVP | Edge compute |
| Vercel | Phase 2 | Next.js optimized |
| Netlify | Phase 2 | JAMstack |
| Custom Domain | MVP | Your domain |
| Preview Deployments | MVP | Per-commit deploys |

### Deployment Features

| Feature | Status | Description |
|---------|--------|-------------|
| Automatic Builds | MVP | Build on push |
| Build Logs | MVP | Real-time output |
| Rollback | MVP | One-click revert |
| Environment Variables | MVP | Per-environment |
| Custom Build Commands | MVP | Configure build |
| Branch Deploys | Phase 2 | Deploy any branch |

---

## Domain Management

| Feature | Status | Description |
|---------|--------|-------------|
| Subdomain | MVP | yourapp.coder.openagents.com |
| Custom Domain | MVP | yourdomain.com |
| SSL Certificates | MVP | Automatic HTTPS |
| DNS Management | Phase 2 | In-platform DNS |
| Redirects | MVP | 301/302 rules |
| Headers | MVP | Custom headers |

---

## Secrets Management

| Feature | Status | Description |
|---------|--------|-------------|
| .env Support | MVP | Environment variables |
| Encrypted Storage | MVP | All secrets encrypted |
| Multiple Environments | MVP | Dev/staging/prod |
| Scoped Access | MVP | Per-workflow access control |
| Rotation | Phase 3 | Automatic key rotation |

---

## External Integrations

| Integration | Status | Description |
|-------------|--------|-------------|
| GitHub | MVP | Repo sync, PRs, issues |
| GitLab | Phase 2 | Repo sync |
| Slack | Phase 2 | Notifications |
| Discord | MVP | Notifications |
| Linear | Phase 2 | Issue tracking |
| Jira | Phase 3 | Issue tracking |
| Figma | Phase 2 | Design import |

---

## Webhooks

Coder emits events that can trigger external systems:

| Event | Description |
|-------|-------------|
| `workflow.started` | Workflow run began |
| `workflow.completed` | Workflow run finished |
| `workflow.failed` | Workflow run failed |
| `agent.started` | Agent run began |
| `agent.completed` | Agent run finished |
| `deployment.started` | Deploy began |
| `deployment.completed` | Deploy finished |
| `pr.opened` | PR was opened |
| `pr.merged` | PR was merged |

---

## API

### Endpoints

| Category | Endpoints | Status |
|----------|-----------|--------|
| Auth | /auth/* | MVP |
| Projects | /projects/* | MVP |
| Files | /projects/:id/files/* | MVP |
| Workflows | /workflows/* | MVP |
| Runs | /runs/* | MVP |
| Deployments | /deployments/* | MVP |
| Agents | /agents/* | MVP |

### API Features

| Feature | Status | Description |
|---------|--------|-------------|
| REST API | MVP | HTTP endpoints |
| WebSocket | MVP | Real-time events |
| Rate Limiting | MVP | Fair usage |
| Versioning | MVP | v1/v2/etc |
| OpenAPI Spec | MVP | Documentation |

---

*Last Updated: December 2025*
