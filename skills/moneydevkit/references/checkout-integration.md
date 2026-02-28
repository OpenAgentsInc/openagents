# Checkout Integration

Use this path when the app needs hosted checkout UX, product catalog support, and API-coordinated payment flows.

## Credential Bootstrap

Preferred:

```bash
npx @moneydevkit/create@latest
```

Required env for checkout packages:

```env
MDK_ACCESS_TOKEN=...
MDK_MNEMONIC=...
```

## Next.js Path (`@moneydevkit/nextjs`)

Install:

```bash
npm install @moneydevkit/nextjs
```

Required wiring:

1. Client action uses `useCheckout()` to create checkout.
2. Hosted checkout route renders `<Checkout id={...} />`.
3. API endpoint exports `POST` from package route helper (`/api/mdk`).
4. Next config wraps with `withMdkCheckout`.
5. Success page verifies payment status (`useCheckoutSuccess`).

## Replit / Express Path (`@moneydevkit/replit`)

Install:

```bash
npm install @moneydevkit/replit express
```

Required wiring:

1. Mount `createMdkExpressRouter()` at `/api/mdk`.
2. Frontend uses `useCheckout()` and hosted `<Checkout />`.
3. Verify checkout paid state on success page.

## Feature Checklist

- Amount-based checkout (`type: AMOUNT`) works.
- Product checkout (`type: PRODUCTS`) works.
- Optional customer data collection fields behave as expected.
- L402 pay-per-call endpoints are gated and verify credentials.
