# OpenAgents Web Platform

The revenue-generating web application that connects users to autonomous code execution.

## Features

- **Landing Page** - Marketing site with clear value proposition
- **GitHub OAuth** - Seamless repo connection via GitHub authentication
- **Stripe Checkout** - Credit-based billing with pay-as-you-go model
- **Autopilot Runner** - Managed autonomous code execution jobs
- **Job Tracking** - Real-time status updates and result delivery

## Quick Start

### 1. Set Up Environment

```bash
cd crates/web-platform
cp .env.example .env
# Edit .env with your GitHub and Stripe credentials
```

### 2. Configure GitHub OAuth

1. Go to https://github.com/settings/developers
2. Click "New OAuth App"
3. Fill in:
   - **Application name**: OpenAgents Development
   - **Homepage URL**: http://localhost:8080
   - **Authorization callback URL**: http://localhost:8080/auth/github/callback
4. Copy Client ID and Client Secret to `.env`

### 3. Configure Stripe

1. Go to https://dashboard.stripe.com/test/apikeys
2. Copy your test API keys to `.env`
3. Set up webhook endpoint (for production):
   - URL: https://your-domain.com/checkout/webhook
   - Events: `checkout.session.completed`, `payment_intent.succeeded`

### 4. Run the Server

```bash
cargo run -p web-platform
```

Visit http://localhost:8080 to see the landing page.

## API Endpoints

### Authentication

- `GET /auth/github` - Start GitHub OAuth flow
- `GET /auth/github/callback` - OAuth callback handler

### Billing

- `POST /checkout/create-session` - Create Stripe checkout session
  ```json
  {
    "credits": 500000
  }
  ```
- `GET /checkout/success` - Checkout success page
- `GET /checkout/cancel` - Checkout cancelled page
- `POST /checkout/webhook` - Stripe webhook handler

### Autopilot Jobs

- `POST /autopilot/start` - Start a new autopilot job
  ```json
  {
    "repo_url": "https://github.com/user/repo",
    "task": "Fix all type errors",
    "user_id": "user_123"
  }
  ```
- `GET /autopilot/status/{job_id}` - Check job status
- `POST /autopilot/cancel/{job_id}` - Cancel running job

## Architecture

```
┌─────────────┐
│   Browser   │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────┐
│      Actix-web HTTP Server          │
├─────────────────────────────────────┤
│  /auth/github  │  GitHub OAuth      │
│  /checkout     │  Stripe Billing    │
│  /autopilot    │  Job Management    │
└──────┬──────────┬──────────┬────────┘
       │          │          │
       ▼          ▼          ▼
   ┌────────┐ ┌─────────┐ ┌──────────┐
   │ GitHub │ │ Stripe  │ │ Autopilot│
   │  API   │ │   API   │ │  Runner  │
   └────────┘ └─────────┘ └──────────┘
```

## Database Schema

### Users
- `id` - Primary key
- `github_id` - GitHub user ID
- `github_login` - GitHub username
- `email` - User email
- `credits` - Available credits (default: 10,000 free)
- `created_at` / `updated_at` - Timestamps

### Jobs
- `id` - Job ID
- `user_id` - Foreign key to users
- `repo_url` - Repository URL
- `task` - Task description
- `status` - queued | running | completed | failed | cancelled
- `pr_url` - Generated pull request URL
- `credits_used` - Credits consumed

### Transactions
- `id` - Transaction ID
- `user_id` - Foreign key to users
- `type` - purchase | refund | usage
- `amount` - Credit amount
- `stripe_session_id` - Stripe checkout session

## Pricing Model

| Plan | Price | Credits | Notes |
|------|-------|---------|-------|
| Free | $0 | 10,000 | Signup bonus |
| Pro | $20 | 500,000 | Pay as you go |
| Team | $15/seat | 300,000/seat | Shared billing |

**Credit Usage:**
- Code analysis: ~500 credits
- Small fix: ~2,000 credits
- Feature implementation: ~10,000 credits
- Large refactor: ~50,000 credits

## Production Deployment

### Option 1: Fly.io (Recommended)

```bash
# Install flyctl
curl -L https://fly.io/install.sh | sh

# Login
fly auth login

# Launch app
fly launch --name openagents-web

# Set secrets
fly secrets set GITHUB_CLIENT_ID=xxx
fly secrets set GITHUB_CLIENT_SECRET=xxx
fly secrets set STRIPE_SECRET_KEY=sk_live_xxx

# Deploy
fly deploy
```

### Option 2: Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p web-platform

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/web-platform /usr/local/bin/
CMD ["web-platform"]
```

```bash
docker build -t openagents-web .
docker run -p 8080:8080 --env-file .env openagents-web
```

### Option 3: systemd Service

```ini
[Unit]
Description=OpenAgents Web Platform
After=network.target

[Service]
Type=simple
User=openagents
WorkingDirectory=/opt/openagents
EnvironmentFile=/opt/openagents/.env
ExecStart=/opt/openagents/web-platform
Restart=always

[Install]
WantedBy=multi-user.target
```

## Security Considerations

- All secrets must be in environment variables, never committed
- Use HTTPS in production (Let's Encrypt via Caddy/nginx)
- Verify Stripe webhook signatures
- Rate limit API endpoints
- Validate GitHub OAuth state parameter
- Sanitize user inputs for task descriptions
- Use parameterized SQL queries (done via rusqlite)

## TODO: Production Readiness

- [ ] Add session management (Redis or JWT)
- [ ] Implement real Stripe integration (replace mock)
- [ ] Connect to actual autopilot execution engine
- [ ] Add WebSocket for live job progress
- [ ] Implement proper error handling and logging
- [ ] Add rate limiting per user
- [ ] Create admin dashboard
- [ ] Set up monitoring (Prometheus/Grafana)
- [ ] Add user email notifications
- [ ] Implement credit usage tracking
- [ ] Add webhook retry logic
- [ ] Create integration tests
- [ ] Set up CI/CD pipeline

## License

CC-0 (Public Domain)
