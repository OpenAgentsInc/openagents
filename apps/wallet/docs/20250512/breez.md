(Condensed version of Breez's Nodeless docs via Gemini. Official docs here: https://sdk-doc-liquid.breez.technology/guide/about_breez_sdk_liquid.html)

# Breez SDK Nodeless (Liquid implementation) Documentation for WebAssembly

This comprehensive document provides all the context needed to build web applications with the WebAssembly bindings of Breez SDK Nodeless, a toolkit for integrating Bitcoin, Lightning Network, and Liquid Network functionality.

## About Breez SDK - Nodeless *(Liquid Implementation)*

### **What Is the Breez SDK?**

The Breez SDK provides developers with an end-to-end solution for integrating self-custodial Lightning payments into their apps and services. It eliminates the need for third parties, simplifies the complexities of Bitcoin and Lightning, and enables seamless onboarding for billions of users to the future of peer-to-peer payments.

To provide the best experience for their end-users, developers can choose between the following implementations:

- Breez SDK - Nodeless *(Liquid Implementation)* (This Document)
- [Breez SDK - Native *(Greenlight Implementation)*](https://sdk-doc.breez.technology/)

### **What Is the Breez SDK - Nodeless *(Liquid Implementation)*?**

It’s a nodeless integration that offers a self-custodial, end-to-end solution for integrating Lightning payments, utilizing the Liquid Network with on-chain interoperability and third-party fiat on-ramps. Using the SDK you will able to:
- **Send payments** via various protocols such as: Bolt11, Bolt12, BIP353, LNURL-Pay, Lightning address, BTC address
- **Receive payments** via various protocols such as: Bolt11, LNURL-Withdraw, LNURL-Pay, Lightning address, BTC address

**Key Features**

- [x] Send and receive Lightning payments
- [x] On-chain interoperability
- [x] Complete LNURL functionality
- [x] Multi-app support
- [x] Multi-device support
- [x] Real-time state backup
- [x] Keys are only held by users
- [x] USDT and multi-asset support on Liquid
- [x] Built-in fiat on-ramp
- [x] Free open-source solution

### How Does Nodeless *(Liquid Implementation)* Work?

The Breez SDK - Nodeless *(Liquid implementation)* uses submarine swaps and reverse submarine swaps to send and receive payments, enabling funds to move frictionlessly between the Lightning Network and the Liquid sidechain.

![Breez SDK - Liquid](https://sdk-doc-liquid.breez.technology/images/BreezSDK_Liquid.png)

When sending a payment the SDK performs a submarine swap, converting L-BTC from a user’s Liquid wallet into sats on the Lightning Network, and sends them to the recipient.

When receiving a payment, the SDK performs a reverse submarine swap, converting incoming sats into L-BTC, and then deposits them in the user’s Liquid wallet.

### **Differences Between Implementations**

| Nodeless *(Liquid Implementation)* | Native *(Greenlight Implementation)* |
| --- | --- |
| Trust profile is with the Liquid sidechain | Pure Lightning Network implementation |
| No channel management or LSP required | Uses Lightning Service Providers (LSPs) for liquidity |
| No setup fees for end-users | Channel opening and closing fees |
| Minimum 100 sats to receive, 21 sats to send | No minimum limit for transactions (after channel opening) |
| Static low fees | Setup costs are correlated to Bitcoin mining fees |
| Bitcoin, USDT & multi-asset support | Bitcoin only |

### Pricing

The Breez SDK is **free** for developers.
See [End-User fees](#end-user-fees) for end-user fees.

### Support

Have a question for the team? Join us on [Telegram](https://t.me/breezsdk) or email us at <contact@breez.technology>.

### Repository

Head over to the [Breez SDK - Nodeless *(Liquid Implementation)* repo](https://github.com/breez/breez-sdk-liquid).

## Getting Started

### API Key
The _Nodeless_ Breez API key must be set for the SDK to work. You can request one for free by filling our form [here](https://forms.gle/L8q3N2L4QyL48GNE7).

**Note:** This is not the same as the _Native_ (Greenlight) Breez API key, so it can't be reused.

### Installation

```bash
npm install @breeztech/breez-sdk-liquid
```

### Initializing the SDK

To get started with Breez SDK Nodeless (Liquid implementation) in the browser, you need to initialize the WASM module and connect with your configuration:

```typescript
import init, { defaultConfig, connect, disconnect } from '@breeztech/breez-sdk-liquid/web';

const initWallet = async () => {
  // Initialize the WASM module first
  await init();

  // Your mnemonic seed phrase for wallet recovery
  const mnemonic = "<mnemonic words>";

  // Create the default config, providing your Breez API key
  const config = defaultConfig('mainnet', '<your-Breez-API-key>');

  // Customize the config object according to your needs
  // The workingDir doesn't need to be set in web environments
  // config.workingDir = 'path to writable directory'; // Not typically needed for web

  try {
    const sdk = await connect({
      config,
      mnemonic
    });
    return sdk;
  } catch (error) {
    console.error(error);
    throw error;
  }
};

const disconnectSdk = async (sdk) => {
  try {
    await sdk.disconnect();
    console.log("SDK disconnected successfully.");
  } catch (error) {
    console.error("Error disconnecting SDK:", error);
  }
};
```

### Custom Signer Support
If you prefer to manage your own keys, you can use a custom signer. To use an external signer, you'll need to:

1. Implement the `Signer` interface
2. Use `connectWithSigner` instead of `connect` when initializing the SDK

```typescript
import init, { connectWithSigner, defaultConfig, type Signer } from '@breeztech/breez-sdk-liquid/web';

const connectWithCustomSigner = async () => {
  await init();
  // Fully implement the Signer interface
  class JsSigner implements Signer {
    // Ensure your implementation matches the Signer interface from the SDK
    xpub = (): Uint8Array => { return new Uint8Array() }
    deriveXpub = (derivationPath: string): Uint8Array => { return new Uint8Array() }
    signEcdsa = (msg: Uint8Array, derivationPath: string): Uint8Array => { return new Uint8Array() }
    signEcdsaRecoverable = (msg: Uint8Array): Uint8Array => { return new Uint8Array() }
    slip77MasterBlindingKey = (): Uint8Array => { return new Uint8Array() }
    hmacSha256 = (msg: Uint8Array, derivationPath: string): Uint8Array => { return new Uint8Array() }
    eciesEncrypt = (msg: Uint8Array): Uint8Array => { return new Uint8Array() }
    eciesDecrypt = (msg: Uint8Array): Uint8Array => { return new Uint8Array() }
  }

  const signer = new JsSigner();

  // Create the default config, providing your Breez API key
  const config = defaultConfig('mainnet', '<your-Breez-API-key>');

  const sdk = await connectWithSigner({ config }, signer);

  return sdk;
};
```
> **Developer note**
> A reference implementation of such signer is available in the SDK repository. You can use it as-is or customize it to meet your requirements: [SdkSigner](https://github.com/breez/breez-sdk-liquid/blob/main/lib/core/src/signer.rs#L198).
> Note that this same implementation is used internally by the SDK when connecting with a mnemonics via the standard `Connect` method.

### Guidelines
- **Always make sure the sdk instance is synced before performing any actions**
- **Add logging**: Add sufficient logging to diagnose any issues users have
- **Display pending payments**: Payments always contain a status field to determine completion. Show the correct status to users.
- **Enable swap refunds**: Swaps resulting from On-Chain Transactions may not complete and change to `Refundable` state. Handle this by allowing the user to retry the refund with different fees until confirmed.
- **Expose swap fees**: When sending or receiving on-chain, clearly show the expected fees and amounts.

### Basic Operations

#### Setting Up Logging
The SDK implements detailed logging via a streaming interface you can manage within your application. The log entries are split into several levels that you can filter and store as desired within your application, for example, by appending them to a log file.
```typescript
import { setLogger, type LogEntry } from '@breeztech/breez-sdk-liquid/web';

const setupLogging = () => {
  class JsLogger {
    log = (l: LogEntry) => {
      console.log(`[${l.level}]: ${l.line}`);
    }
  }

  const logger = new JsLogger();
  setLogger(logger);
};
```

#### Event Handling
The SDK emits several events to provide the application with an up-to-date state of the wallet or ongoing payments.
```typescript
import { type SdkEvent, type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const setupEventListener = async (sdk: LiquidSdk) => {
  class JsEventListener {
    onEvent = (event: SdkEvent) => {
      console.log(`Received event: ${JSON.stringify(event)}`);
      // Example: if (event.type === 'synced') { ... }
    }
  }

  const eventListener = new JsEventListener();
  const listenerId = await sdk.addEventListener(eventListener);

  // Save the listenerId to remove the listener later
  return listenerId;
};

const removeSdkEventListener = async (sdk: LiquidSdk, listenerId: string) => { // Renamed to avoid conflict
  await sdk.removeEventListener(listenerId);
};
```

#### Fetch Balance
Once connected, the balance can be retrieved at any time.
```typescript
import { type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const fetchBalance = async (sdk: LiquidSdk) => {
  try {
    const info = await sdk.getInfo();
    const balanceSat = info.walletInfo.balanceSat;
    const pendingSendSat = info.walletInfo.pendingSendSat;
    const pendingReceiveSat = info.walletInfo.pendingReceiveSat;
    console.log(`Balance: ${balanceSat} sats`);
    console.log(`Pending Send: ${pendingSendSat} sats`);
    console.log(`Pending Receive: ${pendingReceiveSat} sats`);
    return info.walletInfo;
  } catch (error) {
    console.error(error);
    throw error;
  }
};
```

## Core Features

### Buying Bitcoin
The SDK also provides access to a Fiat on-ramp to purchase Bitcoin using Moonpay as a provider. It will generate a Bitcoin address and prepare a URL using the specified provider. The user then needs to open the URL and proceed with the provider flow to buy Bitcoin. Once the buy is completed, the provider will transfer the purchased amount to the Bitcoin address.

#### Checking the limits
Fetch the current onchain limits to check the minimum and maximum allowed to purchase.
```typescript
import { type LiquidSdk, type OnchainPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid/web';

const fetchOnchainLimits = async (sdk: LiquidSdk): Promise<OnchainPaymentLimitsResponse | undefined> => {
  try {
    const currentLimits = await sdk.fetchOnchainLimits();
    console.log(`Minimum amount, in sats: ${currentLimits.receive.minSat}`);
    console.log(`Maximum amount, in sats: ${currentLimits.receive.maxSat}`);
    return currentLimits;
  } catch (err) {
    console.error(err);
  }
};
```

#### Preparing to buy, checking fees
Using the current onchain limits, select a provider and amount to purchase.
```typescript
import { type LiquidSdk, type OnchainPaymentLimitsResponse, type PrepareBuyBitcoinResponse, BuyBitcoinProvider } from '@breeztech/breez-sdk-liquid/web';

const prepareBuyBtc = async (sdk: LiquidSdk, currentLimits: OnchainPaymentLimitsResponse): Promise<PrepareBuyBitcoinResponse | undefined> => {
  try {
    const prepareRes = await sdk.prepareBuyBitcoin({
      provider: BuyBitcoinProvider.MOONPAY, // Ensure BuyBitcoinProvider is correctly imported or defined
      amountSat: currentLimits.receive.minSat
    });

    // Check the fees are acceptable before proceeding
    const receiveFeesSat = prepareRes.feesSat;
    console.log(`Fees: ${receiveFeesSat} sats`);
    return prepareRes;
  } catch (err) {
    console.error(err);
  }
};
```

#### Generate the URL
Generate a URL to the provider with a Bitcoin address to receive the purchase to. You can also pass in an optional redirect URL here that the provider redirects to after a successful purchase.
```typescript
import { type LiquidSdk, type PrepareBuyBitcoinResponse } from '@breeztech/breez-sdk-liquid/web';

const buyBtc = async (sdk: LiquidSdk, prepareResponse: PrepareBuyBitcoinResponse) => {
  try {
    const url = await sdk.buyBitcoin({
      prepareResponse,
      // redirectUrl: "https://example.com/payment-success" // Optional
    });
    // Redirect user to url to complete purchase, e.g., window.location.href = url;
    console.log("Moonpay URL:", url);
    return url;
  } catch (err) {
    console.error(err);
  }
};
```

### Fiat Currencies
#### List fiat currencies
You can get the full details of supported fiat currencies, such as symbols and localized names:
```typescript
import { type LiquidSdk, type FiatCurrency } from '@breeztech/breez-sdk-liquid/web';

const listFiatCurrencies = async (sdk: LiquidSdk): Promise<FiatCurrency[] | undefined> => {
  try {
    const fiatCurrencies = await sdk.listFiatCurrencies();
    return fiatCurrencies;
  } catch (error) {
    console.error("Error fetching fiat currencies:", error);
  }
};
```

#### Fetch fiat rates
To get the current BTC rate in the various supported fiat currencies:
```typescript
import { type LiquidSdk, type Rate } from '@breeztech/breez-sdk-liquid/web';

const fetchFiatRates = async (sdk: LiquidSdk): Promise<Rate[] | undefined> => {
  try {
    const fiatRates = await sdk.fetchFiatRates();
    return fiatRates;
  } catch (error) {
    console.error("Error fetching fiat rates:", error);
  }
};
```

### Payment fundamentals
This section details sending and receiving payments.

### Parsing inputs
The SDK provides a versatile and extensible parsing module designed to process a wide range of input strings and return parsed data in various standardized formats.

Natively supported formats include: BOLT11 invoices, BOLT12 offers, LNURLs of different types, Bitcoin addresses, and others. For the complete list, consult the [API documentation](https://breez.github.io/breez-sdk-liquid/breez_sdk_liquid/enum.InputType.html).

BIP353 addresses are also supported, in which case they will be parsed into a BOLT12 offer or an LNURL-Pay. In these cases, the BIP353 address is also returned to indicate BIP353 was used.

```typescript
import { type LiquidSdk, InputTypeVariant, type InputType } from '@breeztech/breez-sdk-liquid/web';

const parseInput = async (sdk: LiquidSdk, input: string): Promise<InputType | undefined> => {
  try {
    const parsed = await sdk.parse(input);

    switch (parsed.type) {
      case InputTypeVariant.BITCOIN_ADDRESS:
        console.log(`Input is Bitcoin address ${parsed.address.address}`);
        break;

      case InputTypeVariant.BOLT11:
        console.log(
          `Input is BOLT11 invoice for ${
            parsed.invoice.amountMsat != null ? parsed.invoice.amountMsat.toString() : 'unknown'
          } msats`
        );
        break;

      case InputTypeVariant.LN_URL_PAY:
        console.log(
          `Input is LNURL-Pay/Lightning address accepting min/max ${parsed.data.minSendable}/${
            parsed.data.maxSendable
          } msats - BIP353 was used: ${parsed.bip353Address != null}`
        );
        break;

      case InputTypeVariant.LN_URL_WITHDRAW:
        console.log(
          `Input is LNURL-Withdraw for min/max ${parsed.data.minWithdrawable}/${parsed.data.maxWithdrawable} msats`
        );
        break;

      // Add other cases as needed: LN_URL_AUTH, LN_URL_ERROR, LN_URL_CHANNEL, NODE_ID, URL, LIQUID_ADDRESS

      default:
        console.log("Input type not specifically handled in this example:", parsed.type);
        break;
    }
    return parsed;
  } catch (error) {
    console.error("Error parsing input:", error);
  }
};
```

#### Supporting other input formats
The parsing module can be extended using external input parsers provided in the SDK configuration. These will be used when the input is not recognized. You can implement and provide your own parsers, or use existing public ones.

##### Configuring external parsers
Configuring external parsers can only be done before [connecting](#connecting) and the config cannot be changed through the lifetime of the connection.
Multiple parsers can be configured, and each one is defined by:
*   **Provider ID**: an arbitrary id to identify the provider input type
*   **Input regex**: a regex pattern that should reliably match all inputs that this parser can process, even if it may also match some invalid inputs
*   **Parser URL**: a URL containing the placeholder `<input>`

When parsing an input that isn't recognized as one of the native input types, the SDK will check if the input conforms to any of the external parsers regex expressions. If so, it will make an HTTP `GET` request to the provided URL, replacing the placeholder with the input. If the input is recognized, the response should include in its body a string that can be parsed into one of the natively supported types.

```typescript
import init, { defaultConfig, connect, type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const configureExternalParsers = async (): Promise<LiquidSdk | undefined> => {
  try {
    await init();
    const mnemonic = '<mnemonic words>';

    // Create the default config, providing your Breez API key
    const config = defaultConfig('mainnet', '<your-Breez-API-key>');

    // Configure external parsers
    config.externalInputParsers = [
      {
        providerId: 'provider_a',
        inputRegex: '^provider_a', // Example regex
        parserUrl: 'https://parser-domain.com/parser_a?input=<input>'
      },
      {
        providerId: 'provider_b',
        inputRegex: '^provider_b', // Example regex
        parserUrl: 'https://parser-domain.com/parser_b?input=<input>'
      }
    ];

    const sdk = await connect({ mnemonic, config });
    console.log("SDK connected with external parsers configured.");
    return sdk;
  } catch (error) {
    console.error("Error configuring external parsers:", error);
  }
};
```

##### Public external parsers
*   [**PicknPay QRs**](https://www.pnp.co.za/)
    *   Maintainer: [MoneyBadger](https://www.moneybadger.co.za/)
    *   Regex: `(.*)(za.co.electrum.picknpay)(.*)`
    *   URL: `https://cryptoqr.net/.well-known/lnurlp/<input>`
    *   More info: [support+breezsdk@moneybadger.co.za](mailto:support+breezsdk@moneybadger.co.za)

##### Default external parsers
The SDK ships with some embedded default external parsers. If you prefer not to use them, you can disable them in the SDK's configuration. See the available default parsers in the [API Documentation](https://breez.github.io/breez-sdk-liquid/breez_sdk_liquid/sdk/constant.DEFAULT_EXTERNAL_INPUT_PARSERS.html) by checking the source of the constant.

### Receiving Payments
With the Breez SDK you aren't required to open a channel and set up your inbound liquidity.
Once the SDK is initialized, you can directly begin receiving payments. The receive process takes two steps:
1.  [Preparing the Payment](#preparing-receive-payments)
2.  [Receiving the Payment](#execute-receive)

> **Developer note**
> Consider implementing [Mobile Notifications](#mobile-notifications) when using the Breez SDK in a mobile application. By registering a webhook the application can receive notifications to process the payment in the background.

<h4 id="preparing-receive-payments">Preparing Payments</h4>
During the prepare step, the SDK ensures that the inputs are valid with respect to the specified payment method, and also returns the relative fees related to the payment so they can be confirmed.
The SDK currently supports three methods of receiving: Lightning, Bitcoin and Liquid.

##### Lightning
When receiving via Lightning, we generate an invoice to be paid. Note that the payment may fallback to a direct Liquid payment (if the payer's client supports this).

**Note:** The amount field is currently mandatory when paying via Lightning.
```typescript
import { type LiquidSdk, PaymentMethod, ReceiveAmountVariant, type PrepareReceiveResponse, type ReceiveAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareReceivePaymentLightning = async (sdk: LiquidSdk): Promise<PrepareReceiveResponse | undefined> => {
  try {
    // Fetch the lightning Receive limits
    const currentLimits = await sdk.fetchLightningLimits();
    console.log(`Minimum amount allowed to deposit in sats: ${currentLimits.receive.minSat}`);
    console.log(`Maximum amount allowed to deposit in sats: ${currentLimits.receive.maxSat}`);

    // Set the invoice amount you wish the payer to send, which should be within the above limits
    const amount: ReceiveAmount = {
      type: ReceiveAmountVariant.BITCOIN,
      payerAmountSat: BigInt(5000) // Use BigInt for satoshi amounts
    };

    const prepareResponse = await sdk.prepareReceivePayment({
      paymentMethod: PaymentMethod.LIGHTNING,
      amount: amount
    });

    // If the fees are acceptable, continue to create the Receive Payment
    const receiveFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${receiveFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Lightning receive payment:", error);
  }
};
```

##### Bitcoin
When receiving via Bitcoin, we generate a Bitcoin BIP21 URI to be paid.
The `amount` field is optional when preparing a Bitcoin payment. However, if no amount is provided, the returned fees will only be an estimation. This is because:
1. The fees have an amount-dependent component that can only be determined once the sender initiates the payment
2. The fees also depend on current onchain fee conditions, which may change between the time of preparation and actual payment
If the onchain fee rate increases between preparation and payment time, the payment will be put on hold until the user explicitly confirms the new fees. To learn more about this, see the [Amountless Bitcoin Payments](#amountless-bitcoin-payments) section below.

```typescript
import { type LiquidSdk, PaymentMethod, ReceiveAmountVariant, type PrepareReceiveResponse, type ReceiveAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareReceivePaymentOnchain = async (sdk: LiquidSdk): Promise<PrepareReceiveResponse | undefined> => {
  try {
    // Fetch the Onchain Receive limits
    const currentLimits = await sdk.fetchOnchainLimits();
    console.log(`Minimum amount allowed to deposit in sats: ${currentLimits.receive.minSat}`);
    console.log(`Maximum amount allowed to deposit in sats: ${currentLimits.receive.maxSat}`);

    // Set the onchain amount you wish the payer to send, which should be within the above limits
    const optionalAmount: ReceiveAmount = {
      type: ReceiveAmountVariant.BITCOIN,
      payerAmountSat: BigInt(5000) // Example amount
    };

    const prepareResponse = await sdk.prepareReceivePayment({
      paymentMethod: PaymentMethod.BITCOIN_ADDRESS,
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Receive Payment
    const receiveFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${receiveFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing onchain receive payment:", error);
  }
};
```
> **Developer note**
> The above checks include validating against maximum and minimum limits. **Even when no specific amount is provided**, the amount transferred to the swap address must still fall within these limits. Your application's users must be informed of these limits because if the amount transferred falls outside this valid range, the funds will not be successfully received via the normal swap flow. In such cases, a manual refund will be necessary.
> For further instructions on how to execute a manual refund, see the section on [refunding payments](#refunding-payments).

##### Liquid
When receiving via Liquid, we can either generate an address to receive to, or a BIP21 URI with information regarding the payment (currently only the amount and message).
To generate a BIP21 address, all you have to do is specify a payer amount.

> **Developer note**
> To receive non-Bitcoin assets, see [Handling multiple assets](#handling-multiple-assets).

```typescript
import { type LiquidSdk, PaymentMethod, ReceiveAmountVariant, type PrepareReceiveResponse, type ReceiveAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareReceivePaymentLiquid = async (sdk: LiquidSdk): Promise<PrepareReceiveResponse | undefined> => {
  try {
    // Create a Liquid BIP21 URI/address to receive a payment to
    // There are no limits, but the payer amount should be greater than broadcast fees when specified
    // Note: Not setting the amount will generate a plain Liquid address
    const optionalAmount: ReceiveAmount = {
      type: ReceiveAmountVariant.BITCOIN,
      payerAmountSat: BigInt(5000)
    };

    const prepareResponse = await sdk.prepareReceivePayment({
      paymentMethod: PaymentMethod.LIQUID_ADDRESS,
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Receive Payment
    const receiveFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${receiveFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Liquid receive payment:", error);
  }
};
```

<h4 id="execute-receive">Receiving Payments</h4>
Once the payment has been prepared, all you have to do is pass the prepare response as an argument to the receive method, optionally specifying a description.
**Note:** The description field will be used differently, depending on the payment method:
- For Lightning payments, it will be encoded in the invoice
- For Bitcoin/Liquid BIP21 payments, it will be encoded in the URI as the `message` field.
- For plain Liquid payments, the description has no effect.

```typescript
import { type LiquidSdk, type PrepareReceiveResponse, type ReceivePaymentResponse } from '@breeztech/breez-sdk-liquid/web';

const receivePayment = async (sdk: LiquidSdk, prepareResponse: PrepareReceiveResponse): Promise<ReceivePaymentResponse | undefined> => {
  try {
    const optionalDescription = '<description>';
    const res = await sdk.receivePayment({
      prepareResponse,
      description: optionalDescription
      // useDescriptionHash: false, // Optional
    });

    const destination = res.destination;
    console.log("Receive destination:", destination); // This is the invoice or address to share with the payer
    return res;
  } catch (error) {
    console.error("Error receiving payment:", error);
  }
};
```

<h5 id="amountless-bitcoin-payments">Amountless Bitcoin Payments</h5>
To receive a Bitcoin payment that does not specify an amount, it may be necessary to explicitly accept the associated fees. This will be the case when the onchain fee rate increases between preparation and payment time.
Alternatively, if the fees are considered too high, the user can either choose to wait for them to come down or outright refund the payment. To learn more about refunds, see the [Refunding payments](#refunding-payments) section.
To reduce the likelihood of this extra fee review step being necessary, you can configure a fee rate leeway in the SDK's configuration that will automatically accept slightly higher fees within the specified tolerance.

```typescript
import { type LiquidSdk, PaymentState, PaymentDetailsVariant, type ListPaymentsRequest, type FetchPaymentProposedFeesRequest, type AcceptPaymentProposedFeesRequest } from '@breeztech/breez-sdk-liquid/web';

const handlePaymentsWaitingFeeAcceptance = async (sdk: LiquidSdk) => {
  try {
    // Payments on hold waiting for fee acceptance have the state WAITING_FEE_ACCEPTANCE
    const listPaymentsRequest: ListPaymentsRequest = {
        states: [PaymentState.WAITING_FEE_ACCEPTANCE]
    };
    const paymentsWaitingFeeAcceptance = await sdk.listPayments(listPaymentsRequest);

    for (const payment of paymentsWaitingFeeAcceptance) {
      if (payment.details.type !== PaymentDetailsVariant.BITCOIN) {
        // Only Bitcoin payments can be `WAITING_FEE_ACCEPTANCE`
        continue;
      }

      const fetchFeesRequest: FetchPaymentProposedFeesRequest = {
        swapId: payment.details.swapId
      };
      const fetchFeesResponse = await sdk.fetchPaymentProposedFees(fetchFeesRequest);

      console.info(
        `Payer sent ${fetchFeesResponse.payerAmountSat} and currently proposed fees are ${fetchFeesResponse.feesSat}`
      );

      // If the user is ok with the fees, accept them, allowing the payment to proceed
      const acceptFeesRequest: AcceptPaymentProposedFeesRequest = {
          response: fetchFeesResponse
      };
      await sdk.acceptPaymentProposedFees(acceptFeesRequest);
      console.log(`Accepted fees for swap ID: ${payment.details.swapId}`);
    }
  } catch (error) {
    console.error("Error handling payments waiting fee acceptance:", error);
  }
};
```

##### Event Flows for Receiving Payments
Once a receive payment is initiated, you can follow and react to the different payment events using the guide below for each payment method. See [Event Handling](#event-handling) for how to subscribe to events.

**Lightning**
| Event | Description | UX Suggestion |
| --- | --- | --- |
| **PaymentPending** | The swap service is holding an incoming payment for the Lightning invoice and has broadcast a lockup transaction. The SDK has seen the lockup transaction and will broadcast the claim transaction, either when the lockup transaction is confirmed or immediately if it is accepted as a zero-conf payment. | Show payment as pending. |
| **PaymentWaitingConfirmation** | The claim transaction has been broadcast or a direct Liquid transaction ([MRH](https://docs.boltz.exchange/v/api/magic-routing-hints)) has been seen. | Display successful payment feedback. |
| **PaymentSucceeded** | The claim transaction or direct Liquid transaction ([MRH](https://docs.boltz.exchange/v/api/magic-routing-hints)) is confirmed. | Show payment as complete. |
| **PaymentFailed** | The swap has failed from one of several reasons. Either the swap/invoice has expired or the lockup transaction failed to broadcast. |  |

**Bitcoin**
| Event                           | Description                                                                                                                                                                                                                                                                                                                           | UX Suggestion                                   |
|---------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------|
| **PaymentWaitingFeeAcceptance** | The swap service has seen the Bitcoin lockup transaction for an amountless swap and the associated fees need to be accepted. If the fees are within the configured leeway they will be automatically accepted, otherwise the user has to explicitly accept the fees. See [Amountless Bitcoin Payments](#amountless-bitcoin-payments). | Allow the user to review fees for this payment. |
| **PaymentPending**              | The swap service has seen the Bitcoin lockup transaction and the amount is accepted. Once the SDK has seen the Liquid lockup transaction, it will broadcast the Liquid claim transaction, either when the Liquid lockup transaction is confirmed or immediately if it is accepted as a zero-conf payment.                             | Show payment as pending.                        |
| **PaymentWaitingConfirmation**  | The Liquid claim transaction has been broadcast and is waiting confirmation.                                                                                                                                                                                                                                                          | Display successful payment feedback.            |
| **PaymentSucceeded**            | The Liquid claim transaction is confirmed.                                                                                                                                                                                                                                                                                            | Show payment as complete.                       |
| **PaymentFailed**               | The swap has failed from one of several reasons. Either the swap has expired, the fee was not accepted or the Liquid lockup transaction failed to broadcast.                                                                                                                                                                          |                                                 |
| **PaymentRefundable**           | Similar to PaymentFailed, but a Bitcoin lockup transaction was broadcast so the funds will need to be refunded, see [Refunding payments](#refunding-payments).                                                                                                                                                                 | Show payment as refundable.                     |
| **PaymentRefundPending**        | A Bitcoin refund transaction has been broadcast and is waiting confirmation.                                                                                                                                                                                                                                                          |                                                 |

**Liquid**
| Event | Description | UX Suggestion |
| --- | --- | --- |
| **PaymentWaitingConfirmation** | The transaction has been seen. | Display successful payment feedback. |
| **PaymentSucceeded** | The transaction is confirmed. | Show payment as complete. |

### Sending Payments
Once the SDK is initialized, you can directly begin sending payments. The send process takes two steps:
1.  [Preparing the Payment](#preparing-send-payments)
2.  [Sending the Payment](#execute-send-payment)

> **Developer note**
> Consider implementing [Mobile Notifications](#mobile-notifications) when using the Breez SDK in a mobile application. By registering a webhook the application can receive notifications to process the payment in the background.

<h4 id="preparing-send-payments">Preparing Payments</h4>
During the prepare step, the SDK ensures that the inputs are valid with respect to the destination, and also returns the relative fees related to the payment so they can be confirmed.
The `destination` field of the payment request supports Liquid BIP21, Liquid addresses and Lightning invoices.

##### Lightning
Two types of Lightning destinations are possible: BOLT11 invoices and BOLT12 offers.
For BOLT11 invoices, the amount **must** be set. If the optional prepare request amount is also set, the SDK will make sure the two values match, else an error will be thrown.
The SDK will also validate that the amount is within the send lightning limits of the swap service.

```typescript
import { type LiquidSdk, PayAmountVariant, type PrepareSendResponse, type PayAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareSendPaymentLightningBolt11 = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    // Set the bolt11 invoice you wish to pay
    const prepareResponse = await sdk.prepareSendPayment({
      destination: '<bolt11 invoice>'
    });

    // If the fees are acceptable, continue to create the Send Payment
    const sendFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${sendFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Bolt11 send payment:", error);
  }
};

const prepareSendPaymentLightningBolt12 = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    // Set the bolt12 offer you wish to pay
    const optionalAmount: PayAmount = {
      type: PayAmountVariant.BITCOIN,
      receiverAmountSat: BigInt(5000)
    };

    const prepareResponse = await sdk.prepareSendPayment({
      destination: '<bolt12 offer>',
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Send Payment
    const sendFeesSat = prepareResponse.feesSat;
    console.log(`BOLT12 Fees: ${sendFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Bolt12 send payment:", error);
  }
};
```
> **Developer note**
> When paying an invoice generated by another Liquid SDK instance, or any other application which uses the Boltz swapper internally, the payment will fall back to a direct onchain payment.
> The advantage of this is the payer will spend less on fees, as they are no longer relying on external services to execute the payment.
> Note that this also means a [Breez API key](https://forms.gle/L8q3N2L4QyL48GNE7) **will be required** in order for the payment to be executed.
> To learn more about this process and how it works in detail, see the Boltz documentation for  [Magic Routing Hints (MRH)](https://docs.boltz.exchange/v/api/magic-routing-hints).

##### Bitcoin
For onchain (Bitcoin) payments, see [Sending an on-chain transaction](#sending-an-on-chain-transaction).

##### Liquid
When sending via Liquid, a BIP21 URI or Liquid address can be used as the destination.
If a Liquid address is used, the optional prepare request amount **must** be set.
If a BIP21 URI is used, either the BIP21 URI amount or optional prepare request amount **must** be set. When both amounts are set, the SDK will prioritize the **request amount** over the BIP21 amount.
**Note:** If a valid Breez API key is not provided, the method will throw an error requiring you to specify one.

> **Developer note**
> To send non-Bitcoin assets, see [Handling multiple assets](#handling-multiple-assets).

###### Setting the receiver amount
When you want the payment recipient to receive a specific amount.
```typescript
import { type LiquidSdk, PayAmountVariant, type PrepareSendResponse, type PayAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareSendPaymentLiquid = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    // Set the Liquid BIP21 or Liquid address you wish to pay
    const optionalAmount: PayAmount = {
      type: PayAmountVariant.BITCOIN,
      receiverAmountSat: BigInt(5000)
    };

    const prepareResponse = await sdk.prepareSendPayment({
      destination: '<Liquid BIP21 or address>',
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Send Payment
    const sendFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${sendFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Liquid send payment:", error);
  }
};
```

###### Draining all funds
When you want send all funds from your wallet to another address.
```typescript
import { type LiquidSdk, PayAmountVariant, type PrepareSendResponse, type PayAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareSendPaymentLiquidDrain = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    // Set the Liquid BIP21 or Liquid address you wish to pay
    const optionalAmount: PayAmount = {
      type: PayAmountVariant.DRAIN
    };

    const prepareResponse = await sdk.prepareSendPayment({
      destination: '<Liquid BIP21 or address>',
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Send Payment
    const sendFeesSat = prepareResponse.feesSat;
    console.log(`Fees: ${sendFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing Liquid drain payment:", error);
  }
};
```

<h4 id="execute-send-payment">Sending Payments</h4>
Once the payment has been prepared, all you have to do is pass the prepare response as an argument to the send method.
```typescript
import { type LiquidSdk, type PrepareSendResponse, type SendPaymentResponse } from '@breeztech/breez-sdk-liquid/web';

const sendPayment = async (sdk: LiquidSdk, prepareResponse: PrepareSendResponse): Promise<SendPaymentResponse | undefined> => {
  try {
    const sendResponse = await sdk.sendPayment({
      prepareResponse
      // useAssetFees: false // Optional, defaults to false
    });
    const payment = sendResponse.payment;
    console.log("Payment sent:", payment);
    return sendResponse;
  } catch (error) {
    console.error("Error sending payment:", error);
  }
};
```

##### Event Flows for Sending Payments
Once a send payment is initiated, you can follow and react to the different payment events using the guide below for each payment method. See [Event Handling](#event-handling) for how to subscribe to events.

**Lightning**
| Event | Description | UX Suggestion |
| --- | --- | --- |
| **PaymentPending** | The SDK has broadcast the lockup transaction for the swap. | Show payment as pending. |
| **PaymentWaitingConfirmation** | If the Lightning invoice contains an [MRH](https://docs.boltz.exchange/v/api/magic-routing-hints), the SDK will instead broadcast a direct Liquid transaction. | Display successful payment feedback. |
| **PaymentSucceeded** | The swap service has seen the lockup transaction and broadcast the claim transaction, or the direct Liquid transaction ([MRH](https://docs.boltz.exchange/v/api/magic-routing-hints)) is confirmed. | Show payment as complete. |
| **PaymentFailed** | The swap has expired without a lockup transaction. |  |
| **PaymentRefundPending** | The swap can be refunded for several reasons. Either the swap/invoice has expired or the swap service failed to pay the invoice. In this case the SDK will broadcast a refund transaction. |  |
| **PaymentRefunded** | The refund transaction is confirmed. |  |

**Bitcoin** (See [Sending an on-chain transaction](#sending-an-on-chain-transaction))

**Liquid**
| Event | Description | UX Suggestion |
| --- | --- | --- |
| **PaymentWaitingConfirmation** | The transaction has been seen. | Display successful payment feedback. |
| **PaymentSucceeded** | The transaction is confirmed. | Show payment as complete. |

### List Payments
To view your payment history you can list all the sent and received payments made.
```typescript
import { type LiquidSdk, type ListPaymentsRequest, type Payment, PaymentType, ListPaymentDetailsVariant, GetPaymentRequestVariant, type GetPaymentRequest } from '@breeztech/breez-sdk-liquid/web';

const listAllPayments = async (sdk: LiquidSdk): Promise<Payment[] | undefined> => {
  try {
    const payments = await sdk.listPayments({});
    return payments;
  } catch (error) {
    console.error("Error listing all payments:", error);
  }
};
```

#### Filtering Payments
When listing payment you can also filter and page the list results, by:

##### Type and timestamp
```typescript
const listPaymentsFiltered = async (sdk: LiquidSdk): Promise<Payment[] | undefined> => {
  try {
    const request: ListPaymentsRequest = {
      filters: [PaymentType.SEND],
      fromTimestamp: 1696880000, // Example Unix timestamp
      toTimestamp: 1696959200,   // Example Unix timestamp
      offset: 0,
      limit: 50
    };
    const payments = await sdk.listPayments(request);
    return payments;
  } catch (err) {
    console.error("Error listing filtered payments:", err);
  }
};
```

##### Bitcoin address
```typescript
const listPaymentsDetailsAddress = async (sdk: LiquidSdk): Promise<Payment[] | undefined> => {
  try {
    const request: ListPaymentsRequest = {
      details: {
        type: ListPaymentDetailsVariant.BITCOIN,
        address: '<Bitcoin address>'
      }
    };
    const payments = await sdk.listPayments(request);
    return payments;
  } catch (err) {
    console.error("Error listing payments by Bitcoin address:", err);
  }
};
```

##### Liquid destination
```typescript
const listPaymentsDetailsDestination = async (sdk: LiquidSdk): Promise<Payment[] | undefined> => {
  try {
    const request: ListPaymentsRequest = {
      details: {
        type: ListPaymentDetailsVariant.LIQUID,
        destination: '<Liquid BIP21 or address>'
        // assetId: "<asset_id>" // Optional: To filter by asset ID
      }
    };
    const payments = await sdk.listPayments(request);
    return payments;
  } catch (err) {
    console.error("Error listing payments by Liquid destination:", err);
  }
};
```

#### Get Payment
You can also retrieve a single Lightning payment using one of the following identifier kinds:
*   Lightning payment hash
*   Swap ID or its SHA256 hash

```typescript
const getPayment = async (sdk: LiquidSdk) => {
  try {
    const paymentHash = '<payment hash>';
    const paymentByHashRequest: GetPaymentRequest = {
      type: GetPaymentRequestVariant.PAYMENT_HASH,
      paymentHash
    };
    const paymentByHash = await sdk.getPayment(paymentByHashRequest);
    console.log("Payment by hash:", paymentByHash);

    const swapId = '<swap id>';
    const paymentBySwapIdRequest: GetPaymentRequest = {
      type: GetPaymentRequestVariant.SWAP_ID,
      swapId
    };
    const paymentBySwapId = await sdk.getPayment(paymentBySwapIdRequest);
    console.log("Payment by swap ID:", paymentBySwapId);

    return { paymentByHash, paymentBySwapId };
  } catch (error) {
    console.error("Error getting payment:", error);
  }
};
```

<h3 id="refunding-payments">Refunding payments</h3>
The SDK handles refunding of failed payments automatically except when receiving Bitcoin payments where the refund of a failed swap has to be managed manually.

<h4 id="list-refundables">List refundables</h4>
In order to manually execute a Bitcoin refund, you need to supply an on-chain Bitcoin address to which the refunded amount will be sent. The following code will retrieve the refundable swaps:
```typescript
import { type LiquidSdk, type RefundableSwap } from '@breeztech/breez-sdk-liquid/web';

const listRefundables = async (sdk: LiquidSdk): Promise<RefundableSwap[] | undefined> => {
  try {
    const refundables = await sdk.listRefundables();
    return refundables;
  } catch (err) {
    console.error("Error listing refundables:", err);
  }
};
```

<h4 id="recommended-fees-for-refund">Recommended fees for refund</h4>
To refund a swap, you need to set a fee rate for the Bitcoin transaction. You can get the Bitcoin mempool fee estimates from the SDK:
```typescript
import { type LiquidSdk, type RecommendedFees } from '@breeztech/breez-sdk-liquid/web';

const recommendedFeesForRefund = async (sdk: LiquidSdk): Promise<RecommendedFees | undefined> => {
  try {
    const fees = await sdk.recommendedFees();
    console.log("Recommended fees (sat/vbyte):", fees);
    return fees;
  } catch (error) {
    console.error("Error fetching recommended fees:", error);
  }
};
```

<h4 id="execute-refund-payment">Refund payment</h4>
Once you have a refundable swap, use the following code to execute a refund:
```typescript
import { type LiquidSdk, type RefundableSwap, type RefundRequest, type RefundResponse } from '@breeztech/breez-sdk-liquid/web';

const executeRefund = async (sdk: LiquidSdk, refundable: RefundableSwap, refundTxFeeRate: number): Promise<RefundResponse | undefined> => {
  try {
    const destinationAddress = '...'; // User's Bitcoin address for the refund
    const feeRateSatPerVbyte = refundTxFeeRate;

    const refundRequest: RefundRequest = {
      swapAddress: refundable.swapAddress,
      refundAddress: destinationAddress,
      feeRateSatPerVbyte
    };
    const refundResponse = await sdk.refund(refundRequest);
    console.log("Refund executed. Tx ID:", refundResponse.refundTxId);
    return refundResponse;
  } catch (error) {
    console.error("Error executing refund:", error);
  }
};
```
> **Developer note**
> A refund can be attempted several times. A common scenario where this is useful is if the initial refund transaction takes too long to mine, your application's users can be offered the ability to re-trigger the refund with a higher feerate.

<h3 id="rescanning-swaps">Rescanning swaps</h3>
The SDK continuously monitors any ongoing swap transactions until they are either completed or refunded. Once one of these outcomes occurs, the SDK ceases its monitoring activities, and users are advised against sending additional funds to the swap address.
However, if users inadvertently send additional funds to a swap address that was already used, the SDK won't automatically recognize it. In such cases, the SDK provides an option to manually scan the used swap addressed to identify additional transactions. This action allows the address to be included in the list eligible for refunds, enabling the initiation of a refund process. For the purpose of rescanning all historical swap addresses and updating their on-chain status, the following code can be used:

```typescript
import { type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const rescanSwaps = async (sdk: LiquidSdk) => {
  try {
    await sdk.rescanOnchainSwaps();
    console.log("Onchain swaps rescanned successfully.");
  } catch (err) {
    console.error("Error rescanning onchain swaps:", err);
  }
};
```

<h3 id="sending-an-on-chain-transaction">Sending an on-chain transaction (Pay Onchain)</h3>
You can send funds from the Breez SDK wallet to an on-chain address as follows.

> **Developer note**
> Consider implementing [Mobile Notifications](#mobile-notifications) when using the Breez SDK in a mobile application. By registering a webhook the application can receive notifications to process the payment in the background.

<h4 id="preparing-onchain-payments">Preparing Onchain Payments</h4>
When sending an onchain payment, the swap limits for sending onchain need to be first checked.
```typescript
import { type LiquidSdk, type OnchainPaymentLimitsResponse } from '@breeztech/breez-sdk-liquid/web';

const getCurrentPayOnchainLimits = async (sdk: LiquidSdk): Promise<OnchainPaymentLimitsResponse | undefined> => {
  try {
    const currentLimits = await sdk.fetchOnchainLimits();
    console.log(`Minimum send amount, in sats: ${currentLimits.send.minSat}`);
    console.log(`Maximum send amount, in sats: ${currentLimits.send.maxSat}`);
    return currentLimits;
  } catch (err) {
    console.error("Error fetching onchain send limits:", err);
  }
};
```
This represents the range of valid amounts that can be sent at this point in time. The range may change depending on the swap service parameters or mempool feerate fluctuations.
> **Developer note**
> It is best to fetch these limits just before your app shows the Pay Onchain (reverse swap) UI. You can then use these limits to validate the user input.

##### Setting the receiver amount
When you want the payment recipient to receive a specific amount.
```typescript
import { type LiquidSdk, PayAmountVariant, type PreparePayOnchainResponse, type PayAmount, type PreparePayOnchainRequest } from '@breeztech/breez-sdk-liquid/web';

const preparePayOnchain = async (sdk: LiquidSdk): Promise<PreparePayOnchainResponse | undefined> => {
  try {
    const request: PreparePayOnchainRequest = {
      amount: {
        type: PayAmountVariant.BITCOIN,
        receiverAmountSat: BigInt(5000)
      }
      // feeRateSatPerVbyte: 21 // Optional: specific fee rate
    };
    const prepareResponse = await sdk.preparePayOnchain(request);

    // Check if the fees are acceptable before proceeding
    const totalFeesSat = prepareResponse.totalFeesSat;
    console.log(`Total fees for onchain payment: ${totalFeesSat} sats`);
    return prepareResponse;
  } catch (err) {
    console.error("Error preparing onchain payment:", err);
  }
};
```

If you want to set a custom fee rate when the Bitcoin transaction is claimed:
```typescript
const preparePayOnchainWithFeeRate = async (sdk: LiquidSdk): Promise<PreparePayOnchainResponse | undefined> => {
  try {
    const optionalSatPerVbyte = 21; // Example fee rate

    const request: PreparePayOnchainRequest = {
      amount: {
        type: PayAmountVariant.BITCOIN,
        receiverAmountSat: BigInt(5000)
      },
      feeRateSatPerVbyte: optionalSatPerVbyte
    };
    const prepareResponse = await sdk.preparePayOnchain(request);

    // Check if the fees are acceptable before proceeding
    const claimFeesSat = prepareResponse.claimFeesSat;
    const totalFeesSat = prepareResponse.totalFeesSat;
    console.log(`Claim fees: ${claimFeesSat} sats, Total fees: ${totalFeesSat} sats`);
    return prepareResponse;
  } catch (err) {
    console.error("Error preparing onchain payment with fee rate:", err);
  }
};
```

##### Draining all funds
When you want send all funds from your wallet to another address.
```typescript
const preparePayOnchainDrain = async (sdk: LiquidSdk): Promise<PreparePayOnchainResponse | undefined> => {
  try {
    const request: PreparePayOnchainRequest = {
      amount: {
        type: PayAmountVariant.DRAIN
      }
      // feeRateSatPerVbyte: 21 // Optional: specific fee rate
    };
    const prepareResponse = await sdk.preparePayOnchain(request);

    // Check if the fees are acceptable before proceeding
    const totalFeesSat = prepareResponse.totalFeesSat;
    console.log(`Total fees for draining funds onchain: ${totalFeesSat} sats`);
    return prepareResponse;
  } catch (err) {
    console.error("Error preparing onchain drain payment:", err);
  }
};
```

<h4 id="executing-onchain-payments">Executing Onchain Payments</h4>
Once you checked the amounts and the fees are acceptable, you can continue with sending the payment.
Note that one of the arguments will be the result from the `prepare` call above.
```typescript
import { type LiquidSdk, type PreparePayOnchainResponse, type PayOnchainRequest, type SendPaymentResponse } from '@breeztech/breez-sdk-liquid/web';

const executePayOnchain = async (sdk: LiquidSdk, prepareResponse: PreparePayOnchainResponse): Promise<SendPaymentResponse | undefined> => {
  try {
    const destinationAddress = 'bc1...'; // User's Bitcoin destination address

    const request: PayOnchainRequest = {
      address: destinationAddress,
      prepareResponse
    };
    const payOnchainRes = await sdk.payOnchain(request);
    console.log("Onchain payment sent:", payOnchainRes);
    return payOnchainRes;
  } catch (err) {
    console.error("Error executing onchain payment:", err);
  }
};
```

### Using LNURL
Interacting with a LNURL endpoint consists of two steps:
1.  Parse the LNURL string using `parse(lnurl_url).await`. This returns a `Result<InputType>`. The specific `InputType` you receive will tell you what kind of endpoint this is, as well as give you the relevant endpoint parameters.
2.  Call the corresponding service method. For example, for LNURL-pay, that is `LiquidSdk::lnurlPay()`.

#### LNURL Authentication
```typescript
import { type LiquidSdk, InputTypeVariant, LnUrlCallbackStatusVariant, type LnUrlAuthRequestData } from '@breeztech/breez-sdk-liquid/web';

const lnurlAuthenticate = async (sdk: LiquidSdk) => {
  try {
    // Endpoint can also be of the form:
    // keyauth://domain.com/auth?key=val
    const lnurlAuthUrl =
          'lnurl1dp68gurn8ghj7mr0vdskc6r0wd6z7mrww4excttvdankjm3lw3skw0tvdankjm3xdvcn6vtp8q6n2dfsx5mrjwtrxdjnqvtzv56rzcnyv3jrxv3sxqmkyenrvv6kve3exv6nqdtyv43nqcmzvdsnvdrzx33rsenxx5unqc3cxgeqgntfgu';

    const input = await sdk.parse(lnurlAuthUrl);
    if (input.type === InputTypeVariant.LN_URL_AUTH) {
      const result = await sdk.lnurlAuth(input.data as LnUrlAuthRequestData); // Cast to specific type
      if (result.type === LnUrlCallbackStatusVariant.OK) {
        console.log('Successfully authenticated');
      } else {
        console.log('Failed to authenticate', result);
      }
    } else {
      console.log("Parsed input is not LNURL-Auth:", input);
    }
  } catch (err) {
    console.error("Error during LNURL authentication:", err);
  }
};
```
**Supported Specs for LNURL-Auth:**
- [LUD-01](https://github.com/lnurl/luds/blob/luds/01.md) LNURL bech32 encoding
- [LUD-04](https://github.com/lnurl/luds/blob/luds/04.md) `auth` base spec
- [LUD-17](https://github.com/lnurl/luds/blob/luds/17.md) Support for keyauth prefix with non-bech32-encoded LNURL URLs

#### LNURL Pay
During the prepare step, the SDK ensures that the inputs are valid with respect to the LNURL-pay request, and also returns the relative fees related to the payment so they can be confirmed. If the LNURL-pay invoice includes a [Magic Routing Hint](https://docs.boltz.exchange/v/api/magic-routing-hints) for a direct Liquid payment, the fees will reflect this.

##### Preparing LNURL Payments
###### Setting the receiver amount
When you want the payment recipient to receive a specific amount. The SDK will also validate that the amount is within the sendable limits of the LNURL-pay request.
```typescript
import { type LiquidSdk, InputTypeVariant, PayAmountVariant, type PrepareLnUrlPayResponse, type LnUrlPayRequestData, type PayAmount, type PrepareLnUrlPayRequest } from '@breeztech/breez-sdk-liquid/web';

const prepareLnurlPayment = async (sdk: LiquidSdk): Promise<PrepareLnUrlPayResponse | undefined> => {
  try {
    // Endpoint can also be of the form:
    // lnurlp://domain.com/lnurl-pay?key=val
    const lnurlPayUrl = 'lightning@address.com'; // Or an actual LNURL-Pay string

    const input = await sdk.parse(lnurlPayUrl);
    if (input.type === InputTypeVariant.LN_URL_PAY) {
      const amount: PayAmount = {
        type: PayAmountVariant.BITCOIN,
        receiverAmountSat: BigInt(5000)
      };
      const optionalComment = '<comment>';
      const optionalValidateSuccessActionUrl = true;

      const request: PrepareLnUrlPayRequest = {
        data: input.data as LnUrlPayRequestData, // Cast to specific type
        amount,
        bip353Address: input.bip353Address,
        comment: optionalComment,
        validateSuccessActionUrl: optionalValidateSuccessActionUrl
      };
      const prepareResponse = await sdk.prepareLnurlPay(request);

      // If the fees are acceptable, continue to create the LNURL Pay
      const feesSat = prepareResponse.feesSat;
      console.log(`Fees for LNURL-Pay: ${feesSat} sats`);
      return prepareResponse;
    } else {
      console.log("Parsed input is not LNURL-Pay:", input);
    }
  } catch (err) {
    console.error("Error preparing LNURL-Pay:", err);
  }
};
```

###### Draining all funds
When you want to send all funds from your wallet to the payment recipient.
```typescript
const prepareLnurlPayDrain = async (sdk: LiquidSdk): Promise<PrepareLnUrlPayResponse | undefined> => {
  try {
    const lnurlPayUrl = 'lightning@address.com'; // Or an actual LNURL-Pay string
    const input = await sdk.parse(lnurlPayUrl);

    if (input.type === InputTypeVariant.LN_URL_PAY) {
        const amount: PayAmount = { type: PayAmountVariant.DRAIN };
        const request: PrepareLnUrlPayRequest = {
            data: input.data as LnUrlPayRequestData,
            amount,
            bip353Address: input.bip353Address,
        };
        const prepareResponse = await sdk.prepareLnurlPay(request);
        console.log(`Fees for LNURL-Pay drain: ${prepareResponse.feesSat} sats`);
        return prepareResponse;
    } else {
        console.log("Parsed input is not LNURL-Pay:", input);
    }
  } catch (err) {
      console.error("Error preparing LNURL-Pay drain:", err);
  }
};
```

##### LNURL Payments
Once the payment has been prepared and the fees are accepted, all you have to do is pass the prepare response as an argument to the LNURL pay method.
```typescript
import { type LiquidSdk, type PrepareLnUrlPayResponse, type LnUrlPayRequest, type LnUrlPayResult } from '@breeztech/breez-sdk-liquid/web';

const executeLnurlPay = async (sdk: LiquidSdk, prepareResponse: PrepareLnUrlPayResponse): Promise<LnUrlPayResult | undefined> => {
  try {
    const request: LnUrlPayRequest = {
      prepareResponse
    };
    const result = await sdk.lnurlPay(request);
    console.log("LNURL-Pay result:", result);
    return result;
  } catch (err) {
    console.error("Error executing LNURL-Pay:", err);
  }
};
```
> **Developer note**
> By default when the LNURL-pay results in a success action with a URL, the URL is validated to check if there is a mismatch with the LNURL callback domain. You can disable this behaviour by setting the optional validation `PrepareLnUrlPayRequest` param to false.

**Supported Specs for LNURL-Pay:**
- [LUD-01](https://github.com/lnurl/luds/blob/luds/01.md) LNURL bech32 encoding
- [LUD-06](https://github.com/lnurl/luds/blob/luds/06.md) `payRequest` spec
- [LUD-09](https://github.com/lnurl/luds/blob/luds/09.md) `successAction` field for `payRequest`
- [LUD-16](https://github.com/lnurl/luds/blob/luds/16.md) LN Address
- [LUD-17](https://github.com/lnurl/luds/blob/luds/17.md) Support for lnurlp prefix with non-bech32-encoded LNURL URLs

#### Receiving payments using LNURL-Pay (LNURL-Pay Service)
Breez SDK - Nodeless *(Liquid Implementation)* users have the ability to receive Lightning payments using [LNURL-Pay](https://github.com/lnurl/luds/blob/luds/06.md).
LNURL-Pay requires a web service that serves LNURL-Pay requests. This service needs to communicate with the SDK in order to fetch the necessary metadata data and the associated payment request.
To interact with the SDK, the service uses a simple protocol over push notifications:
*   The service sends a push notification to the user's mobile app with the LNURL-Pay request and a reply URL.
*   The app responds to reply URL with the required data.
*   The service forwards the data to the payer.

##### General workflow for LNURL-Pay Service
The following workflow is application specific and the steps detailed below refer to the misty-breez wallet implementation which requires running **[breez-lnurl](https://github.com/breez/breez-lnurl)** service.

![pay](https://sdk-doc-liquid.breez.technology/images/lnurl-pay-sequence.png) <!-- Placeholder, image path might differ -->

**Step 1: Registering for an LNURL-Pay service**
Use a POST request to the service endpoint `https://app.domain/lnurlpay/[pubkey]` with the following payload to register the app for an LNURL-Pay service:
```json
{
 "time": 1231006505, // Current UNIX timestamp
 "webhook_url": "[notification service webhook URL]",
 "username": "[optional username]",
 "signature": "[signed message]"
}
```
The `signature` refers to the result of a message signed by the private key of the `pubkey`, where the message is comprised of the following text:
`[time]-[webhook_url]`
or, when the optional `username` field is set:
`[time]-[webhook_url]-[username]`
where `time`, `webhook_url` and `username` are the payload fields.
The service responds with following payload:
```json
{
 "lnurl": "[LNURL-pay encoded endpoint]",
 "lightning_address": "username@app.domain" // Only set when username is included
}
```
> **Developer note**
> When a user changes their already registered username, this previous username is freely available to be registered by another user.

**Step 2: Processing an LNURL-Pay request**
When an LNURL-Pay GET request is received at `https://app.domain/lnurlp/[identifier]` (or `https://app.domain/.well-known/lnurlp/[identifier]` for lightning addresses) the service then sends a push notification to the app with the LNURL-Pay request and a callback URL. The payload may look like the following:
```json
{
 "template": "lnurlpay_info",
 "data": {
  "reply_url": "https://app.domain/respond/[request_id]",
  "callback_url": "https://app.domain/lnurlpay/[identifier]/invoice"
  }
}
```
The `reply_url` is used by the app to respond to the LNURL-Pay request.
The `callback_url` is the LNURL-Pay callback URL, used by the payer to fetch the invoice.

**Step 3: Responding to the callback url**
When the app receives the push notification, it parses the payload and then uses the `reply_url` to respond with the required data, for example:
```json
{
 "callback": "https://app.domain/lnurlpay/[identifier]/invoice",
 "maxSendable": 10000,
 "minSendable": 1000,
 "metadata": "[[\"text/plain\",\"Pay to Breez\"]]",
 "tag": "payRequest"
}
```
The service receives the response from the app and forwards it to the sender.

**Step 4: Fetching a bolt11 invoice**
The sender fetches a bolt11 invoice by invoking a GET request to the `callback` URL when a specific amount is added as a query parameter. For example:
`https://app.domain/lnurlpay/[identifier]/invoice?amount=1000`
An additional push notification is triggered to send the invoice request to the app. Then the app responds with the bolt11 invoice data.

**Step 5: Paying the invoice**
In the last step, the payer pays the received bolt11 invoice. Follow the steps [here](#mobile-notifications) to receive payments via push notifications.

**Reference implementation for LNURL-Pay Service:**
For a complete reference implementation, see:
*   [Breez's NotificationService (iOS example)](https://github.com/breez/misty-breez/blob/main/ios/NotificationService/NotificationService.swift)
*   [Breez's LNURL-Pay service](https://github.com/breez/breez-lnurl)

#### LNURL Withdraw
```typescript
import { type LiquidSdk, InputTypeVariant, type LnUrlWithdrawRequestData, type LnUrlWithdrawRequest, type LnUrlWithdrawResult } from '@breeztech/breez-sdk-liquid/web';

const executeLnurlWithdraw = async (sdk: LiquidSdk): Promise<LnUrlWithdrawResult | undefined> => {
  try {
    // Endpoint can also be of the form:
    // lnurlw://domain.com/lnurl-withdraw?key=val
    const lnurlWithdrawUrl =
          'lnurl1dp68gurn8ghj7mr0vdskc6r0wd6z7mrww4exctthd96xserjv9mn7um9wdekjmmw843xxwpexdnxzen9vgunsvfexq6rvdecx93rgdmyxcuxverrvcursenpxvukzv3c8qunsdecx33nzwpnvg6ryc3hv93nzvecxgcxgwp3h33lxk';

    const input = await sdk.parse(lnurlWithdrawUrl);
    if (input.type === InputTypeVariant.LN_URL_WITHDRAW) {
      const amountMsat = input.data.minWithdrawable; // Or any amount within min/max
      const request: LnUrlWithdrawRequest = {
        data: input.data as LnUrlWithdrawRequestData, // Cast to specific type
        amountMsat,
        description: 'comment' // Optional description
      };
      const lnUrlWithdrawResult = await sdk.lnurlWithdraw(request);
      console.log("LNURL-Withdraw result:", lnUrlWithdrawResult);
      return lnUrlWithdrawResult;
    } else {
      console.log("Parsed input is not LNURL-Withdraw:", input);
    }
  } catch (err) {
    console.error("Error during LNURL withdraw:", err);
  }
};
```
**Supported Specs for LNURL-Withdraw:**
- [LUD-01](https://github.com/lnurl/luds/blob/luds/01.md) LNURL bech32 encoding
- [LUD-03](https://github.com/lnurl/luds/blob/luds/03.md) `withdrawRequest` spec
- [LUD-17](https://github.com/lnurl/luds/blob/luds/17.md) Support for lnurlw prefix with non-bech32-encoded LNURL URLs

### Messages and Signing
Through signing and verifying messages we can provide proof that a digital signature was created by a private key.

#### Signing a message
By signing a message using the SDK we can provide a digital signature. Anyone with the `message`, `pubkey` and `signature` can verify the signature was created by the private key of this pubkey.
```typescript
import { type LiquidSdk, type SignMessageRequest, type SignMessageResponse } from '@breeztech/breez-sdk-liquid/web';

const signMessage = async (sdk: LiquidSdk): Promise<{ signature: string; pubkey: string } | undefined> => {
  try {
    const request: SignMessageRequest = {
      message: '<message to sign>'
    };
    const signMessageResponse = sdk.signMessage(request); // Note: This is synchronous in WASM

    // Get the wallet info for your pubkey
    const info = await sdk.getInfo();

    const signature = signMessageResponse.signature;
    const pubkey = info.walletInfo.pubkey;

    console.log(`Pubkey: ${pubkey}`);
    console.log(`Signature: ${signature}`);
    return { signature, pubkey };
  } catch (error) {
    console.error("Error signing message:", error);
  }
};
```

#### Verifying a message
You can prove control of a private key by verifying a `message` with it's `signature` and `pubkey`.
```typescript
import { type LiquidSdk, type CheckMessageRequest, type CheckMessageResponse } from '@breeztech/breez-sdk-liquid/web';

const checkMessage = (sdk: LiquidSdk): boolean | undefined => { // Note: This is synchronous in WASM
  try {
    const request: CheckMessageRequest = {
      message: '<message>',
      pubkey: '<pubkey of signer>',
      signature: '<message signature>'
    };
    const checkMessageResponse = sdk.checkMessage(request);
    const isValid = checkMessageResponse.isValid;

    console.log(`Signature valid: ${isValid}`);
    return isValid;
  } catch (error) {
    console.error("Error checking message:", error);
  }
};
```

<h3 id="handling-multiple-assets">Handling multiple assets</h3>
The Liquid sidechain can also be used to send and receive other assets registered on the Liquid Network. Using the SDK you can send and receive these assets by using a Liquid payment with an additional asset ID. By default the SDK includes the metadata for [L-BTC and Tether USD](#default-asset-metadata). To include addition asset metadata, see [Adding asset metadata](#adding-asset-metadata).

<h4 id="adding-asset-metadata">Adding asset metadata</h4>
You can add addition asset metadata to the SDK when you configure it on connect. In the example below we will add the [PEGx EUR](https://assets.blockstream.info/18729918ab4bca843656f08d4dd877bed6641fbd596a0a963abbf199cfeb3cec.json) asset. Once the asset metadata is added, it can be used as an asset to send and receive.
You can find the asset metadata for other assets in the Mainnet [Liquid Asset Registry](https://assets.blockstream.info/) ([Testnet](https://assets-testnet.blockstream.info/)).
```typescript
import init, { defaultConfig, connect, type Config } from '@breeztech/breez-sdk-liquid/web';

const configureAssetMetadata = async () => {
  try {
    await init();
    // Create the default config
    const config: Config = defaultConfig('mainnet', '<your-Breez-API-key>');

    // Configure asset metadata
    config.assetMetadata = [
      ...(config.assetMetadata || []), // Keep existing default assets if any
      {
        assetId: '18729918ab4bca843656f08d4dd877bed6641fbd596a0a963abbf199cfeb3cec',
        name: 'PEGx EUR',
        ticker: 'EURx',
        precision: 8,
        // fiatId: "EUR" // Optional: If you want to fetch fiat rates for this asset
      }
    ];

    // Now connect with this modified config
    // const mnemonic = "<mnemonic words>";
    // const sdk = await connect({ mnemonic, config });
    // console.log("SDK connected with custom asset metadata.");
    return config; // Or return the SDK instance after connecting
  } catch (error) {
    console.error("Error configuring asset metadata:", error);
  }
};
```

##### Default asset metadata
**Mainnet**
| Name | Ticker | Asset ID | Precision |
| --- | --- | --- | --- |
| Bitcoin | BTC | 6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d | 8 |
| Tether USD | USDt | ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2 | 8 |

**Testnet**
| Name | Ticker | Asset ID | Precision |
| --- | --- | --- | --- |
| Testnet Bitcoin | BTC | 144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49 | 8 |
| Testnet Tether USD | USDt | b612eb46313a2cd6ebabd8b7a8eed5696e29898b87a43bff41c94f51acef9d73 | 8 |

<h4 id="fetching-the-asset-balances">Fetching the asset balances</h4>
Once connected, the asset balances can be retrieved.
```typescript
import { type LiquidSdk, type AssetBalance } from '@breeztech/breez-sdk-liquid/web';

const fetchAssetBalance = async (sdk: LiquidSdk): Promise<AssetBalance[] | undefined> => {
  try {
    const info = await sdk.getInfo();
    const assetBalances = info.walletInfo.assetBalances;
    console.log("Asset balances:", assetBalances);
    return assetBalances;
  } catch (error) {
    console.error("Error fetching asset balances:", error);
  }
};
```

<h4 id="receiving-a-non-bitcoin-asset">Receiving a non-Bitcoin asset</h4>
When receiving an asset via Liquid, we can generate a BIP21 URI with information regarding the payment of a specific asset. The amount to receive is optional and omitting it will result in an amountless BIP21 URI.
In the example below we are using the [Mainnet Tether USD](https://assets.blockstream.info/ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2.json) asset.
```typescript
import { type LiquidSdk, PaymentMethod, ReceiveAmountVariant, type PrepareReceiveResponse, type ReceiveAmount } from '@breeztech/breez-sdk-liquid/web';

const prepareReceiveAsset = async (sdk: LiquidSdk): Promise<PrepareReceiveResponse | undefined> => {
  try {
    // Create a Liquid BIP21 URI/address to receive an asset payment to.
    // Note: Not setting the amount will generate an amountless BIP21 URI.
    const usdtAssetId = 'ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2'; // Mainnet USDt
    const optionalAmount: ReceiveAmount = {
      type: ReceiveAmountVariant.ASSET,
      assetId: usdtAssetId,
      payerAmount: 1.50 // Amount of the asset (e.g., 1.50 USDt)
    };

    const prepareResponse = await sdk.prepareReceivePayment({
      paymentMethod: PaymentMethod.LIQUID_ADDRESS,
      amount: optionalAmount
    });

    // If the fees are acceptable, continue to create the Receive Payment
    const receiveFeesSat = prepareResponse.feesSat; // Fees are still in L-BTC (sats)
    console.log(`Fees for receiving asset: ${receiveFeesSat} sats`);
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing to receive asset:", error);
  }
};
```

<h4 id="sending-a-non-bitcoin-asset">Sending a non-Bitcoin asset</h4>
When sending an asset via Liquid, a BIP21 URI or Liquid address can be used as the destination. If a Liquid address is used, the optional prepare request amount **must** be set. If a BIP21 URI is used, either the BIP21 URI amount or optional prepare request amount **must** be set. When both amounts are set, the SDK will prioritize the **request amount** over the BIP21 amount.
In the example below we are using the [Mainnet Tether USD](https://assets.blockstream.info/ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2.json) asset.
```typescript
import { type LiquidSdk, PayAmountVariant, type PrepareSendResponse, type PayAmount, type PrepareSendRequest } from '@breeztech/breez-sdk-liquid/web';

const prepareSendPaymentAsset = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    // Set the Liquid BIP21 or Liquid address you wish to pay
    const destination = '<Liquid BIP21 or address for asset>';
    // If the destination is an address or an amountless BIP21 URI,
    // you must specify an asset amount

    const usdtAssetId = 'ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2';
    const optionalAmount: PayAmount = {
      type: PayAmountVariant.ASSET,
      assetId: usdtAssetId,
      receiverAmount: 1.50, // Amount of the asset
      estimateAssetFees: false // Set to true if you want to estimate fees in the asset itself
    };

    const request: PrepareSendRequest = {
      destination,
      amount: optionalAmount
    };
    const prepareResponse = await sdk.prepareSendPayment(request);

    // If the fees are acceptable, continue to create the Send Payment
    const sendFeesSat = prepareResponse.feesSat; // Fees are in L-BTC (sats)
    console.log(`Fees for sending asset: ${sendFeesSat} sats`);
    if (prepareResponse.assetFees) {
        console.log(`Asset fees: ${prepareResponse.assetFees} of ${usdtAssetId}`);
    }
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing to send asset:", error);
  }
};
```

<h4 id="paying-fees-with-a-non-bitcoin-asset">Paying fees with a non-Bitcoin asset</h4>
For some assets, like [Tether USD](https://assets.blockstream.info/ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2.json), you can pay the sending transaction fees with the asset.

> **Developer note**
> When adding additional [asset metadata](#adding-asset-metadata), the optional **fiat ID** has to be set and the Payjoin provider has to support paying fees for this asset. When the asset is not supported, the **asset fees** in the prepare send payment response will be not set.

In the prepare send payment step, set the **estimate asset fees** param to `true` to validate and calculate the **asset fees**.
```typescript
import { type LiquidSdk, PayAmountVariant, type PrepareSendResponse, type PayAmount, type PrepareSendRequest } from '@breeztech/breez-sdk-liquid/web';

const prepareSendPaymentAssetWithAssetFees = async (sdk: LiquidSdk): Promise<PrepareSendResponse | undefined> => {
  try {
    const destination = '<Liquid BIP21 or address for asset>';
    const usdtAssetId = 'ce091c998b83c78bb71a632313ba3760f1763d9cfcffae02258ffa9865a37bd2';
    const amount: PayAmount = {
      type: PayAmountVariant.ASSET,
      assetId: usdtAssetId,
      receiverAmount: 1.50,
      estimateAssetFees: true // Set to true to estimate fees in the asset
    };

    const request: PrepareSendRequest = {
      destination,
      amount
    };
    const prepareResponse = await sdk.prepareSendPayment(request);

    console.log(`L-BTC Fees: ${prepareResponse.feesSat} sats`);
    if (prepareResponse.assetFees) {
        console.log(`Fees in Asset (${prepareResponse.assetId}): ${prepareResponse.assetFees}`);
    } else {
        console.log("Asset fees not available for this asset/configuration.");
    }
    return prepareResponse;
  } catch (error) {
    console.error("Error preparing to send asset with asset fees:", error);
  }
};
```
If the **asset fees** are set in the response, then set the **use asset fees** to `true` to pay fees with the asset. You can still pay fees in satoshi if you set the **use asset fees** to `false` (default).
```typescript
import { type LiquidSdk, type PrepareSendResponse, type SendPaymentRequest, type SendPaymentResponse } from '@breeztech/breez-sdk-liquid/web';

const sendPaymentWithAssetFees = async (sdk: LiquidSdk, prepareResponse: PrepareSendResponse): Promise<SendPaymentResponse | undefined> => {
  try {
    // Set the use asset fees param to true if assetFees were estimated and are acceptable
    const useAssetFees = prepareResponse.assetFees !== undefined && prepareResponse.assetFees > 0;

    const request: SendPaymentRequest = {
      prepareResponse,
      useAssetFees
    };
    const sendResponse = await sdk.sendPayment(request);
    const payment = sendResponse.payment;
    console.log("Asset payment sent (fees paid with asset if applicable):", payment);
    return sendResponse;
  } catch (error) {
    console.error("Error sending asset payment with asset fees:", error);
  }
};
```

### Mobile Notifications

Implementing mobile notifications with the WASM SDK primarily involves setting up webhooks, as the SDK itself runs in the browser and doesn't directly handle native mobile push notifications. The general flow involves your backend service (NDS) receiving events and then using web technologies (like WebSockets or Server-Sent Events) to notify your web application, or by having the web app poll for updates.

If your web application is a Progressive Web App (PWA) and has capabilities to receive web push notifications, your NDS could send these.

#### Setting up an NDS (Notification Delivery Service)
Receiving push notifications involves using an Notification Delivery Service (NDS) as an intermediary to receive the webhook event from one of the SDK services. These can be currently one of several services that provide information about events that the SDK registers for. For example, LNURL-pay requests or swap events from the swap service. The NDS then processes this information and dispatches a push notification to the intended mobile device, ensuring the user receives timely updates about incoming events. This architecture necessitates developers set up and maintain their own NDS, tailored to handle and forward these notifications efficiently. An example payload when a `swap_updated` POST request to the webhook URL contains the following JSON formatted structure:

```json
{
    "template": "swap_updated",
    "data": {
        "id": "",    // The hashed swap id
        "status": "" // The latest swap status
    }
}
```

The need to run your own NDS is because it's configured to send push notifications to your application users and therefore should be configured with the required keys and certificates. You can use our [reference NDS implementation](https://github.com/breez/notify) as a starting point or as is. Our implementation of the NDS expects URLs in the following format:
`https://your-nds-service.com/notify?platform=<ios|android|web>&token=[PUSH_TOKEN_OR_ENDPOINT]`

This is the same format used when [using webhooks](#using-webhooks) in the SDK, replacing the `PUSH_TOKEN_OR_ENDPOINT` with the mobile push token or web push subscription endpoint. Once the NDS receives such a request it will send a push notification to the corresponding device/browser.

#### Using Webhooks
<h5 id="registering-a-webhook">Registering a Webhook</h5>
Once your vendor NDS is set up and can accept POST requests from the SDK services, you can register the webhook URL within your main application by calling the register webhook API as follows:

```typescript
import { type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const registerWebHook = async (sdk: LiquidSdk) => { // Renamed to avoid conflict
  try {
    // For web, the token might be a VAPID public key or a specific session ID
    // depending on how your NDS and web push/notification system is set up.
    // This example assumes a generic endpoint for web notifications.
    await sdk.registerWebhook('https://your-nds-service.com/notify?platform=web&token=<YOUR_WEB_PUSH_SUBSCRIPTION_ENDPOINT_OR_ID>');
    console.log("Webhook registered successfully.");
  } catch (err) {
    console.error("Error registering webhook:", err);
  }
};
```
When the NDS receives a POST request for the registered webhook URL, it will forward the request data via push notification (or other web mechanism) to the application.

<h5 id="unregistering-a-webhook">Unregistering a Webhook</h5>
When a webhook is no longer needed you can unregister the webhook as follows:
```typescript
import { type LiquidSdk } from '@breeztech/breez-sdk-liquid/web';

const unregisterWebHook = async (sdk: LiquidSdk) => { // Renamed to avoid conflict
  try {
    await sdk.unregisterWebhook();
    console.log("Webhook unregistered successfully.");
  } catch (err) {
    console.error("Error unregistering webhook:", err);
  }
};
```
> **Developer note**
> Any payments that use a swap service will use the same registered webhook URL until the swap is complete.

## End-User Fees

**The Breez SDK is free for developers. There are small fees for *end-users* to send and receive payments.**
- [Sending Lightning Payments](#sending-lightning-payments-fees)
- [Receiving Lightning Payments](#receiving-lightning-payments-fees)
- [Sending to a BTC Address](#sending-to-a-btc-address-fees)
- [Receiving from a BTC Address](#receiving-from-a-btc-address-fees)

**Note:** The SDK uses Liquid confidential transactions. This means a discount v-size is used to calculate transaction fees. For more details, see [ELIP-200](https://github.com/ElementsProject/ELIPs/blob/main/elip-0200.mediawiki).

<h3 id="sending-lightning-payments-fees">Sending Lightning Payments Fees</h3>
Sending Lightning payments involves a submarine swap and two Liquid on-chain transactions. The process is as follows:
1. User broadcasts an L-BTC transaction to a Liquid lockup address.
2. Swapper pays the invoice, sending to the recipient, and then gets a preimage.
3. Swapper broadcasts an L-BTC transaction to claim the funds from the Liquid lockup address.

The fee a user pays to send a Lightning payment is composed of three parts:
1. **Lockup Transaction Fee:** ~34 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
2. **Claim Transaction Fee:** ~19 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
3. **Swap Service Fee:** 0.1% fee on the amount sent.
Note: swap service fee is dynamic and can change. Currently, it is 0.1%.
> **Example**: If the user sends 10k sats, the fee would be:
> - 34 sats [Lockup Transaction Fee] + 19 sats [Claim Transaction Fee] + 10 sats [Swapper Service Fee] = 63 sats

<h3 id="receiving-lightning-payments-fees">Receiving Lightning Payments Fees</h3>
Receiving Lightning payments involves a reverse submarine swap and requires two Liquid on-chain transactions. The process is as follows:
1. Sender pays the Swapper invoice.
2. Swapper broadcasts an L-BTC transaction to a Liquid lockup address.
3. SDK claims the funds from the Liquid lockup address and then exposes the preimage.
4. Swapper uses the preimage to claim the funds from the Liquid lockup address.

The fee a user pays to receive a Lightning payment is composed of three parts:
1. **Lockup Transaction Fee:** ~27 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
2. **Claim Transaction Fee:** ~20 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
3. **Swap Service Fee:** 0.25% fee on the amount received.
Note: swap service fee is dynamic and can change. Currently, it is 0.25%.
> **Example**: If the sender sends 10k sats, the fee for the end-user would be:
> - 27 sats [Lockup Transaction Fee] + 20 sats [Claim Transaction Fee] + 25 sats [Swapper Service Fee] = 72 sats

<h3 id="sending-to-a-btc-address-fees">Sending to a BTC Address Fees</h3>
Sending to a BTC address involves a trustless chain swap, 2 Liquid on-chain transactions, and 2 BTC on-chain transactions. The process is as follows:
1. SDK broadcasts an L-BTC transaction to a Liquid lockup address.
2. Swapper broadcasts a BTC transaction to a Bitcoin lockup address.
3. Recipient claims the funds from the Bitcoin lockup address.
4. Swapper claims the funds from the Liquid lockup address.

The fee to send to a BTC address is composed of four parts:
1. **L-BTC Lockup Transaction Fee**: ~34 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
2. **BTC Lockup Transaction Fee**: the swapper charges a mining fee based on the current bitcoin mempool usage.
3. **Swap Service Fee:** 0.1% fee on the amount sent.
4. **BTC Claim Transaction Fee:** the SDK fees to claim BTC funds to the destination address, based on the current Bitcoin mempool usage.
Note: swap service fee is dynamic and can change. Currently, it is 0.1%.
> **Example**: If the user sends 100k sats, the mining fees returned by the Swapper are 2000 sats, and the claim fees for the user are 1000 sats—the fee would be:
> - 34 sats [Lockup Transaction Fee] + 2000 sats [BTC Claim Transaction Fee] + 100 sats [Swapper Service Fee] + 1000 sats [BTC Lockup Transaction Fee] = 3132 sats

<h3 id="receiving-from-a-btc-address-fees">Receiving from a BTC Address Fees</h3>
Receiving from a BTC address involves a trustless chain swap, 2 Liquid on-chain transactions, and 2 BTC on-chain transactions.
The process is as follows:
1. Sender broadcasts a BTC transaction to the Bitcoin lockup address.
2. Swapper broadcasts an L-BTC transaction to a Liquid lockup address.
3. SDK claims the funds from the Liquid lockup address.
4. Swapper claims the funds from the Bitcoin lockup address.

The fee to receive from a BTC address is composed of three parts:
1. **L-BTC Claim Transaction Fee:** ~20 sats (0.1&nbsp;sat/discount&nbsp;vbyte).
2. **BTC Claim Transaction Fee:** the swapper charges a mining fee based on the Bitcoin mempool usage at the time of the swap.
3. **Swapper Service Fee:** the swapper charges a 0.1% fee on the amount received.
Note: swapper service see is dynamic and can change. Currently, it is 0.1%.
> **Example**: If the sender sends 100k sats and the mining fees returned by the Swapper are 2000 sats—the fee for the end-user would be:
> - 20 sats [Claim Transaction Fee] + 100 sats [Swapper Service Fee] + 2000 sats [BTC Claim Transaction Fee] = 2120 sats

## Best Practices

### Syncing
Always ensure the SDK instance is synced before performing actions:
```typescript
import { type LiquidSdk, type SdkEvent } from '@breeztech/breez-sdk-liquid/web';

const waitForSynced = async (sdk: LiquidSdk): Promise<void> => {
  const eventPromise = new Promise<void>((resolve) => {
    const listener = {
      onEvent: (event: SdkEvent) => {
        if (event.type === 'synced') {
          // Assuming the listener should be removed after first sync for this specific promise.
          // If you need persistent listening, manage listener removal separately.
          sdk.removeEventListener(listenerId).catch(console.error); // Requires listenerId to be accessible
          resolve();
        }
      }
    };
    // Add listener and store its ID for removal
    let listenerId: string;
    sdk.addEventListener(listener).then(id => listenerId = id).catch(console.error);
  });

  // Wait for sync event or timeout after 30 seconds
  return Promise.race([
    eventPromise,
    new Promise<void>((_, reject) => setTimeout(() => reject(new Error('Sync timeout')), 30000))
  ]);
};

const performActionAfterSync = async (sdk: LiquidSdk) => { // Renamed to avoid conflict
  try {
    await waitForSynced(sdk);
    // Now it's safe to perform actions
    const info = await sdk.getInfo();
    console.log("SDK is synced. Wallet Info:", info);
  } catch (error) {
    console.error("Error during sync or action:", error);
  }
};
```

### Error Handling
Implement robust error handling:
```typescript
const safeOperation = async (operation: () => Promise<any>) => {
  try {
    return await operation();
  } catch (error: any) { // Explicitly type error
    console.error(`Operation failed: ${error.message}`);
    // Handle specific error types appropriately
    if (error.message.includes('insufficient funds')) {
      // Handle insufficient funds error
    } else if (error.message.includes('connection')) {
      // Handle connection error
    }
    throw error; // Re-throw or handle at a higher level
  }
};

// Usage example:
// Assuming sdk and prepareResponse are defined
// const result = await safeOperation(() => sdk.sendPayment({ prepareResponse }));
```

### Connection Lifecycle
Manage the connection lifecycle properly:
```typescript
// Initialize only once per session (example in initWallet above)
// const sdk = await initWallet();

// Use SDK
// ...

// Disconnect when done (example in disconnectSdk above or in component unmount)
// try {
//   if (sdk) await sdk.disconnect();
// } catch (error: any) {
//   console.error(`Error disconnecting: ${error.message}`);
// }
```

### Fee Handling
Always check fees before executing payments:
```typescript
import { type LiquidSdk, type PrepareSendRequest, type PrepareSendResponse } from '@breeztech/breez-sdk-liquid/web';

const executeSafePaymentWithFeeCheck = async (sdk: LiquidSdk, prepareRequest: PrepareSendRequest, maxAcceptableFee: bigint = BigInt(1000)): Promise<any> => {
  // Get fee information
  const prepareResponse: PrepareSendResponse = await sdk.prepareSendPayment(prepareRequest);

  const feesSat = prepareResponse.feesSat || BigInt(0);

  // Check if fees are acceptable before proceeding
  if (feesSat <= maxAcceptableFee) {
    // Execute payment
    return await sdk.sendPayment({
      prepareResponse
    });
  } else {
    // Fees are too high
    throw new Error(`Fees too high: ${feesSat} sats (maximum: ${maxAcceptableFee} sats)`);
  }
};
```

### Browser Storage Considerations
When working with browser storage, consider:
```typescript
const secureMnemonicStorage = {
  // Save mnemonic - in production, consider more secure options than localStorage
  save: (mnemonic: string) => {
    // For demonstration only - not secure for production
    // Consider Web Crypto API for encryption before storing
    localStorage.setItem('encrypted_mnemonic', btoa(mnemonic)); // Basic Base64, not encryption
  },

  // Retrieve mnemonic
  retrieve: (): string | null => {
    const storedMnemonic = localStorage.getItem('encrypted_mnemonic');
    return storedMnemonic ? atob(storedMnemonic) : null;
  },

  // Clear mnemonic
  clear: () => {
    localStorage.removeItem('encrypted_mnemonic');
  }
};
```

## Security Considerations
1.  **Protecting Mnemonics**
    *   Never hardcode mnemonics in your code
    *   Store encrypted or in secure storage (e.g., using Web Crypto API before placing in `localStorage` or `IndexedDB`)
    *   Consider using a [custom signer](#custom-signer-support) for production apps
    *   For web apps, consider using hardware wallet integration if feasible for your use case.
2.  **API Key Security**
    *   Store API keys in environment variables on your build server, not directly in client-side code.
    *   Consider a backend proxy for API calls that require the API key, so the key is never exposed to the frontend.
    *   Don't commit API keys to source control.
3.  **Validating Inputs**
    *   Always validate payment destinations and amounts on the client-side before sending to the SDK.
    *   Check amounts are within reasonable limits.
    *   Sanitize and validate all external inputs.
4.  **WASM-Specific Considerations**
    *   Load WASM files from a trusted source (usually your own domain or a trusted CDN).
    *   Be aware of cross-origin restrictions and CORS settings if loading from a different domain.
    *   Consider using Content Security Policy (CSP) to restrict where WASM can be loaded from (`script-src 'wasm-unsafe-eval' ...` or specific hash/nonce).
5.  **Browser Environment**
    *   Use HTTPS for all connections to protect data in transit.
    *   Be aware of limited storage options in browsers and their security implications.
    *   Consider the implications of users clearing browser storage (mnemonics, SDK state) and provide robust recovery options (e.g., seed phrase backup prompts).

## Complete Web App Example (Conceptual React)
Here's a simplified React component that demonstrates how to use the SDK in a web app:
```jsx
import React, { useEffect, useState, useCallback } from 'react';
import init, { connect, defaultConfig, type LiquidSdk, type WalletInfo, type Payment, type SdkEvent, PaymentMethod, ReceiveAmountVariant, PayAmountVariant, type PrepareReceiveResponse, type PrepareSendResponse } from '@breeztech/breez-sdk-liquid/web';

// A simple (and insecure) way to generate/store mnemonic for example purposes
const generateNewMnemonic = () => {
    // In a real app, use a proper library like bip39
    console.warn("Using placeholder mnemonic generation. Use a proper library in production.");
    return "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
};

function WalletComponent() {
  const [sdk, setSdk] = useState<LiquidSdk | null>(null);
  const [walletInfo, setWalletInfo] = useState<WalletInfo | null>(null);
  const [transactions, setTransactions] = useState<Payment[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [listenerId, setListenerId] = useState<string | null>(null);

  const refreshWalletData = useCallback(async (currentSdk: LiquidSdk) => {
    if (!currentSdk) return;

    try {
      const info = await currentSdk.getInfo();
      const txs = await currentSdk.listPayments({});

      setWalletInfo(info.walletInfo);
      setTransactions(txs);
    } catch (err: any) {
      console.error('Error refreshing wallet data:', err);
      setError(err.message || "Failed to refresh data");
    }
  }, []);

  // Initialize SDK on component mount
  useEffect(() => {
    const initWallet = async () => {
      try {
        setIsLoading(true);
        setError(null);

        // Initialize WASM module
        await init();

        // Get saved mnemonic or create new one
        let mnemonic = localStorage.getItem('walletMnemonic');
        if (!mnemonic) {
            mnemonic = generateNewMnemonic(); // Replace with secure generation/input
            localStorage.setItem('walletMnemonic', mnemonic);
        }

        // Create config with API key
        const config = defaultConfig('testnet', process.env.REACT_APP_BREEZ_API_KEY || "YOUR_API_KEY_HERE"); // Use testnet for example

        // Connect to Breez SDK
        const sdkInstance = await connect({
          config,
          mnemonic,
        });

        // Set up event listener
        const eventListener = {
          onEvent: (event: SdkEvent) => {
            console.log('SDK event:', event.type, event);
            if (event.type === 'synced' || event.type === 'paymentSucceeded' || event.type === 'paymentFailed') {
              refreshWalletData(sdkInstance);
            }
          }
        };

        const id = await sdkInstance.addEventListener(eventListener);
        setListenerId(id);

        setSdk(sdkInstance);
        await refreshWalletData(sdkInstance);

      } catch (err: any) {
        console.error('Wallet initialization error:', err);
        setError(err.message || "Initialization failed");
      } finally {
        setIsLoading(false);
      }
    };

    initWallet();

    // Clean up on unmount
    return () => {
      if (sdk && listenerId) {
        sdk.removeEventListener(listenerId).catch(console.error);
      }
      if (sdk) {
        sdk.disconnect().catch(console.error);
      }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); //refreshWalletData dependency removed as it depends on sdk which is set here.

  // Handle receiving payment
  const handleReceive = async () => {
    if (!sdk) return;

    try {
      setIsLoading(true);
      // Prepare receive with amount of 5000 sats
      const prepareResponse: PrepareReceiveResponse | undefined = await sdk.prepareReceivePayment({
        paymentMethod: PaymentMethod.LIGHTNING,
        amount: {
          type: ReceiveAmountVariant.BITCOIN,
          payerAmountSat: BigInt(5000)
        }
      });

      if (!prepareResponse) throw new Error("Failed to prepare receive payment");

      // Create the invoice
      const response = await sdk.receivePayment({
        prepareResponse,
        description: 'Payment received via web app'
      });

      // Show the invoice to user
      alert(`Generated invoice: ${response.destination}`);
    } catch (err: any) {
      console.error('Error generating invoice:', err);
      setError(err.message || "Failed to generate invoice");
    } finally {
      setIsLoading(false);
    }
  };

  // Handle sending payment
  const handleSend = async (invoice: string) => {
    if (!sdk || !invoice) return;

    try {
      setIsLoading(true);
      // Prepare send payment
      const prepareResponse: PrepareSendResponse | undefined = await sdk.prepareSendPayment({
        destination: invoice
      });

      if (!prepareResponse) throw new Error("Failed to prepare send payment");

      // Check if fees are acceptable
      if (prepareResponse.feesSat && prepareResponse.feesSat > BigInt(100)) { // Example fee check
        const confirmed = window.confirm(`Fee is ${prepareResponse.feesSat} sats. Continue?`);
        if (!confirmed) {
          setIsLoading(false);
          return;
        }
      }

      // Send payment
      await sdk.sendPayment({
        prepareResponse
      });

      alert('Payment sent successfully!');
      if (sdk) refreshWalletData(sdk);
    } catch (err: any) {
      console.error('Error sending payment:', err);
      setError(err.message || "Failed to send payment");
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading && !sdk) { // Show loading only during initial setup
    return <div>Loading wallet...</div>;
  }

  if (error) {
    return <div>Error: {error} <button onClick={() => setError(null)}>Dismiss</button></div>;
  }

  return (
    <div>
      <h1>Breez SDK Wallet (WASM Example)</h1>

      {walletInfo && sdk ? (
        <div>
          <h2>Balance: {walletInfo.balanceSat ? walletInfo.balanceSat.toString() : '0'} sats</h2>
          <p>Pending Send: {walletInfo.pendingSendSat ? walletInfo.pendingSendSat.toString() : '0'} sats</p>
          <p>Pending Receive: {walletInfo.pendingReceiveSat ? walletInfo.pendingReceiveSat.toString() : '0'} sats</p>
          <button onClick={handleReceive} disabled={isLoading}>Receive 5000 sats (LN)</button>
          <button onClick={() => {
            const invoice = prompt('Enter BOLT11 invoice to pay:');
            if (invoice) handleSend(invoice);
          }} disabled={isLoading}>Send Payment</button>
          <button onClick={() => refreshWalletData(sdk)} disabled={isLoading}>Refresh Data</button>

          <h3>Transaction History ({transactions.length})</h3>
          {transactions.length > 0 ? (
            <ul style={{ maxHeight: '300px', overflowY: 'auto', border: '1px solid #ccc', padding: '10px' }}>
              {transactions.map((tx, index) => (
                <li key={tx.id || index} style={{ marginBottom: '5px', borderBottom: '1px solid #eee', paddingBottom: '5px' }}>
                  Type: {tx.paymentType} <br />
                  Amount: {tx.amountSat ? tx.amountSat.toString() : 'N/A'} sats <br />
                  Fees: {tx.feesSat ? tx.feesSat.toString() : 'N/A'} sats <br />
                  Status: {tx.status} <br />
                  Description: {tx.description || "N/A"} <br />
                  Date: {new Date(Number(tx.paymentTime) * 1000).toLocaleString()} <br />
                  ID: {tx.id}
                </li>
              ))}
            </ul>
          ) : <p>No transactions yet.</p>}
        </div>
      ) : isLoading ? <div>Loading wallet data...</div> : <p>SDK not initialized. Check console for errors.</p>}
    </div>
  );
}

export default WalletComponent;

```
