# Neobank Operator Guide

**For operators deploying treasury infrastructure in innovation-friendly jurisdictions**

This guide is for entities establishing neobank operations using OpenAgents technology. It assumes a jurisdiction with no restrictive financial regulations—a startup society, special economic zone, or digital jurisdiction actively seeking to co-develop an appropriate regulatory framework through practical operation.

OpenAgents Inc. (US) provides the technology. You, the operator, provide the jurisdiction, entity, reserves, and operational expertise.

---

## Table of Contents

1. [Overview](#overview)
2. [Entity Structure](#entity-structure)
3. [What You're Actually Operating](#what-youre-actually-operating)
4. [Technical Infrastructure](#technical-infrastructure)
5. [Reserve Management](#reserve-management)
6. [Key Management](#key-management)
7. [User Onboarding](#user-onboarding)
8. [Operational Procedures](#operational-procedures)
9. [Self-Regulatory Framework](#self-regulatory-framework)
10. [Transparency and Auditing](#transparency-and-auditing)
11. [Risk Management](#risk-management)
12. [Working with Your Jurisdiction](#working-with-your-jurisdiction)
13. [Launch Checklist](#launch-checklist)

---

## Overview

### What This Is

A neobank operator runs infrastructure enabling:

- **eCash issuance** — Cashu tokens backed 1:1 by Bitcoin reserves
- **Multi-currency support** — BTC-denominated and USD-denominated tokens
- **Lightning connectivity** — Send/receive via Lightning Network
- **Agent treasury services** — Budget enforcement, payment routing, receipts
- **Lightning Addresses** — Human-readable receive addresses

### What This Is NOT

- Not a fractional reserve bank (100% backing required)
- Not credit creation (you issue tokens against deposits, not loans)
- Not custodial in the traditional sense (users hold encrypted proofs)
- Not regulated as a bank in most jurisdictions (though this may change)

### The Operator's Role

```
┌─────────────────────────────────────────────────────────────────┐
│                     OPERATOR RESPONSIBILITIES                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  TECHNOLOGY (from OpenAgents)        YOUR RESPONSIBILITIES       │
│  ┌─────────────────────────┐        ┌─────────────────────────┐ │
│  │ • Neobank crate         │        │ • Legal entity          │ │
│  │ • Cashu protocol        │        │ • Jurisdiction choice   │ │
│  │ • Lightning integration │   →    │ • Reserve custody       │ │
│  │ • Agent SDK             │        │ • Operational staff     │ │
│  │ • UI components         │        │ • User support          │ │
│  │ • Documentation         │        │ • Compliance decisions  │ │
│  └─────────────────────────┘        └─────────────────────────┘ │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Entity Structure

### Recommended: Foundation + Operating Company

**Foundation (e.g., "Neobank Foundation")**
- Holds protocol intellectual property licenses
- Governs upgrade decisions
- Appoints board/council
- Non-profit, mission-driven
- Jurisdiction: Your startup society

**Operating Company (e.g., "Neobank Services Ltd")**
- Runs day-to-day operations
- Employs staff
- Holds reserves
- Signs user agreements
- For-profit or social enterprise
- Jurisdiction: Your startup society

### Alternative: Single Entity

For smaller operations, a single company suffices:

**Company Requirements:**
- Registered in your jurisdiction
- Clear beneficial ownership
- Bank account for fiat operations (if any)
- Crypto custody solution
- Directors with operational authority

### DAO-Wrapped Option

For maximum decentralization:

```
Jurisdiction Entity (legal wrapper)
        ↓
    DAO Treasury (on-chain multisig)
        ↓
    Operational Multisig (day-to-day)
        ↓
    Mint Infrastructure
```

The jurisdiction entity provides legal standing; the DAO provides governance; operational multisig handles routine operations.

---

## What You're Actually Operating

### Component Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                      NEOBANK INFRASTRUCTURE                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  USER-FACING                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │  Web Wallet  │  │ Mobile App   │  │  Agent SDK   │               │
│  │  (optional)  │  │  (optional)  │  │  (primary)   │               │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘               │
│         └─────────────────┴─────────────────┘                        │
│                           │                                          │
│  GATEWAY SERVICES         │                                          │
│  ┌────────────────────────┴────────────────────────────────────┐    │
│  │  Lightning Address Server  │  API Gateway  │  WebSocket Hub │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                           │                                          │
│  CORE INFRASTRUCTURE      │                                          │
│  ┌──────────────┐  ┌──────┴───────┐  ┌──────────────┐               │
│  │  Cashu Mint  │  │   Lightning  │  │   Database   │               │
│  │  (BTC + USD) │  │     Node     │  │  (encrypted) │               │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘               │
│         └─────────────────┴─────────────────┘                        │
│                           │                                          │
│  RESERVES                 │                                          │
│  ┌────────────────────────┴────────────────────────────────────┐    │
│  │  Cold Storage (BTC)  │  Hot Wallet (LN)  │  USD Reserves    │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1. Cashu Mint

The mint is the core—it issues eCash tokens.

**What it does:**
- Accepts Lightning deposits → Issues blind-signed tokens
- Accepts token redemption → Pays Lightning invoices
- Manages keysets (rotate periodically for privacy)
- Tracks spent secrets (prevent double-spending)

**Software options:**
- `nutshell` (Python reference implementation)
- `moksha` (Rust implementation)
- `cashu-rs` / CDK (Cashu Dev Kit)

**You must run:**
- At least one BTC-denominated mint
- Optionally, one or more USD-denominated mints

### 2. Lightning Node

Connects your mint to the Lightning Network.

**What it does:**
- Receives deposits (generates invoices)
- Sends withdrawals (pays invoices)
- Routes payments (optional, for revenue)
- Manages channel liquidity

**Software options:**
- LND (most common)
- Core Lightning (CLN)
- Eclair
- Or use Spark SDK (nodeless)

**Channel requirements:**
- Sufficient inbound liquidity for deposits
- Sufficient outbound liquidity for withdrawals
- Diversified channel partners (don't depend on one LSP)

### 3. Database

Stores operational data (encrypted).

**What's stored:**
- User accounts (if any—can be accountless)
- Spent secrets (double-spend prevention)
- Keyset information
- Transaction logs
- Encrypted proof backups (optional, for user recovery)

**Requirements:**
- PostgreSQL or SQLite
- Encryption at rest
- Regular backups
- Point-in-time recovery capability

### 4. Lightning Address Server

Enables `user@yourdomain.com` receive addresses.

**What it does:**
- Responds to `/.well-known/lnurlp/{username}` requests
- Creates receive quotes on demand
- Returns BOLT11 invoices
- Provides verification callbacks

**Implementation:**
- Use the neobank crate's `LightningAddressService`
- Or implement LUD-16 yourself

### 5. API Gateway

Exposes mint and treasury services to clients.

**Endpoints:**
- Cashu NUT endpoints (mint quotes, melt quotes, swap, etc.)
- Account management (if applicable)
- Balance queries
- Transaction history

---

## Technical Infrastructure

### Minimum Viable Setup

For initial launch with <1000 users:

```yaml
# Infrastructure Requirements

Compute:
  - 1x Application Server (4 CPU, 16GB RAM, 500GB SSD)
  - 1x Database Server (2 CPU, 8GB RAM, 200GB SSD)
  # Can be same machine for MVP

Lightning:
  - LND or CLN node
  - 10-50M sats in channels (both directions)
  - 3-5 well-connected channel partners

Cold Storage:
  - Hardware wallet (Coldcard, Foundation, BitBox)
  - Multisig 2-of-3 recommended
  - Geographic distribution of signers

Networking:
  - Static IP or reliable DNS
  - TLS certificates (Let's Encrypt)
  - DDoS protection (Cloudflare or similar)

Monitoring:
  - Uptime monitoring
  - Balance alerts
  - Error logging
```

### Production Setup

For 10,000+ users:

```yaml
# Production Infrastructure

Compute:
  - Load balancer (HAProxy or cloud LB)
  - 2-3x Application servers (8 CPU, 32GB RAM)
  - Primary + replica database (16 CPU, 64GB RAM, 1TB SSD)
  - Redis for caching/sessions

Lightning:
  - Primary LND node (dedicated server)
  - Backup node (hot standby)
  - 100M+ sats channel capacity
  - 10+ diverse channel partners
  - Automated rebalancing (Loop, Pool, or custom)

Cold Storage:
  - Multisig 2-of-3 or 3-of-5
  - HSM for hot wallet signing (optional)
  - Geographic + jurisdictional distribution
  - Time-locked recovery paths

Security:
  - VPN for admin access
  - Firewall rules (whitelist only)
  - Intrusion detection
  - Regular penetration testing

Backup:
  - Database: Continuous WAL archiving
  - Lightning: SCB (Static Channel Backup) to multiple locations
  - Keys: Encrypted backups in separate jurisdictions
```

### Docker Compose Example (MVP)

```yaml
version: '3.8'

services:
  mint:
    image: cashu/nutshell:latest
    environment:
      - MINT_PRIVATE_KEY=${MINT_PRIVATE_KEY}
      - MINT_LIGHTNING_BACKEND=LndRestWallet
      - MINT_LND_REST_ENDPOINT=https://lnd:8080
      - MINT_LND_REST_MACAROON=${LND_MACAROON}
      - MINT_DATABASE_URL=postgres://mint:${DB_PASSWORD}@db:5432/mint
    ports:
      - "3338:3338"
    depends_on:
      - db
      - lnd

  lnd:
    image: lightninglabs/lnd:v0.17.0-beta
    volumes:
      - lnd_data:/root/.lnd
    ports:
      - "9735:9735"   # P2P
      - "8080:8080"   # REST (internal only)

  db:
    image: postgres:15
    environment:
      - POSTGRES_USER=mint
      - POSTGRES_PASSWORD=${DB_PASSWORD}
      - POSTGRES_DB=mint
    volumes:
      - db_data:/var/lib/postgresql/data

  lnurl:
    image: openagents/lnurl-server:latest
    environment:
      - MINT_URL=http://mint:3338
      - DATABASE_URL=postgres://mint:${DB_PASSWORD}@db:5432/mint
      - BASE_URL=https://yourdomain.com
    ports:
      - "3000:3000"

volumes:
  lnd_data:
  db_data:
```

---

## Reserve Management

### The Fundamental Rule

**100% reserves. Always. No exceptions.**

Every token in circulation must be backed by an equivalent amount in reserves. This is not a suggestion—it is the core trust assumption of the system.

### BTC-Denominated Tokens

For every sat of BTC eCash issued, you hold 1 sat.

```
Reserve Location:
├── Hot Wallet (Lightning channels): 20-40%
│   └── For immediate redemptions
├── Warm Wallet (on-chain, quick access): 20-30%
│   └── For channel rebalancing, large redemptions
└── Cold Storage (multisig, offline): 40-60%
    └── Long-term reserves, emergency only
```

### USD-Denominated Tokens

More complex—you're providing volatility protection.

**Option A: BTC-Backed with Hedging**

```
User deposits 100 USD worth of BTC
    ↓
You receive ~0.001 BTC (at current rate)
    ↓
You issue 100 USD eCash tokens
    ↓
You hedge the BTC exposure:
  - Perpetual short on exchange
  - Options collar
  - Or accept the risk (dangerous)
```

**Option B: Actual USD Reserves**

```
User deposits BTC
    ↓
You sell BTC for USD (via exchange or OTC)
    ↓
You hold USD in:
  - Bank account
  - USD stablecoin (USDC, USDT)
  - Treasury bills
    ↓
You issue USD eCash tokens
```

**Option C: Stablecoin Reserves**

```
User deposits BTC or stablecoin
    ↓
You convert to/hold USDC or USDT
    ↓
You issue USD eCash tokens backed by stablecoin
```

**Recommendation:** Option C for simplicity. Hold USDC on a reputable exchange or in a multisig with the stablecoin issuer's blessing.

### Reserve Ratio Monitoring

```rust
// Pseudocode for reserve monitoring

struct ReserveStatus {
    btc_issued: Satoshis,      // Total BTC tokens in circulation
    btc_reserves: Satoshis,    // Total BTC in all wallets
    usd_issued: Cents,         // Total USD tokens in circulation
    usd_reserves: Cents,       // Total USD/stablecoin held
}

impl ReserveStatus {
    fn btc_ratio(&self) -> f64 {
        self.btc_reserves as f64 / self.btc_issued as f64
    }

    fn usd_ratio(&self) -> f64 {
        self.usd_reserves as f64 / self.usd_issued as f64
    }

    fn is_healthy(&self) -> bool {
        self.btc_ratio() >= 1.0 && self.usd_ratio() >= 1.0
    }
}

// Alert if ratio drops below 100%
// This should NEVER happen in normal operations
// If it does, something is very wrong
```

### Proof of Reserves

Publish periodic attestations:

```
Monthly Reserve Attestation
Date: 2025-01-01
Auditor: [Third Party or Self-Attested]

BTC Reserves:
  Cold Storage (bc1q...): 50,000,000 sats
  Hot Wallet (channels): 20,000,000 sats
  Warm Wallet (bc1p...): 10,000,000 sats
  TOTAL: 80,000,000 sats

BTC Tokens Issued: 78,500,000 sats
Reserve Ratio: 101.9%

USD Reserves:
  USDC (0x...): $150,000.00
  Bank Account: $50,000.00
  TOTAL: $200,000.00

USD Tokens Issued: $195,000.00
Reserve Ratio: 102.6%

Merkle Root of All Proofs: 0xabc123...
Verification Instructions: [link]

Signed: [Operator Signature]
```

---

## Key Management

### Key Types

```
┌─────────────────────────────────────────────────────────────────┐
│                        KEY HIERARCHY                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  MINT KEYS (most critical)                                       │
│  ├── Signing Key: Signs all eCash tokens                        │
│  │   └── Compromise = can forge tokens = CATASTROPHIC           │
│  └── Keyset Keys: Rotated periodically                          │
│      └── Old keys still valid for redemption                    │
│                                                                  │
│  LIGHTNING KEYS                                                  │
│  ├── Node Identity: Persistent across restarts                  │
│  ├── Channel Keys: Per-channel, derived from seed               │
│  └── Macaroons: API access tokens                               │
│                                                                  │
│  RESERVE KEYS                                                    │
│  ├── Cold Storage: Multisig (2-of-3 or 3-of-5)                 │
│  ├── Warm Wallet: Single-sig with hardware wallet               │
│  └── Hot Wallet: LN node's internal wallet                      │
│                                                                  │
│  OPERATIONAL KEYS                                                │
│  ├── Database Encryption: Encrypts data at rest                 │
│  ├── API Signing: Signs API responses                           │
│  └── TLS Certificates: HTTPS termination                        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Mint Key Security

The mint signing key is the crown jewel. If compromised, an attacker can forge unlimited tokens.

**Storage options (in order of security):**

1. **HSM (Hardware Security Module)** — Best
   - Key never leaves the HSM
   - Signing operations happen inside
   - Expensive but worth it for production

2. **Hardware Wallet + Air Gap** — Good for cold operations
   - Sign keyset rotations offline
   - Not practical for hot mint operations

3. **Encrypted File + Secure Enclave** — Acceptable
   - Key encrypted at rest
   - Decrypted into memory at startup
   - Memory protections (mlock, no swap)

4. **Environment Variable** — MVP only
   - Quick to set up
   - Vulnerable to memory dumps, logs
   - Replace before production

### Multisig Setup for Reserves

**2-of-3 Configuration:**

```
Signer 1: Operator CEO/Founder
  - Hardware wallet (Coldcard)
  - Stored in home safe
  - Geographic: City A

Signer 2: Operator COO/Director
  - Hardware wallet (Foundation Passport)
  - Stored in bank safe deposit box
  - Geographic: City B

Signer 3: Independent Director or Custodian
  - Hardware wallet (BitBox02)
  - Stored in separate jurisdiction
  - Geographic: Country B
```

**Signing Procedure:**

```
1. Prepare transaction on coordinator laptop (view-only wallet)
2. Export PSBT (Partially Signed Bitcoin Transaction)
3. Signer 1 reviews and signs on air-gapped device
4. PSBT transferred to Signer 2 (email, Signal, USB)
5. Signer 2 reviews and signs
6. Coordinator combines signatures
7. Broadcast transaction
```

### Key Rotation

**Mint Keysets:** Rotate every 1-3 months

```
New Keyset Rotation Procedure:
1. Generate new keyset (new derivation path)
2. Add to mint's active keysets
3. Announce new keyset to users (NUT-07)
4. Old keyset remains valid for redemption
5. After grace period (30-90 days), old keyset stops issuing
6. Old keyset remains redeemable indefinitely
```

**Why rotate?**
- Limits exposure if a keyset is compromised
- Privacy: makes traffic analysis harder
- Allows protocol upgrades

### Backup and Recovery

```
BACKUP CHECKLIST:

□ Mint master seed
  - Written on metal plate
  - Stored in 2+ geographic locations
  - One copy with trusted third party

□ Lightning node seed + SCB
  - Encrypted cloud backup (auto)
  - Local encrypted copy (manual)
  - Test recovery annually

□ Database
  - Continuous replication to standby
  - Daily encrypted snapshots to cold storage
  - Monthly restore test

□ Operational secrets
  - Documented in encrypted vault (1Password, Bitwarden)
  - Known to 2+ authorized personnel
  - Bus factor > 1
```

---

## User Onboarding

### Account Models

**Option A: Accountless (Most Private)**

No user accounts. Anyone can:
- Deposit BTC → Receive tokens
- Redeem tokens → Receive BTC
- No KYC, no identity, no tracking

Pros: Maximum privacy, simplest operations
Cons: No recovery if user loses tokens, no user support

**Option B: Optional Accounts**

Users can optionally create accounts for:
- Lightning Address (`user@yourdomain.com`)
- Cloud backup of encrypted proofs
- Transaction history
- Higher limits

Account creation requires:
- Username (can be pseudonymous)
- Encryption pubkey (user-generated)
- Optional: email for notifications

**Option C: Required Accounts with Tiers**

```
TIER 0: Anonymous
  - No account required
  - Daily limit: $100 equivalent
  - No Lightning Address

TIER 1: Pseudonymous
  - Username + encryption key
  - Daily limit: $1,000 equivalent
  - Lightning Address enabled

TIER 2: Verified
  - Email verification
  - Phone verification (optional)
  - Daily limit: $10,000 equivalent

TIER 3: Full KYC (optional/enterprise)
  - Government ID
  - Proof of address
  - No daily limit
  - Enterprise features
```

### Onboarding Flow (Tier 1 Example)

```
1. User visits wallet or downloads app

2. User generates keypair locally
   └── Mnemonic never leaves device
   └── Encryption pubkey derived

3. User chooses username
   └── Check availability against database
   └── Reserve username

4. User submits registration
   └── Username + encryption pubkey
   └── No password (key-based auth)

5. System creates account
   └── Store: { username, pubkey, created_at }
   └── Generate Lightning Address

6. User receives confirmation
   └── Lightning Address active
   └── Ready to receive deposits
```

### Terms of Service

Even without regulations, you need clear terms:

```markdown
# Terms of Service (Template)

## What We Do
We operate a Cashu eCash mint backed by Bitcoin reserves.

## Your Responsibilities
- Secure your tokens (we cannot recover them)
- Comply with laws in YOUR jurisdiction
- Don't use our service for illegal purposes

## Our Responsibilities
- Maintain 100% reserves at all times
- Process redemptions within [X hours]
- Publish regular proof of reserves
- Notify you of any security incidents

## What We Don't Do
- We don't provide investment advice
- We don't guarantee BTC/USD exchange rates
- We don't offer credit or loans
- We don't comply with regulations we're not subject to

## Prohibited Uses
- Money laundering
- Terrorism financing
- Sanctions evasion
- Child exploitation
- [Other obviously illegal activities]

## Limitation of Liability
[Standard limitation language]

## Dispute Resolution
[Your jurisdiction's arbitration/mediation]

## Changes to Terms
We may update these terms. Continued use = acceptance.
```

---

## Operational Procedures

### Daily Operations

```
DAILY CHECKLIST:

□ Morning (start of business)
  ├── Check system health dashboard
  ├── Review overnight alerts
  ├── Verify reserve ratios
  └── Check Lightning channel status

□ Continuous
  ├── Monitor error rates
  ├── Respond to support tickets
  ├── Process manual interventions (if any)
  └── Watch for anomalies

□ Evening (end of business)
  ├── Review day's transactions
  ├── Confirm backups completed
  ├── Check reserve movements
  └── Document any incidents
```

### Incident Response

```
SEVERITY LEVELS:

SEV 1 - Critical (respond immediately)
  - Reserve shortfall detected
  - Mint key suspected compromised
  - Major security breach
  - Complete service outage
  Response: All hands, executive notification, consider halt

SEV 2 - High (respond within 1 hour)
  - Partial service outage
  - Lightning connectivity issues
  - Significant error rate spike
  - Attempted breach detected
  Response: On-call engineer, manager notification

SEV 3 - Medium (respond within 4 hours)
  - Single component degraded
  - Non-critical errors elevated
  - User-reported issues (multiple)
  Response: On-call engineer

SEV 4 - Low (respond within 24 hours)
  - Minor bugs
  - Single user issues
  - Non-urgent maintenance
  Response: Normal queue
```

### Emergency Procedures

**Reserve Shortfall:**

```
1. HALT all new issuance immediately
2. CONTINUE processing redemptions
3. INVESTIGATE cause:
   - Accounting error?
   - Theft?
   - Technical bug?
4. NOTIFY users if shortfall confirmed
5. REMEDIATE:
   - Inject capital if available
   - Or wind down operations
6. POST-MORTEM with full disclosure
```

**Key Compromise Suspected:**

```
1. ROTATE compromised key immediately
2. HALT operations using that key
3. ASSESS damage:
   - Any unauthorized signatures?
   - Any forged tokens?
4. NOTIFY affected users
5. IMPLEMENT additional security
6. FULL DISCLOSURE in transparency report
```

**Natural Disaster / Infrastructure Loss:**

```
1. ACTIVATE backup site (if available)
2. RESTORE from most recent backup
3. VERIFY data integrity
4. RECONCILE any gaps:
   - Compare Lightning state
   - Verify reserve balances
5. RESUME operations
6. DOCUMENT lessons learned
```

---

## Self-Regulatory Framework

Since your jurisdiction has no existing regulations, you define your own standards. This is an opportunity to demonstrate responsible operation and potentially influence future regulation.

### Principles

1. **User Protection** — Act in users' interests even when not required
2. **Transparency** — Publish what others hide
3. **Soundness** — 100% reserves, no fractional games
4. **Privacy** — Collect minimum necessary data
5. **Security** — Invest in protection before incidents occur

### Voluntary Standards

**Reserve Standards:**
- Maintain ≥100% reserves at all times
- Publish monthly proof of reserves
- Third-party audit annually
- Real-time reserve ratio on dashboard (optional)

**Operational Standards:**
- 99.9% uptime target
- <1 hour redemption processing (business hours)
- <24 hour redemption processing (any time)
- Incident disclosure within 72 hours

**Privacy Standards:**
- Minimal data collection
- No sale of user data
- Encryption at rest and in transit
- Data deletion upon request (where possible)

**Security Standards:**
- Annual penetration testing
- Bug bounty program
- Secure development practices
- Key management audits

### Self-Regulatory Body

Consider forming or joining an industry association:

```
NEOBANK OPERATORS ASSOCIATION (example)

Members: Operators in innovation-friendly jurisdictions

Functions:
- Develop shared standards
- Peer review and audit
- Incident information sharing
- Advocacy with regulators
- Consumer complaint handling

Membership Requirements:
- 100% reserve commitment
- Transparency reporting
- Security standards compliance
- Dispute resolution participation
```

---

## Transparency and Auditing

### Public Disclosures

**Real-Time (Dashboard):**
- System status (up/degraded/down)
- Recent block height (proves liveness)
- Active keysets

**Monthly:**
- Reserve attestation with merkle proof
- Transaction volume (aggregate)
- User count (if accounts exist)
- Incident summary

**Quarterly:**
- Financial summary
- Security audit results (high-level)
- Roadmap update
- Regulatory engagement summary

**Annually:**
- Full reserve audit (third-party)
- Security penetration test results
- Complete financial statements
- Governance report

### Audit Types

**Reserve Audit:**
```
What's verified:
- On-chain BTC balances (provable)
- Lightning channel balances (harder, requires cooperation)
- USD/stablecoin balances (bank statements, exchange accounts)
- Total tokens issued (from mint database)
- Reserve ratio calculation

Who can do it:
- Reputable crypto audit firm
- Traditional accounting firm with crypto expertise
- Or: Community auditors with published methodology
```

**Security Audit:**
```
What's tested:
- Mint software for vulnerabilities
- Infrastructure configuration
- Key management practices
- Incident response procedures
- Social engineering resistance

Who can do it:
- Specialized crypto security firms
- Traditional penetration testing firms
- Bug bounty hunters (ongoing)
```

**Operational Audit:**
```
What's reviewed:
- Policies and procedures
- Staff training records
- Incident logs and responses
- Backup and recovery tests
- Business continuity plans

Who can do it:
- Internal audit function
- External consultants
- Peer operators (reciprocal)
```

### Transparency Report Template

```markdown
# Neobank Transparency Report
Period: Q1 2025

## Operations Summary
- Days operational: 90
- Uptime: 99.95%
- Transactions processed: 125,000
- Unique users: 8,500

## Reserve Status
- BTC reserves: 150,000,000 sats
- BTC tokens issued: 147,500,000 sats
- BTC reserve ratio: 101.7%
- USD reserves: $500,000
- USD tokens issued: $485,000
- USD reserve ratio: 103.1%
- [Link to merkle proof]

## Incidents
- SEV 1: 0
- SEV 2: 1 (Lightning node restart, 15 min outage)
- SEV 3: 3 (various, see appendix)
- Security incidents: 0

## Regulatory Engagement
- Met with [Jurisdiction] Finance Ministry
- Submitted feedback on proposed framework
- No enforcement actions

## Security
- Penetration test: Passed (see summary)
- Bug bounties paid: $2,500 (3 reports)
- No breaches

## Looking Ahead
- Planned upgrades: [list]
- New features: [list]
- Challenges: [honest assessment]

Signed: [Operator]
Published: [Date]
```

---

## Risk Management

### Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Key compromise | Low | Critical | HSM, multisig, rotation |
| Reserve shortfall | Very Low | Critical | Daily reconciliation, alerts |
| Lightning channel depletion | Medium | High | Rebalancing, reserve liquidity |
| DDoS attack | Medium | Medium | CDN, rate limiting |
| Regulatory action (other jurisdictions) | Medium | Medium | Geo-blocking, legal structure |
| Staff fraud | Low | High | Separation of duties, audits |
| Software bug | Medium | Variable | Testing, staged rollouts |
| Vendor failure (exchange, LSP) | Low | High | Diversification |

### Insurance

Consider (if available in your jurisdiction):
- **Cyber insurance** — Covers breach costs
- **Crime insurance** — Covers theft by employees
- **Professional liability** — Covers operational errors
- **Directors & Officers** — Covers personal liability

Note: Crypto-specific insurance is limited and expensive. Self-insurance (reserve buffer) may be more practical.

### Business Continuity

```
CONTINUITY SCENARIOS:

Scenario: Primary datacenter loss
Response: Failover to backup site within 4 hours
Recovery: Full operations within 24 hours

Scenario: Key person unavailable
Response: Documented procedures, cross-trained staff
Recovery: No single point of failure for operations

Scenario: Banking partner terminates relationship
Response: Backup banking relationships pre-established
Recovery: USD operations may pause, BTC continues

Scenario: Regulatory pressure from user jurisdictions
Response: Geo-blocking, legal review, user notification
Recovery: Operations continue for non-affected jurisdictions
```

---

## Working with Your Jurisdiction

### Building the Relationship

Your jurisdiction wants to learn from you. Be a good partner:

1. **Proactive Communication**
   - Regular updates to relevant officials
   - Invite them to observe operations
   - Share anonymized data for policy development

2. **Policy Input**
   - Propose practical frameworks
   - Highlight what works in other jurisdictions
   - Explain technical constraints honestly

3. **Economic Contribution**
   - Hire locally where possible
   - Pay applicable taxes/fees
   - Support local Bitcoin/crypto community

4. **Responsible Operation**
   - Don't embarrass them
   - Handle incidents professionally
   - Maintain high standards voluntarily

### Regulatory Framework Suggestions

Propose frameworks that work for both sides:

**Licensing Approach:**
```
Tier 1: Notification Only
- <$1M monthly volume
- Basic registration
- Annual self-attestation

Tier 2: Light Touch License
- $1M-$10M monthly volume
- Reserve audit annually
- Basic reporting requirements

Tier 3: Full License
- >$10M monthly volume
- Quarterly reserve audits
- Enhanced reporting
- Capital requirements
```

**Consumer Protection:**
```
- Mandatory disclosure of risks
- Clear fee schedules
- Dispute resolution mechanism
- No misleading marketing
```

**AML/CTF (if required):**
```
- Risk-based approach
- Thresholds appropriate to risk
- Focus on actual illicit activity
- Not blanket surveillance
```

### What to Avoid

- Don't promise things you can't deliver
- Don't hide problems
- Don't badmouth other jurisdictions
- Don't lobby for protectionism
- Don't become a regulatory capture vehicle

---

## Launch Checklist

### Pre-Launch (T-30 days)

```
LEGAL
□ Entity registered in jurisdiction
□ Beneficial ownership documented
□ Terms of service drafted
□ Privacy policy drafted
□ Legal opinion on structure (if desired)

TECHNICAL
□ Infrastructure provisioned
□ Mint software deployed and tested
□ Lightning node operational
□ Channels opened and balanced
□ Database configured with backups
□ Monitoring and alerting set up
□ Security audit completed (or scheduled)

OPERATIONAL
□ Procedures documented
□ Staff trained
□ Support channels established
□ Incident response tested
□ Backup/recovery tested

RESERVES
□ Initial capital deposited
□ Cold storage setup complete
□ Multisig tested
□ Reserve monitoring active
```

### Soft Launch (T-7 days)

```
□ Internal testing complete
□ Invite beta testers (trusted users)
□ Monitor closely for issues
□ Fix critical bugs
□ Document edge cases
□ Adjust procedures as needed
```

### Public Launch (T-0)

```
□ Announcement prepared
□ Support team on standby
□ All hands available
□ Monitoring dashboard visible
□ Communication channels open
□ Rollback plan ready (just in case)
```

### Post-Launch (T+7 days)

```
□ Review all incidents
□ Gather user feedback
□ Assess performance metrics
□ Adjust capacity if needed
□ Begin normal operations cadence
□ Schedule first transparency report
```

---

## Support and Resources

### From OpenAgents

- Technical documentation: [docs.openagents.com]
- Operator Slack/Discord: [invite link]
- Security advisories: [mailing list]
- Software updates: [release notes]

### Community Resources

- Cashu Protocol: https://cashu.space
- Lightning Dev Kit: https://lightningdevkit.org
- Bitcoin Optech: https://bitcoinops.org
- Nostr Resources: https://nostr.com/resources

### Professional Services

For operators wanting additional support:
- Security audits: [recommended firms]
- Legal advice: [crypto-friendly counsel]
- Infrastructure: [hosting providers]
- Insurance: [crypto-specialized brokers]

---

## Conclusion

Operating a neobank is not simple, but it's tractable. The technology works. The economics work. The remaining challenges are operational: key management, reserve management, user support, and regulatory relationships.

Your jurisdiction is giving you an opportunity to demonstrate what responsible innovation looks like. Take that seriously. Build something trustworthy. Document what you learn. Share your experience.

The agent economy needs this infrastructure. Build it well.

---

*This guide is provided by OpenAgents Inc. for informational purposes. It does not constitute legal, financial, or regulatory advice. Operators are responsible for their own compliance and operational decisions.*
