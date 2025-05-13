Okay, let's break down this Blitz Wallet Web App and its use of the `@buildonspark/spark-sdk`.

**App Summary**

The "Blitz Wallet Web App" is a React-based self-custodial Bitcoin Lightning Network wallet. Its primary goal is to provide a simple and seamless experience for sending and receiving Bitcoin payments using the Spark infrastructure.

Key features of the app, as indicated by the file structure and README, include:

1.  **Wallet Creation:**
    *   Generating a mnemonic seed phrase (`createSeed.jsx`, `@scure/bip39`).
    *   Setting a local password to encrypt the seed phrase (`createPassword.jsx`, `encription.js`).
2.  **Authentication & Session Management:**
    *   Login using the password to decrypt the wallet key (`login.jsx`, `authContext.jsx`).
    *   Session timeout (2 hours) requiring re-login (`authGate.jsx`).
3.  **Sending Payments:**
    *   Scanning QR codes via camera (`camera.jsx`, `jsqr`).
    *   Pasting payment requests from clipboard (`sendPage.jsx`, `getDataFromClipboard.js`).
    *   Decoding various payment types (Lightning, Spark, Bitcoin on-chain) (`sendPayment.js`).
    *   Confirming payment details before sending (`sendPage.jsx`).
4.  **Receiving Payments:**
    *   Generating QR codes for Lightning, Spark, and Bitcoin on-chain (`receiveQRPage.jsx`, `qrcode.react`).
    *   Allowing users to specify an amount and description for the invoice (`receiveAmount.jsx`).
    *   Switching between different receive formats (`switchReceiveOption.jsx`).
5.  **Wallet Management:**
    *   Viewing wallet balance (`wallet.jsx`, `userBalanceContainer.jsx`).
    *   Viewing transaction history (`transactionContainer.jsx`, `viewAllTxPage.jsx`).
    *   Backing up/viewing the recovery phrase (`viewKey.jsx`).
    *   Restoring a wallet from a seed phrase (`restoreWallet.jsx`).
    *   Viewing Spark address and public key (`settings.jsx`).
    *   Logging out and deleting the wallet (`settings.jsx`).
6.  **Transaction Handling:**
    *   Displaying transaction status (pending, confirmed, failed) (`confirmPaymentScreen.jsx`).
    *   Storing and managing transaction history locally using IndexedDB (`txStorage.js`).
    *   Syncing transaction history with the Spark backend (`restore.js`).

The application is built using Vite, React, and `react-router-dom` for navigation. It leverages `crypto-js` for local encryption of the wallet key and `@scure/bip39` for mnemonic generation.

**Spark SDK Usage**

The `@buildonspark/spark-sdk` is central to the app's functionality, enabling interaction with the Spark network for managing the wallet, sending/receiving payments, and fetching transaction data.

The primary integration points are:

1.  **`src/contexts/sparkContext.jsx`**:
    *   This context provider initializes and manages the `sparkWallet` instance from the SDK.
    *   It holds state related to Spark, such as balance, transactions, connection status, public key, and Spark address (`sparkInformation`).
    *   It listens to Spark wallet events like `transfer:claimed` and `deposit:confirmed` to update the UI and local transaction storage in real-time.
    *   It periodically checks for L1 Bitcoin deposits to claim them.
    *   It attempts to restore transaction state on initialization.

2.  **`src/functions/spark.js`**:
    *   This file acts as a wrapper around direct Spark SDK calls, making them available to the rest of the application.
    *   It exports the `sparkWallet` instance.
    *   It defines several asynchronous functions that call methods on the `sparkWallet` object.

3.  **`src/functions/payments.js`**:
    *   This file orchestrates more complex payment and receive flows, often combining multiple SDK calls or adding application-specific logic (like handling support fees).
    *   It uses functions from `spark.js` (which in turn use the SDK).

4.  **`src/functions/restore.js`**:
    *   `restoreSparkTxState`: Uses SDK's `getSparkTransactions` to fetch transaction history and sync it with the local IndexedDB storage (`txStorage.js`).

**Specific Spark SDK Functions Used (via `src/functions/spark.js` and `src/functions/payments.js`)**

The application appears to use the following functions/objects from the `@buildonspark/spark-sdk`:

*   **`SparkWallet.initialize({ mnemonicOrSeed, options })`**:
    *   Called in `initializeSparkWallet` (in `spark.js`, used by `sparkContext.jsx`).
    *   Initializes the Spark wallet instance using the user's mnemonic seed phrase and network options (MAINNET).

*   **`sparkWallet.getIdentityPublicKey()`**:
    *   Called in `getSparkIdentityPublicKey` (in `spark.js`, used by `sparkContext.jsx`).
    *   Fetches the user's Spark identity public key.

*   **`sparkWallet.getBalance()`**:
    *   Called in `getSparkBalance` (in `spark.js`, used by `sparkContext.jsx`).
    *   Retrieves the current balance of the Spark wallet.

*   **`sparkWallet.getSingleUseDepositAddress()`**:
    *   Called in `getSparkBitcoinL1Address` (in `spark.js`) and `sparkReceivePaymentWrapper` (in `payments.js`).
    *   Generates a new, single-use Bitcoin L1 address for deposits into the Spark wallet.

*   **`sparkWallet.getUnusedDepositAddresses()`**:
    *   Called in `getUnusedSparkBitcoinL1Address` (in `spark.js`, used by `sparkContext.jsx`).
    *   Retrieves a list of unused Bitcoin L1 deposit addresses.

*   **`getLatestDepositTxId(depositAddress)`**: (SDK utility function)
    *   Called in `querySparkBitcoinL1Transaction` (in `spark.js`, used by `sparkContext.jsx`).
    *   Queries an external service (likely a block explorer API via Spark infrastructure) to find the latest transaction ID for a given Bitcoin L1 deposit address.

*   **`sparkWallet.claimDeposit(txId)`**:
    *   Called in `claimSparkBitcoinL1TransactionWithTxID` (in `spark.js`, used by `sparkContext.jsx`).
    *   Claims a Bitcoin L1 deposit into the Spark wallet using the transaction ID.

*   **`sparkWallet.getSparkAddress()`**:
    *   Called in `getSparkAddress` (in `spark.js`, used by `sparkContext.jsx` and `sparkReceivePaymentWrapper` in `payments.js`).
    *   Retrieves the user's unique Spark address for receiving Spark-to-Spark payments.

*   **`sparkWallet.transfer({ receiverSparkAddress, amountSats })`**:
    *   Called in `sendSparkPayment` (in `spark.js`, used by `sparkPaymenWrapper` in `payments.js`).
    *   Initiates a Spark-to-Spark transfer.

*   **`sparkWallet.transferTokens({ tokenPublicKey, tokenAmount, receiverSparkAddress })`**:
    *   Called in `sendSparkTokens` (in `spark.js`). *(Though not explicitly shown being used by higher-level functions in the provided structure, it's available)*.
    *   Initiates a transfer of Spark-compatible tokens.

*   **`sparkWallet.getLightningSendFeeEstimate({ encodedInvoice })`**:
    *   Called in `getSparkLightningPaymentFeeEstimate` (in `spark.js`) and `sparkPaymenWrapper` (in `payments.js`).
    *   Estimates the fee for sending a Lightning payment.

*   **`sparkWallet.getCoopExitFeeEstimate({ amountSats, withdrawalAddress })`**:
    *   Called in `getSparkBitcoinPaymentFeeEstimate` (in `spark.js`). *(Note: `payments.js` calls `getWithdrawalFeeEstimate` which might be a different or more specific SDK function for L1 withdrawals, or this is a wrapper for it.)*
    *   Estimates the fee for a cooperative exit (on-chain Bitcoin withdrawal).

*   **`sparkWallet.getWithdrawalFeeEstimate({ amountSats, withdrawalAddress })`**: (Likely the actual SDK function)
    *   Used in `sparkPaymenWrapper` (in `payments.js`) for Bitcoin on-chain fee estimation.

*   **`sparkWallet.getSwapFeeEstimate(amountSats)`**:
    *   Called in `getSparkPaymentFeeEstimate` (in `spark.js`) and `sparkPaymenWrapper` (in `payments.js`).
    *   Estimates the fee for internal Spark swaps (potentially related to Spark-to-Spark or other internal mechanisms).

*   **`sparkWallet.createLightningInvoice({ amountSats, memo })`**:
    *   Called in `receiveSparkLightningPayment` (in `spark.js`) and `sparkReceivePaymentWrapper` (in `payments.js`).
    *   Generates a Lightning invoice for receiving payments.

*   **`sparkWallet.getLightningReceiveRequest(lightningInvoiceId)`**:
    *   Called in `getSparkLightningPaymentStatus` (in `spark.js`).
    *   Fetches the status of a Lightning receive request (invoice).

*   **`sparkWallet.payLightningInvoice({ invoice, maxFeeSats })`**:
    *   Called in `sendSparkLightningPayment` (in `spark.js`) and `sparkPaymenWrapper` (in `payments.js`).
    *   Pays a Lightning invoice. The `maxFeeSats` parameter is available in the SDK but might not be explicitly used or exposed to the user in this app's current implementation.

*   **`sparkWallet.getTransfers(transferCount, offsetIndex)`**:
    *   Called in `getSparkTransactions` (in `spark.js`, used by `sparkContext.jsx` and `restore.js`).
    *   Fetches a list of transactions from the Spark wallet, with pagination support.

*   **`sparkWallet.withdraw({ onchainAddress, exitSpeed, amountSats })`**:
    *   Used in `sparkPaymenWrapper` (in `payments.js`).
    *   Initiates an on-chain Bitcoin withdrawal from the Spark wallet.

*   **Event Emitters (`sparkWallet.on()` and `sparkWallet.off()`)**:
    *   Used in `sparkContext.jsx` to listen for:
        *   `transfer:claimed`: When an incoming Spark-to-Spark transfer is successfully claimed.
        *   `deposit:confirmed`: When a Bitcoin L1 deposit is confirmed and credited.
    *   These are crucial for real-time updates of balance and transaction history.

**Helper/Utility Functions in the App that use SDK Data/Types:**

*   **`useSparkPaymentType(tx)`** (in `spark.js`): Determines if a transaction (`tx` object, likely from `getTransfers`) is "lightning", "bitcoin", or "spark" based on `tx.type`. It uses `TransferType` enum values from the SDK (e.g., `PREIMAGE_SWAP`, `COOPERATIVE_EXIT`, `TRANSFER`).
*   **`useIsSparkPaymentPending(tx, transactionPaymentType)`** (in `spark.js`): Checks the `tx.status` against known pending statuses for different `TransferType`s.
*   **`useIsSparkPaymentFailed(tx, transactionPaymentType)`** (in `spark.js`): Checks the `tx.status` for failure statuses.
*   **`TransferDirection`** (from `@buildonspark/spark-sdk/types`): Used in `transactionContainer.jsx` to determine if a transaction is incoming or outgoing.
*   **`TransferStatus`** (from `@buildonspark/spark-sdk/proto/spark`): Implicitly used when checking `tx.status` values in helper functions and contexts.

**Data Flow and Management:**

1.  **Initialization:** On login, `mnemoinc` is decrypted, and `initializeSparkWallet` is called in `sparkContext.jsx`.
2.  **State Management:** `sparkContext.jsx` maintains `sparkInformation` (balance, transactions, etc.) by calling SDK getter functions (`getSparkBalance`, etc.) and updates it based on SDK events.
3.  **Sending Payments:**
    *   User inputs/scans an address (`sendPage.jsx`, `camera.jsx`).
    *   `processInputType` (`sendPayment.js`) decodes it (LN, Spark, BTC).
    *   `sparkPaymenWrapper` (`payments.js`) is called:
        *   First to get fees (using SDK's `get...FeeEstimate` functions).
        *   Then to execute the payment (using SDK's `payLightningInvoice`, `withdraw`, or `transfer` via `sendSparkPayment`).
    *   Transaction state is updated locally (`txStorage.js`) and potentially via Spark events.
4.  **Receiving Payments:**
    *   User specifies amount/description (`receiveAmount.jsx`).
    *   `sparkReceivePaymentWrapper` (`payments.js`) is called:
        *   Uses SDK's `createLightningInvoice` for LN.
        *   Uses SDK's `getSingleUseDepositAddress` for Bitcoin L1.
        *   Uses SDK's `getSparkAddress` for Spark.
    *   The generated invoice/address is displayed as a QR code (`receiveQRPage.jsx`).
    *   Incoming payments trigger SDK events handled by `sparkContext.jsx`, updating balance and transactions.
5.  **Transaction History & Sync:**
    *   `txStorage.js` uses IndexedDB for local persistence.
    *   `restoreSparkTxState` (`restore.js`) uses `getSparkTransactions` (SDK) to fetch recent history and reconcile it with local storage, ensuring new or missed transactions are added.

In essence, the Spark SDK provides the bridge to the Spark backend for all core wallet operations. The application wraps these SDK calls with its own logic for UI presentation, local data persistence, fee calculations (including app-specific support fees), and user flow management. The `sparkContext.jsx` is pivotal in managing the SDK lifecycle and state.
