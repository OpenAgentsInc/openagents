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
