import { ErrorCorrectionLevel, QRCode } from '@liquid-js/qrcode-generator'
import { Effect } from 'effect'

import type {
  OpenAgentsHostedMdkClient,
  OpenAgentsHostedMdkClientError,
} from './hosted-mdk-client'
import { methodNotAllowed } from './http/responses'

type HttpResponse = globalThis.Response

const checkoutPagePattern = /^\/checkout\/([A-Za-z0-9]{8,64})$/

const invoiceQrSvg = (invoice: string): string => {
  try {
    const qr = new QRCode(0, ErrorCorrectionLevel.L)

    qr.addData(`LIGHTNING:${invoice.toUpperCase()}`, 'alphanumeric')
    qr.make()

    return qr.createSvgTag({ cellSize: 4, margin: 4, scalable: true })
  } catch {
    return ''
  }
}

const escapeHtml = (value: string): string =>
  value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')

const pageShell = (title: string, body: string, refresh: boolean): string =>
  `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
${refresh ? '<meta http-equiv="refresh" content="12" />' : ''}
<title>${escapeHtml(title)} | OpenAgents</title>
<style>
body { background:#0a0a0a; color:#e5e5e5; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; display:flex; justify-content:center; padding:48px 16px; }
main { max-width: 640px; width:100%; }
h1 { font-size: 18px; color:#f97316; }
p { line-height: 1.5; font-size: 14px; color:#a3a3a3; }
pre { background:#171717; border:1px solid #262626; padding:12px; white-space:pre-wrap; word-break:break-all; font-size:12px; border-radius:6px; }
a.button { display:inline-block; background:#f97316; color:#0a0a0a; padding:10px 16px; border-radius:6px; text-decoration:none; font-weight:bold; margin-top:8px; }
.status { margin-top:16px; font-size:13px; }
.qr { background:#ffffff; border-radius:8px; padding:8px; width:fit-content; max-width:320px; margin:16px 0; }
.qr svg { display:block; width:100%; max-width:304px; height:auto; }
.ok { color:#22c55e; }
</style>
</head>
<body><main>${body}</main></body>
</html>`

const htmlPage = (
  title: string,
  body: string,
  options: Readonly<{ refresh?: boolean; status?: number }> = {},
): HttpResponse =>
  new Response(pageShell(title, body, options.refresh === true), {
    headers: {
      'cache-control': 'no-store',
      'content-type': 'text/html; charset=utf-8',
    },
    status: options.status ?? 200,
  })

export type CheckoutPageRouteDependencies<Bindings> = Readonly<{
  hostedMdkClient: (env: Bindings) => OpenAgentsHostedMdkClient
}>

export const makeCheckoutPageRoutes = <Bindings,>(
  dependencies: CheckoutPageRouteDependencies<Bindings>,
) => ({
  routeCheckoutPageRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)
    const match = checkoutPagePattern.exec(url.pathname)

    if (match === null) {
      return undefined
    }

    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    const checkoutId = match[1] ?? ''
    const client = dependencies.hostedMdkClient(env)
    const statusRequest = {
      checkoutRef: `mdk_checkout.${checkoutId}`,
      environment:
        client.implementationState === 'live_provider_configured'
          ? ('production' as const)
          : ('sandbox' as const),
      providerRef: 'provider.openagents.mdk_hosted',
      sandbox: client.implementationState !== 'live_provider_configured',
      siteRef: null,
    }

    return Effect.gen(function* () {
      const status = yield* client.getCheckoutStatus(statusRequest).pipe(
        Effect.catch((error: OpenAgentsHostedMdkClientError) =>
          Effect.succeed({ error } as const),
        ),
      )

      if ('error' in status) {
        return htmlPage(
          'Checkout unavailable',
          `<h1>Checkout unavailable</h1>
<p>This checkout could not be loaded. It may have expired, or the payment
provider is unreachable. If you followed a payment link, ask for a fresh one.</p>`,
          { status: 404 },
        )
      }

      if (status.status === 'payment_received') {
        return htmlPage(
          'Payment received',
          `<h1>Payment received</h1>
<p class="status ok">This checkout is paid. You can close this page; the
purchase completes automatically.</p>`,
        )
      }

      if (status.status === 'expired') {
        return htmlPage(
          'Checkout expired',
          `<h1>Checkout expired</h1>
<p>This checkout expired before payment. Start the purchase again to get a
fresh invoice.</p>`,
          { status: 410 },
        )
      }

      const payload = yield* client
        .getPrivateL402PaymentPayload(statusRequest)
        .pipe(
          Effect.catch((error: OpenAgentsHostedMdkClientError) =>
            Effect.succeed({ error } as const),
          ),
        )

      if ('error' in payload || payload.bolt11 === null) {
        return htmlPage(
          'Awaiting payment',
          `<h1>Awaiting payment</h1>
<p>This checkout is awaiting payment, but its Lightning invoice could not be
loaded right now. Refresh in a few seconds.</p>`,
          { refresh: true },
        )
      }

      const invoice = escapeHtml(payload.bolt11)
      const qrSvg = invoiceQrSvg(payload.bolt11)

      return htmlPage(
        'Pay with Lightning',
        `<h1>Pay with Lightning</h1>
<p>Scan the QR code with a Lightning wallet, or pay the BOLT11 invoice below.
This page refreshes automatically and will confirm when payment is received.</p>
${qrSvg === '' ? '' : `<div class="qr">${qrSvg}</div>`}
<pre>${invoice}</pre>
<a class="button" href="lightning:${invoice}">Open in wallet</a>
<p class="status">Status: ${escapeHtml(status.status)} - waiting for payment.</p>`,
        { refresh: true },
      )
    })
  },
})
