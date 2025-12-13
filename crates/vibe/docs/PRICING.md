# Vibe Platform Pricing

Detailed pricing structure for the Vibe platform.

---

## 1. Platform Subscriptions

### 1.1 Tier Overview

| Tier | Monthly Price | Annual Price | Target User |
|------|--------------|--------------|-------------|
| **Free** | $0 | $0 | Explorers, students |
| **Pro** | $29/mo | $290/yr (17% off) | Indie developers |
| **Team** | $99/seat/mo | $990/seat/yr | Startups, small teams |
| **Business** | $299/seat/mo | $2,990/seat/yr | Companies |
| **Enterprise** | Custom | Custom | Large organizations |

### 1.2 Feature Matrix

| Feature | Free | Pro | Team | Business | Enterprise |
|---------|------|-----|------|----------|------------|
| **Projects** | 1 | Unlimited | Unlimited | Unlimited | Unlimited |
| **AI Prompts/Day** | 100 | 1,000 | 10,000/seat | 50,000/seat | Unlimited |
| **Agent Runs/Day** | 10 | 100 | 1,000 | 10,000 | Unlimited |
| **Collaborators** | 0 | 3 | Unlimited | Unlimited | Unlimited |
| **Storage** | 100 MB | 10 GB | 100 GB | 1 TB | Custom |
| **Deployments/Day** | 3 | 50 | 200 | Unlimited | Unlimited |
| **Custom Domains** | - | 1 | 5 | 20 | Unlimited |
| **Templates** | Community | All | All | All + Custom | All + Custom |
| **Support** | Community | Email | Priority | Dedicated | 24/7 SLA |
| **SSO** | - | - | - | SAML/OIDC | SAML/OIDC |
| **Audit Logs** | - | - | - | 30 days | Unlimited |
| **SLA** | - | - | - | 99.5% | 99.9% |
| **Private Cloud** | - | - | - | - | Optional |

### 1.3 Detailed Tier Breakdown

#### Free Tier
**Target:** Exploration, learning, hobby projects

**Includes:**
- 1 active project
- 100 AI prompts per day
- 10 agent runs per day
- 100 MB storage
- 3 deployments per day
- Community templates
- Community support (Discord)
- Vibe badge required on deployed sites

**Limits:**
- No custom domains
- No collaboration
- No private projects (all public)
- Rate limited API access

---

#### Pro Tier ($29/month)
**Target:** Indie developers, freelancers, side projects

**Includes Everything in Free, plus:**
- Unlimited projects
- 1,000 AI prompts per day
- 100 agent runs per day
- 10 GB storage
- 50 deployments per day
- 1 custom domain
- All templates (including premium)
- Email support (48h response)
- Private projects
- 3 collaborators per project
- No Vibe badge required

**Additional Usage:**
- Extra AI prompts: $0.01 per prompt
- Extra storage: $1/GB/month
- Extra domains: $5/domain/month

---

#### Team Tier ($99/seat/month)
**Target:** Startups, agencies, growing teams

**Includes Everything in Pro, plus:**
- Per-seat billing
- 10,000 AI prompts per day per seat
- 1,000 agent runs per day per seat
- 100 GB shared storage
- 200 deployments per day
- 5 custom domains
- Unlimited collaborators
- Priority support (24h response)
- Team management dashboard
- Shared billing
- Role-based permissions
- Usage analytics

**Additional Usage:**
- Extra AI prompts: $0.008 per prompt
- Extra storage: $0.80/GB/month
- Extra domains: $4/domain/month

**Minimum Seats:** 3

---

#### Business Tier ($299/seat/month)
**Target:** Companies with compliance needs

**Includes Everything in Team, plus:**
- 50,000 AI prompts per day per seat
- 10,000 agent runs per day per seat
- 1 TB shared storage
- Unlimited deployments
- 20 custom domains
- SAML/OIDC SSO
- 30-day audit logs
- 99.5% uptime SLA
- Dedicated support (4h response)
- Custom onboarding
- Quarterly business reviews
- Security questionnaire support

**Additional Usage:**
- Extra AI prompts: $0.006 per prompt
- Extra storage: $0.50/GB/month
- Extra domains: $3/domain/month

**Minimum Seats:** 10

---

#### Enterprise Tier (Custom Pricing)
**Target:** Large organizations, regulated industries

**Includes Everything in Business, plus:**
- Unlimited AI prompts
- Unlimited agent runs
- Custom storage allocation
- Unlimited custom domains
- Unlimited audit log retention
- 99.9% uptime SLA with credits
- 24/7 dedicated support
- Named account manager
- Custom integrations
- Private cloud deployment option
- HIPAA BAA available
- SOC 2 Type II attestation
- Custom security review
- Priority feature requests
- Executive briefings

**Starting at:** $5,000/month

**Typical Contracts:**
- $50K-$150K/year for mid-market
- $150K-$500K/year for enterprise
- $500K+/year for strategic accounts

---

## 2. Infrastructure Resale Pricing

### 2.1 Platform Fee
Customers pay a platform fee plus usage-based charges.

| Tier | Monthly Platform Fee | Included Credits | Support |
|------|---------------------|------------------|---------|
| **Starter** | $99/mo | $50 | Email |
| **Growth** | $499/mo | $300 | Priority |
| **Scale** | $2,499/mo | $2,000 | Dedicated |
| **Enterprise** | $9,999/mo | $8,000 | 24/7 SLA |

### 2.2 Usage-Based Pricing

| Resource | Unit | Our Price | Cloudflare Cost | Margin |
|----------|------|-----------|-----------------|--------|
| **Worker Requests** | 1M requests | $2.00 | $0.50 | 75% |
| **Durable Object Requests** | 1M requests | $5.00 | $1.25 | 75% |
| **Durable Object Duration** | 1M GB-seconds | $15.00 | $3.75 | 75% |
| **R2 Class A Operations** | 1M ops | $5.00 | $4.50 | 10% |
| **R2 Class B Operations** | 1M ops | $0.50 | $0.36 | 28% |
| **R2 Storage** | GB/month | $0.02 | $0.015 | 25% |
| **D1 Rows Read** | 1M rows | $0.002 | $0.001 | 50% |
| **D1 Rows Written** | 1M rows | $1.50 | $1.00 | 33% |
| **D1 Storage** | GB/month | $1.00 | $0.75 | 25% |
| **AI Inference** | 1K tokens | $0.10 | $0.02 | 80% |
| **Bandwidth Egress** | GB | $0.10 | $0.04 | 60% |

### 2.3 Volume Discounts

| Monthly Spend | Discount |
|--------------|----------|
| $1K - $5K | 0% |
| $5K - $25K | 10% |
| $25K - $100K | 15% |
| $100K - $500K | 20% |
| $500K+ | Custom |

### 2.4 Example Bills

**Startup (Growth Tier):**
```
Platform Fee:                    $499.00
Worker Requests (50M):           $100.00
DO Requests (10M):               $50.00
R2 Storage (50 GB):              $1.00
D1 Storage (5 GB):               $5.00
AI Inference (500K tokens):      $50.00
Bandwidth (100 GB):              $10.00
─────────────────────────────────────────
Subtotal:                        $715.00
Included Credits:               -$300.00
─────────────────────────────────────────
Total:                           $415.00
```

**Scale Company (Scale Tier):**
```
Platform Fee:                    $2,499.00
Worker Requests (500M):          $1,000.00
DO Requests (100M):              $500.00
DO Duration (50M GB-s):          $750.00
R2 Storage (500 GB):             $10.00
D1 Storage (50 GB):              $50.00
AI Inference (5M tokens):        $500.00
Bandwidth (1 TB):                $100.00
─────────────────────────────────────────
Subtotal:                        $5,409.00
Included Credits:               -$2,000.00
Volume Discount (10%):           -$340.90
─────────────────────────────────────────
Total:                           $3,068.10
```

---

## 3. Marketplace Fees

### 3.1 Seller Fees

| Transaction Type | Fee | Minimum Fee |
|-----------------|-----|-------------|
| Agent Sales | 15% | $0.50 |
| Template Sales | 20% | $1.00 |
| Compute Credit Sales | 5% | $0.25 |
| Service Listings | 10% | $2.00 |

### 3.2 Seller Tiers

| Tier | Lifetime Sales | Fee Reduction | Benefits |
|------|---------------|---------------|----------|
| **New** | $0 - $1K | 0% | Basic listing |
| **Rising** | $1K - $10K | 2% off | Featured placement |
| **Established** | $10K - $100K | 5% off | Premium badge |
| **Star** | $100K+ | 8% off | Homepage feature |

### 3.3 Payout Schedule

| Method | Minimum | Fee | Timeline |
|--------|---------|-----|----------|
| Lightning | $1 | 0% | Instant |
| Bitcoin On-chain | $100 | Network fee | 1-6 confirmations |
| Bank Transfer | $100 | $5 | 3-5 business days |
| Stripe | $50 | 2.9% + $0.30 | 2 business days |

---

## 4. AI Usage Pricing

### 4.1 Models Available

| Model | Quality | Speed | Cost (per 1K tokens) |
|-------|---------|-------|---------------------|
| Claude 3.5 Sonnet | High | Medium | $0.015 |
| Claude 3 Opus | Highest | Slow | $0.075 |
| Claude 3 Haiku | Medium | Fast | $0.0025 |
| GPT-4 Turbo | High | Medium | $0.03 |
| GPT-4o | High | Fast | $0.015 |
| Llama 3 70B (Workers AI) | Medium | Fast | $0.002 |
| Llama 3 8B (Workers AI) | Lower | Fastest | $0.0005 |

### 4.2 Prompt Pricing

Each "prompt" in tier limits = approximately 2,000 tokens (input + output combined).

**Example:**
- Free tier: 100 prompts/day ≈ 200K tokens/day
- Pro tier: 1,000 prompts/day ≈ 2M tokens/day

### 4.3 Overage Rates

When exceeding tier limits, overage rates apply:

| Tier | Overage Rate (per prompt) |
|------|--------------------------|
| Free | Not allowed (hard limit) |
| Pro | $0.01 |
| Team | $0.008 |
| Business | $0.006 |
| Enterprise | Custom (typically $0.004) |

---

## 5. Add-Ons

### 5.1 Platform Add-Ons

| Add-On | Monthly Price | Description |
|--------|--------------|-------------|
| **Extra Storage** | $1/GB | Beyond tier allocation |
| **Extra Domains** | $5/domain | Custom domains beyond tier |
| **Priority AI Queue** | $19/mo | Skip the queue for AI calls |
| **Advanced Analytics** | $29/mo | Detailed usage insights |
| **Audit Log Extended** | $49/mo | 1 year retention |
| **Uptime SLA Upgrade** | $99/mo | 99.99% for Pro/Team |

### 5.2 Enterprise Add-Ons

| Add-On | Price | Description |
|--------|-------|-------------|
| **Private Cloud** | +50% | Dedicated infrastructure |
| **Data Residency** | +20% | Geographic restrictions |
| **HIPAA Compliance** | +30% | Healthcare compliance |
| **Custom SLA** | Custom | Tailored guarantees |
| **Professional Services** | $250/hr | Implementation help |

---

## 6. Discounts & Programs

### 6.1 Startup Program

**Eligibility:**
- Less than $5M in funding
- Less than 50 employees
- Founded within last 3 years

**Benefits:**
- 50% off first year
- Free Team tier for 6 months
- $1,000 infrastructure credits
- Priority support

### 6.2 Education Program

**Eligibility:**
- Students with .edu email
- Teachers/professors
- Educational institutions

**Benefits:**
- Free Pro tier for students
- 75% off Team for institutions
- Free access to all templates
- Educational resources

### 6.3 Open Source Program

**Eligibility:**
- Public GitHub repo with 500+ stars
- Active maintenance
- Open source license (MIT, Apache, etc.)

**Benefits:**
- Free Team tier
- $500/month infrastructure credits
- Featured in marketplace
- Co-marketing opportunities

### 6.4 Non-Profit Program

**Eligibility:**
- Registered 501(c)(3) or equivalent
- Non-political organization

**Benefits:**
- 50% off all tiers
- Free consultation
- Priority support

---

## 7. Billing & Payment

### 7.1 Payment Methods

| Method | Availability | Notes |
|--------|-------------|-------|
| Credit Card | All tiers | Visa, MC, Amex |
| Lightning Network | All tiers | Instant, low fees |
| Bitcoin On-chain | Business+ | Monthly invoices |
| ACH/Wire | Enterprise | Net 30 terms available |
| Invoice | Enterprise | Net 30/60/90 terms |

### 7.2 Billing Cycles

| Type | When Billed | Notes |
|------|------------|-------|
| Subscriptions | Start of period | Monthly or annual |
| Usage Overages | End of period | Aggregated monthly |
| Infrastructure | End of period | Usage-based |
| Marketplace | On transaction | Real-time |

### 7.3 Refund Policy

| Situation | Refund |
|-----------|--------|
| Annual subscription (first 14 days) | Full refund |
| Annual subscription (after 14 days) | Pro-rated credit |
| Monthly subscription | No refund |
| Infrastructure usage | No refund |
| Marketplace purchases | Case-by-case |

---

## 8. Revenue Projections

### 8.1 Month 6 Target Mix

| Stream | Monthly | % of Total |
|--------|---------|-----------|
| Subscriptions | $50M | 30% |
| Infrastructure | $60M | 36% |
| Marketplace | $30M | 18% |
| Enterprise | $27M | 16% |
| **Total** | **$167M** | **100%** |

### 8.2 Average Revenue Per User (ARPU)

| Tier | Monthly ARPU | Users (M6) | MRR |
|------|-------------|------------|-----|
| Free → Paid Conversion | - | 5M free | - |
| Pro | $32 | 300K | $9.6M |
| Team | $145 | 150K | $21.8M |
| Business | $450 | 45K | $20.3M |
| Enterprise | $15K | 90 | $1.4M |

### 8.3 Infrastructure Customers

| Tier | Monthly | Customers | MRR |
|------|---------|-----------|-----|
| Starter | $400 avg | 2,000 | $0.8M |
| Growth | $2K avg | 2,500 | $5M |
| Scale | $15K avg | 1,200 | $18M |
| Enterprise | $100K avg | 300 | $30M |

---

## 9. Competitive Pricing Analysis

| Feature | Vibe | Cursor | Lovable | Bolt | Replit |
|---------|------|--------|---------|------|--------|
| Free Tier | Yes | Yes | Yes | Yes | Yes |
| Pro Price | $29 | $20 | $25 | $20 | $25 |
| Team Price | $99 | N/A | $50 | $30 | Custom |
| AI Prompts (Pro) | 1K/day | 500/mo | Limited | 10M tok | Limited |
| Storage (Pro) | 10 GB | Local | 5 GB | Local | 10 GB |
| Custom Domains | Yes | N/A | Yes | Yes | Yes |
| Infrastructure | Yes | No | No | No | Limited |
| Lightning Payments | Yes | No | No | No | No |

**Positioning:**
- Higher than Cursor/Bolt on Pro (justified by infrastructure access)
- Competitive with Lovable on Team (more features)
- Infrastructure resale is unique differentiator
- Lightning payments reduce transaction costs

---

*Document Version: 1.0*
*Last Updated: December 2024*
