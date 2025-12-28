# Agicash Research

**Analysis of MakePrisms/agicash for OpenAgents Neobank integration**

Agicash (formerly Boardwalk Cash) is a self-custodial eCash wallet implementing the Cashu protocol with Lightning integration. It's particularly relevant to our neobank vision because it has working implementations of several primitives we need.

---

## Executive Summary

### What Agicash Does Well (Yoink Candidates)

1. **Dual Account System** - Cashu eCash + Spark Lightning in one app
2. **Money Library** - Excellent multi-currency handling with BTC/USD, unit conversions
3. **Quote/Transaction Flow** - Well-architected send/receive with state machines
4. **Exchange Rate Service** - Multi-provider fallback (Mempool, Coingecko, Coinbase)
5. **ECIES Encryption** - Client-side encryption for sensitive data
6. **Lightning Address (LNURL)** - Full LUD-16 implementation with verify callbacks

### What We Should Build Differently

1. **No Threshold Signatures** - They use single-key custodial model per mint
2. **No Policy Engine** - No programmable budgets or approval workflows
3. **Web-First** - React Router/Vercel; we need native Rust
4. **No Agent Support** - Human-only UX; no programmatic API

---

## Architecture Overview

### Tech Stack

```
Frontend:     React 19 + React Router 7 (framework mode)
State:        Zustand + React Query
Database:     Supabase (Postgres) with real-time subscriptions
Auth/Keys:    Open Secret platform (hosted key management)
Build:        Bun + Vite + Biome
Deployment:   Vercel
```

### Key Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `@buildonspark/spark-sdk` | 0.5.1 | Spark L2 Bitcoin payments |
| `@cashu/cashu-ts` | 2.6.0 | Cashu eCash protocol |
| `@cashu/crypto` | 0.3.4 | Cashu cryptographic primitives |
| `@noble/ciphers` | 1.3.0 | ChaCha20-Poly1305, XChaCha |
| `@noble/curves` | 1.9.7 | secp256k1, ECDH |
| `@noble/hashes` | 1.8.0 | SHA-256, HKDF |
| `@scure/bip32` | 1.7.0 | HD key derivation |
| `@scure/bip39` | 1.6.0 | Mnemonic handling |

---

## Account Model

### Account Types

```typescript
type AccountType = 'cashu' | 'spark';

type Account = {
  id: string;
  name: string;
  type: AccountType;
  isOnline: boolean;
  currency: Currency;  // 'BTC' | 'USD'
  createdAt: string;
  version: number;     // Optimistic locking
} & (CashuAccountDetails | SparkAccountDetails);
```

### Cashu Account Details

```typescript
type CashuAccountDetails = {
  type: 'cashu';
  mintUrl: string;
  isTestMint: boolean;
  keysetCounters: Record<string, number>;  // Per-keyset counters for deterministic secrets
  proofs: CashuProof[];
  wallet: ExtendedCashuWallet;
};
```

### Spark Account Details

```typescript
type SparkAccountDetails = {
  type: 'spark';
  balance: Money | null;
  network: SparkNetwork;  // 'MAINNET' | 'TESTNET' | 'REGTEST'
  wallet: SparkWallet;
};
```

### Key Insight: Multi-Currency Support

Agicash uniquely supports **USD stablecoins** via Cashu mints that issue dollar-denominated eCash. The mint `https://stablenut.cashu.network` is their recommended USD mint.

This is exactly what our neobank research identified as needed for "stable unit-of-account" in agent budgets.

---

## Money Library

**Location:** `app/lib/money/money.ts`

This is production-quality code worth studying/adapting.

### Design

```typescript
class Money<T extends Currency = Currency> {
  private readonly _data: MoneyData<T>;

  // Immutable - constructor freezes
  constructor(data: MoneyInput<T>) { ... }

  // Static factories
  static sum<T>(moneys: Money<T>[], currency?: T): Money<T>;
  static max<T>(moneys: Money<T>[]): Money<T>;
  static min<T>(moneys: Money<T>[]): Money<T>;
  static zero(currency: Currency): Money;
  static createMinAmount<T>(currency: T, unit?: CurrencyUnit<T>): Money<T>;

  // Arithmetic (returns new instance)
  add(money: Money<T>): Money<T>;
  subtract(money: Money<T>): Money<T>;
  multiply(factor: NumberInput): Money<T>;
  divide(divisor: NumberInput): Money<T>;
  abs(): Money<T>;

  // Comparison
  equals(money: Money<T>): boolean;
  greaterThan(money: Money<T>): boolean;
  lessThan(money: Money<T>): boolean;
  isZero(): boolean;
  isPositive(): boolean;
  isNegative(): boolean;

  // Conversion
  amount(unit?: CurrencyUnit<T>): Big;
  convert<U extends Currency>(currency: U, exchangeRate: NumberInput): Money<U>;

  // Formatting
  toLocaleString(options?): string;
  toLocalizedStringParts(options?): LocalizedStringParts;
}
```

### Currency Units

```typescript
const currencyDataMap = {
  USD: {
    baseUnit: 'usd',
    units: [
      { name: 'usd', decimals: 2, symbol: '$', factor: Big(1) },
      { name: 'cent', decimals: 0, symbol: '¢', factor: Big(10**-2) },
    ],
  },
  BTC: {
    baseUnit: 'btc',
    units: [
      { name: 'btc', decimals: 8, symbol: '₿', factor: Big(1) },
      { name: 'sat', decimals: 0, symbol: '₿', factor: Big(10**-8) },
      { name: 'msat', decimals: 0, symbol: 'msat', factor: Big(10**-11) },
    ],
  },
};
```

### Why This Matters

The Money class handles:
- Internal storage in smallest unit (sat/cent) to avoid float errors
- Proper rounding on division
- Currency-aware formatting with localization
- Type-safe currency operations (can't add USD to BTC)

**Recommendation:** Port this to Rust for our neobank crate.

---

## Cashu Implementation

### Extended Wallet

**Location:** `app/lib/cashu/utils.ts`

They extend `CashuWallet` from `@cashu/cashu-ts`:

```typescript
class ExtendedCashuWallet extends CashuWallet {
  private _bip39Seed: Uint8Array | undefined;

  get seed(): Uint8Array { ... }

  // Fee estimation for receiving
  getFeesEstimateToReceiveAtLeast(amount: number | Big): number;

  // Idempotent melt (handles retry scenarios)
  async meltProofsIdempotent(
    meltQuote: MeltQuoteResponse,
    proofs: Proof[],
    options?: Parameters<CashuWallet['meltProofs']>[2]
  ): Promise<...>;
}
```

### Protocol Extensions

**Location:** `app/lib/cashu/PROTOCOL_EXTENSIONS.md`

Agicash has a fork of CDK (Cashu Dev Kit) with custom extensions:

```json
{
  "agicash": {
    "minting_fee": {
      "type": "basis_points",
      "value": 100
    }
  }
}
```

This extends NUT-06 mint info with minting fees (1% = 100 basis points).

### Proof Management

Proofs are the "coins" in eCash. Agicash stores them encrypted:

```typescript
type CashuProof = {
  id: string;
  accountId: string;
  userId: string;
  keysetId: string;
  amount: number;
  secret: string;           // Encrypted
  unblindedSignature: string;
  publicKeyY: string;
  dleq: Proof['dleq'];
  witness: Proof['witness'];
  state: 'UNSPENT' | 'RESERVED' | 'SPENT';
  version: number;
  createdAt: string;
  reservedAt?: string | null;
  spentAt?: string | null;
};
```

---

## Quote System

### Send Quote Flow (Cashu)

**Location:** `app/features/send/`

```
User enters amount/destination
        ↓
cashu-send-quote-service.ts creates quote
        ↓
Quote stored in DB with state: 'UNPAID'
        ↓
User confirms
        ↓
cashu-send-quote-hooks.ts executes melt
        ↓
State transitions: UNPAID → PENDING → PAID/FAILED
        ↓
Transaction record created
```

**Quote Type:**

```typescript
type CashuSendQuote = {
  id: string;
  createdAt: string;
  expiresAt: string;
  userId: string;
  accountId: string;
  paymentRequest: string;       // BOLT11 invoice
  amountRequested: Money;
  amountRequestedInMsat: number;
  amountToReceive: Money;
  lightningFeeReserve: Money;
  cashuFee: Money;
  quoteId: string;              // Mint's melt quote ID
  proofs: CashuProof[];         // Reserved proofs for this payment
  keysetId: string;
  keysetCounter: number;
  numberOfChangeOutputs: number;
  state: 'UNPAID' | 'PENDING' | 'PAID' | 'EXPIRED' | 'FAILED';
  version: number;
  transactionId: string;
};
```

### Receive Quote Flow (Cashu)

```
User requests receive
        ↓
cashu-receive-quote-service.ts creates mint quote
        ↓
Returns BOLT11 invoice
        ↓
Mint quote subscription monitors for payment
        ↓
On payment: mint proofs
        ↓
Store new proofs encrypted
        ↓
Transaction record created
```

---

## Spark Integration

**Location:** `app/lib/spark/` + `app/features/shared/spark.ts`

### Wallet Initialization

```typescript
const sparkWalletQueryOptions = ({ network, mnemonic }) =>
  queryOptions({
    queryKey: ['spark-wallet', network],
    queryFn: async ({ client }) => {
      const { wallet } = await SparkWallet.initialize({
        mnemonicOrSeed: mnemonic,
        options: { network },
      });
      // Enable privacy mode (hide from explorers)
      await wallet.setPrivacyEnabled(true);
      return wallet;
    },
    staleTime: Infinity,
    gcTime: Infinity,
  });
```

### Key Derivation

```typescript
// Derive Spark identity pubkey from mnemonic
async function getSparkIdentityPublicKeyFromMnemonic(
  mnemonic: string,
  network: NetworkType,
  accountNumber?: number
): Promise<string> {
  const signer = new DefaultSparkSigner();
  const seed = await signer.mnemonicToSeed(mnemonic);

  // Account number: 0 for regtest, 1 for others
  const accountNum = accountNumber ?? (network === 'REGTEST' ? 0 : 1);
  await signer.createSparkWalletFromSeed(seed, accountNum);

  const publicKey = await signer.getIdentityPublicKey();
  return bytesToHex(publicKey);
}
```

---

## Exchange Rate Service

**Location:** `app/lib/exchange-rate/`

### Multi-Provider Design

```typescript
class ExchangeRateService {
  private providers: ExchangeRateProvider[];

  constructor(providers?: ExchangeRateProvider[]) {
    // Priority order - first successful response wins
    this.providers = providers ?? [
      new MempoolSpace(),  // BTC-USD only
      new Coingecko(),     // BTC-USD only
      new Coinbase(),      // BTC-USD only
    ];
  }

  async getRates({ tickers, signal }: GetRatesParams): Promise<Rates> {
    // Find providers supporting all requested tickers
    // Try in priority order until one succeeds
    // Return cached timestamp with rates
  }
}
```

### Ticker Format

```typescript
type Ticker = 'BTC-USD' | 'USD-BTC' | 'BTC-BTC' | 'USD-USD';
```

**Recommendation:** We need this for agent budgets denominated in USD but paid in BTC.

---

## Encryption Architecture

**Location:** `app/features/shared/encryption.ts`

### ECIES Encryption

All sensitive data (proof secrets, transaction details) is encrypted client-side:

```typescript
// Encrypt to user's public key
function encryptToPublicKey<T>(data: T, publicKeyHex: string): string {
  const serialized = serializeData(data);  // Handles Money, Date, etc.
  const dataBytes = encoder.encode(serialized);
  const encryptedBytes = eciesEncrypt(dataBytes, publicKeyBytes);
  return base64Encode(encryptedBytes);
}

// Decrypt with user's private key
function decryptWithPrivateKey<T>(encryptedData: string, privateKeyBytes: Uint8Array): T {
  const encryptedBytes = base64Decode(encryptedData);
  const decryptedBytes = eciesDecrypt(encryptedBytes, privateKeyBytes);
  return deserializeData(decoder.decode(decryptedBytes));
}
```

### Batch Encryption

For efficiency, they support batch encryption with shared ephemeral key:

```typescript
function encryptBatchToPublicKey<T extends readonly unknown[]>(
  data: T,
  publicKeyHex: string
): string[] {
  // Same ephemeral key for whole batch = faster
  // But items are linkable (same batch)
}
```

### Key Derivation Path

```typescript
// Encryption key: m/10111099'/0'  (10111099 = 'enc' in ASCII)
const encryptionKeyDerivationPath = `m/10111099'/0'`;
```

---

## Lightning Address (LNURL)

**Location:** `app/features/receive/lightning-address-service.tsx`

### LUD-16 Implementation

```typescript
class LightningAddressService {
  // Handle /.well-known/lnurlp/{username}
  async handleLud16Request(username: string): Promise<LNURLPayParams | LNURLError> {
    const user = await this.userRepository.getByUsername(username);
    if (!user) return { status: 'ERROR', reason: 'not found' };

    return {
      callback: `${this.baseUrl}/api/lnurlp/callback/${user.id}`,
      maxSendable: this.maxSendable.toNumber('msat'),
      minSendable: this.minSendable.toNumber('msat'),
      metadata: JSON.stringify([
        ['text/plain', `Pay to ${address}`],
        ['text/identifier', address],
      ]),
      tag: 'payRequest',
    };
  }

  // Handle callback with amount
  async handleLnurlpCallback(userId: string, amount: Money<'BTC'>): Promise<LNURLPayResult> {
    // Get user's default account
    // Create receive quote (Cashu or Spark)
    // Return invoice with verify URL
  }

  // Verify payment status
  async handleLnurlpVerify(encryptedQuoteData: string): Promise<LNURLVerifyResult> {
    // Decrypt quote data
    // Check mint quote or Spark receive request status
    // Return settled: true/false
  }
}
```

### Cross-Currency Support

Unique feature: They handle USD-denominated accounts receiving BTC payments:

```typescript
let amountToReceive: Money = amount;
if (amount.currency !== account.currency) {
  const rate = await this.exchangeRateService.getRate(
    `${amount.currency}-${account.currency}`
  );
  amountToReceive = amount.convert(account.currency, rate);
}
```

---

## Database Schema

### Tables (from migrations)

```sql
-- Core tables
accounts        -- User accounts (Cashu/Spark)
cashu_proofs    -- eCash proofs (encrypted)
transactions    -- All payment records

-- Quote tables
cashu_receive_quotes
cashu_send_quotes
cashu_send_swaps     -- Internal transfers between accounts
spark_receive_quotes
spark_send_quotes

-- Supporting tables
contacts        -- User contact book
task_processing_locks  -- Distributed locking for background tasks
```

### Transaction Types

```typescript
type TransactionType =
  | 'CASHU_LIGHTNING'   // Cashu → Lightning (melt)
  | 'CASHU_TOKEN'       // Cashu → Cashu (token send)
  | 'SPARK_LIGHTNING';  // Spark → Lightning

type TransactionDirection = 'SEND' | 'RECEIVE';
type TransactionState = 'PENDING' | 'COMPLETED' | 'FAILED';
```

---

## What to Yoink

### 1. Money Library (High Priority)

Port the Money class to Rust:

```rust
pub struct Money<C: Currency> {
    amount: BigDecimal,  // In smallest unit
    currency: PhantomData<C>,
}

impl<C: Currency> Money<C> {
    pub fn add(&self, other: &Money<C>) -> Money<C>;
    pub fn convert<D: Currency>(&self, rate: BigDecimal) -> Money<D>;
    pub fn format(&self, locale: &str) -> String;
}
```

### 2. Exchange Rate Service (High Priority)

```rust
pub struct ExchangeRateService {
    providers: Vec<Box<dyn ExchangeRateProvider>>,
}

pub trait ExchangeRateProvider {
    fn supported_pairs(&self) -> &[CurrencyPair];
    async fn get_rate(&self, pair: CurrencyPair) -> Result<ExchangeRate>;
}
```

### 3. Quote State Machine (Medium Priority)

The quote lifecycle with state transitions and optimistic locking is well-designed:

```
UNPAID → PENDING → PAID
           ↓
        FAILED
           ↓
        EXPIRED
```

### 4. Cashu Protocol Types (Medium Priority)

Their NUT-10 secret handling, proof types, and protocol extension patterns.

### 5. LNURL Implementation (Lower Priority)

For agent payment profiles, the Lightning Address server implementation is valuable.

---

---

## Patterns to Extract (Missed in Initial Analysis)

### 1. Idempotency Keys Everywhere

Every send must have an idempotency key derived from deterministic inputs:

```rust
/// Idempotency key for safe retries after crash
fn idempotency_key(account_id: &str, quote_id: &str, attempt: u32) -> String {
    format!("{}-{}-{}", account_id, quote_id, attempt)
}

/// Melt is safe to retry - mint tracks by quote ID
pub async fn melt_proofs_idempotent(
    &self,
    quote: &MeltQuote,
    proofs: &[Proof],
) -> Result<MeltResult> {
    // If quote already settled, mint returns success
    // If quote failed, mint returns error
    // If quote pending, mint completes or returns status
    self.wallet.melt_proofs(quote, proofs).await
}
```

**Critical:** Agicash's `meltProofsIdempotent` wrapper handles the case where invoice is already paid.

### 2. Crash Recovery Loop

On startup, scan for incomplete operations:

```rust
impl CrashRecovery {
    /// Run on every startup to resolve incomplete state
    pub async fn recover(&mut self) -> Result<RecoveryReport> {
        let mut report = RecoveryReport::default();

        // 1. Find PENDING quotes
        for quote in self.get_pending_quotes().await? {
            match self.poll_mint_quote_status(&quote).await? {
                MintQuoteStatus::Paid => {
                    // Invoice was paid, mint the proofs
                    self.mint_proofs(&quote).await?;
                    report.mints_completed += 1;
                }
                MintQuoteStatus::Unpaid => {
                    if quote.is_expired() {
                        self.mark_quote_expired(&quote).await?;
                        report.quotes_expired += 1;
                    }
                }
                MintQuoteStatus::Pending => {
                    // Still in flight, will check again later
                }
            }
        }

        // 2. Find RESERVED proofs without active quote
        for proof in self.get_orphaned_reservations().await? {
            self.release_reservation(&proof).await?;
            report.reservations_released += 1;
        }

        // 3. Find PENDING proofs (melt started but not confirmed)
        for proof in self.get_pending_proofs().await? {
            // Check if the melt completed
            if let Some(preimage) = self.check_melt_status(&proof).await? {
                self.mark_proof_spent(&proof, preimage).await?;
                report.melts_confirmed += 1;
            }
        }

        Ok(report)
    }
}
```

### 3. Deterministic Secret Generation

Per-keyset counters enable wallet recovery:

```rust
pub struct KeysetCounters {
    counters: HashMap<KeysetId, u64>,
}

impl KeysetCounters {
    /// Generate next secret for a keyset.
    /// CRITICAL: Counter must be persisted BEFORE using the secret.
    pub fn next_secret(&mut self, keyset_id: &KeysetId, seed: &[u8]) -> Result<Secret> {
        let counter = self.counters.entry(keyset_id.clone()).or_insert(0);
        let secret = derive_secret_from_seed(seed, keyset_id, *counter)?;

        // Persist counter FIRST (before network call)
        *counter += 1;

        Ok(secret)
    }
}

/// BIP-32 derivation for Cashu secrets
/// Path: m/129372'/0'/{keyset_id}'/{counter}
fn derive_secret_from_seed(seed: &[u8], keyset_id: &KeysetId, counter: u64) -> Secret {
    let path = format!("m/129372'/0'/{}'/{}", keyset_id.as_u64(), counter);
    let key = derive_key(seed, &path);
    Secret::from_bytes(key.secret_bytes())
}
```

**Why this matters:** If counters aren't persisted before use, a crash could cause secret reuse (security issue) or lost proofs (funds loss).

### 4. Fee Modeling

Agicash tracks fees explicitly:

```rust
pub struct FeeModel {
    /// Fixed fee in smallest unit
    pub fixed: Amount,
    /// Variable fee in basis points
    pub rate_bps: u16,
    /// Minimum fee
    pub min: Amount,
    /// Maximum fee (cap)
    pub max: Option<Amount>,
}

impl FeeModel {
    pub fn calculate(&self, amount: Amount) -> Amount {
        let variable = amount.basis_points(self.rate_bps);
        let total = self.fixed.add(&variable);

        // Apply bounds
        let fee = total.max(&self.min);
        match &self.max {
            Some(max) => fee.min(max),
            None => fee,
        }
    }
}

/// Surfaces in quote preview AND receipt
pub struct QuoteWithFees {
    pub amount_requested: Amount,
    pub fee_breakdown: FeeBreakdown,
    pub amount_after_fees: Amount,
}

pub struct FeeBreakdown {
    pub routing_fee: Amount,     // LN routing
    pub minting_fee: Amount,     // Mint's fee (from NUT-06 extension)
    pub service_fee: Amount,     // Our fee (if any)
}
```

---

## Portability: DB vs Relay State

Agicash stores everything in Supabase (centralized DB). OpenAgents needs:

**Local SQLite as source of truth:**
- Proofs, quotes, transactions stored locally
- Works offline (read-only balance, history)
- Fast for budget checks and balance queries

**Optional relay backup (NIP-60):**
- Encrypted token events for portability
- Enables wallet recovery on new device
- Agent can migrate between machines

```rust
pub struct WalletSync {
    local: SqlitePool,
    relay: Option<RelayConnection>,
}

impl WalletSync {
    /// Sync local → relay (backup)
    pub async fn push_to_relay(&self) -> Result<()> {
        let unsynced = self.get_unsynced_proofs().await?;
        for proof in unsynced {
            let event = self.create_token_event(&proof)?;  // NIP-60
            self.relay.as_ref().unwrap().publish(event).await?;
            self.mark_synced(&proof).await?;
        }
        Ok(())
    }

    /// Sync relay → local (recovery)
    pub async fn pull_from_relay(&self) -> Result<()> {
        let events = self.relay.as_ref().unwrap()
            .query_wallet_events(self.pubkey).await?;

        for event in events {
            if !self.has_proof(&event.proof_id).await? {
                let proof = self.decrypt_proof(&event)?;
                self.store_proof(proof).await?;
            }
        }
        Ok(())
    }
}
```

---

## USD Cashu Mint Risk Profile

Treating USD mints as "stablecoins exist today" understates the risk:

### Risk Stack

```
┌─────────────────────────────────────────┐
│  Your exposure                          │
├─────────────────────────────────────────┤
│  Cashu proof → Mint solvency            │  ← If mint rugs, proofs worthless
│  Mint → BTC reserves                    │  ← Mint absorbs volatility
│  Mint → Operational continuity          │  ← Mint goes down, can't redeem
│  Mint → Honest spent-secret tracking    │  ← Mint double-spends, you lose
└─────────────────────────────────────────┘
```

### Concentrated Risk

Unlike BTC Cashu (where you just trust the mint), USD Cashu has:

1. **Issuer risk** — Mint is making markets, absorbing volatility
2. **FX risk** — BTC/USD rate matters for mint's reserve ratio
3. **Operational risk** — More complex to run than pure BTC mint

### Mitigation Policy

```rust
pub struct UsdMintPolicy {
    /// Maximum USD exposure at any single mint
    pub max_per_mint: Amount,

    /// Maximum total USD across all mints (as % of total treasury)
    pub max_total_pct: u8,

    /// Auto-diversify when exceeding threshold
    pub auto_diversify_at_pct: u8,

    /// Minimum mint reputation score (from NIP-87)
    pub min_reputation: f32,

    /// Required reserve proof frequency
    pub require_reserve_proof_days: u16,
}

impl Default for UsdMintPolicy {
    fn default() -> Self {
        Self {
            max_per_mint: Amount::from_cents(50_000),     // $500 max per mint
            max_total_pct: 30,                            // Max 30% of treasury in USD
            auto_diversify_at_pct: 20,                    // Diversify at 20%
            min_reputation: 0.8,                          // 80%+ positive attestations
            require_reserve_proof_days: 30,               // Monthly reserve proofs
        }
    }
}
```

**Default stance:** USD Cashu is useful but treat it as higher-risk than BTC Cashu. Cap exposure, diversify, monitor mint health.

---

## What NOT to Copy

### 1. Key Management

They use Open Secret (hosted KMS) which is custodial. We use FROST threshold signatures.

### 2. Web Architecture

React/Supabase doesn't fit our Rust-native stack.

### 3. No Policy Engine

They have no concept of:
- Spending limits
- Approval workflows
- Per-agent budgets
- Policy signers

### 4. Single-User Model

No multi-agent, no hierarchical accounts, no treasury management.

---

## Integration Path

### Phase 1: Study & Extract

1. Port Money library types to Rust
2. Extract Cashu protocol types/logic patterns
3. Study exchange rate provider patterns

### Phase 2: Implement in neobank crate

1. `Money<Currency>` type with BigDecimal
2. `ExchangeRateService` with Mempool/Coingecko/Coinbase
3. Quote state machine for send/receive

### Phase 3: Add What's Missing

1. Policy engine for budgets/limits
2. FROST integration for threshold signing
3. Agent-first API design
4. TreasuryRouter with multi-rail support

---

## References

- **Repository:** https://github.com/MakePrisms/agicash
- **Cashu Protocol:** https://github.com/cashubtc/nuts
- **Cashu-TS:** https://github.com/cashubtc/cashu-ts
- **Spark SDK:** https://github.com/buildonspark/spark-sdk
- **LNURL Spec:** https://github.com/lnurl/luds

---

## Appendix: Key Files to Review

| File | Why |
|------|-----|
| `app/lib/money/money.ts` | Money class implementation |
| `app/lib/cashu/utils.ts` | ExtendedCashuWallet |
| `app/lib/cashu/types.ts` | NUT-10 secret types |
| `app/features/accounts/account.ts` | Account type definitions |
| `app/features/send/cashu-send-quote.ts` | Quote type definition |
| `app/features/shared/encryption.ts` | ECIES encryption |
| `app/lib/exchange-rate/exchange-rate-service.ts` | Rate fetching |
| `app/features/receive/lightning-address-service.tsx` | LNURL server |
