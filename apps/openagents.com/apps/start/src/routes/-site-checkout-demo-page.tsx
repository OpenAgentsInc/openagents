import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

type SiteCheckoutDemoAction = 'cancel' | 'start' | 'status' | 'success'

const panelClass = 'border border-khala-border bg-khala-surface'
const eyebrowClass =
  'font-mono text-sm uppercase tracking-wide text-khala-text-faint'
const mutedClass = 'text-base/7 text-khala-text-muted sm:text-sm/6'
const primaryButtonClass =
  'khala-focus inline-flex min-h-10 items-center justify-center border border-khala-text bg-khala-text px-4 py-2 font-mono text-base font-medium text-black hover:bg-white disabled:cursor-not-allowed disabled:opacity-45 sm:min-h-8 sm:text-sm'
const secondaryButtonClass =
  'khala-focus inline-flex min-h-10 items-center justify-center border border-khala-border bg-transparent px-4 py-2 font-mono text-base text-khala-text-muted hover:bg-white/5 hover:text-khala-text sm:min-h-8 sm:text-sm'

const normalizeCheckoutAction = (
  action: string | undefined,
): SiteCheckoutDemoAction =>
  action === 'cancel' || action === 'status' || action === 'success'
    ? action
    : 'start'

export const checkoutDemoScript = (
  action: SiteCheckoutDemoAction,
): string => `(() => {
  const initial = ${JSON.stringify({ action })};
  const siteId = 'site_omega_mdk_demo';
  const discoveryEndpoint = '/api/sites/' + encodeURIComponent(siteId) + '/commerce/discovery';
  const intentStorageKey = 'openagents.demoCheckout.intentRef';
  const idempotencyStorageKey = 'openagents.demoCheckout.idempotencyKey';
  const main = document.querySelector('[data-site-checkout-main]');
  if (!main) return;

  const escapeHtml = value => String(value ?? '').replace(/[&<>"']/g, char => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  })[char]);
  const readStorage = key => {
    try { return sessionStorage.getItem(key); } catch (_) { return null; }
  };
  const writeStorage = (key, value) => {
    try { sessionStorage.setItem(key, value); } catch (_) {}
  };
  const randomSegment = () => {
    const bytes = new Uint8Array(12);
    if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
      globalThis.crypto.getRandomValues(bytes);
      return Array.from(bytes, byte => byte.toString(16).padStart(2, '0')).join('');
    }
    throw new Error('Secure browser randomness is required to start checkout.');
  };
  const idempotencyKey = () => {
    const existing = readStorage(idempotencyStorageKey);
    if (existing) return existing;
    const next = 'omega-demo-checkout-' + randomSegment();
    writeStorage(idempotencyStorageKey, next);
    return next;
  };
  const fetchJson = async (path, options = {}) => {
    const response = await fetch(path, {
      ...options,
      headers: {
        ...(options.body ? { 'Content-Type': 'application/json' } : {}),
        ...(options.headers || {}),
      },
    });
    const payload = await response.json().catch(() => ({}));
    if (!response.ok) {
      const error = new Error(payload.message || payload.reason || payload.error || 'Request failed');
      error.payload = payload;
      error.status = response.status;
      throw error;
    }
    return payload;
  };
  const priceText = price => {
    if (!price) return 'Price pending';
    if (price.asset === 'usd' && price.denomination === 'usd_cent') {
      return '$' + (Number(price.amountMinorUnits || 0) / 100).toFixed(2);
    }
    if (price.asset === 'bitcoin' && price.denomination === 'bitcoin_millisatoshi') {
      return (Number(price.amountMinorUnits || 0) / 100000000000).toFixed(8) + ' bitcoin';
    }
    if (price.asset === 'sats') return String(price.amount || price.amountMinorUnits || 0) + ' satoshis of bitcoin';
    return String(price.amountMinorUnits || price.amount || 0) + ' credits';
  };
  const productLabel = item => {
    if (!item) return 'Demo product';
    if (item.displayRef === 'display.omega_demo_checkout') return 'Omega demo checkout';
    if (item.productId) return String(item.productId).replaceAll('_', ' ');
    return 'Demo product';
  };
  const titleForState = state => {
    switch (state) {
      case 'cancel': return 'Checkout cancelled';
      case 'entitled': return 'Checkout complete';
      case 'expired': return 'Checkout expired';
      case 'paid': return 'Payment received';
      case 'pending': return 'Payment pending';
      case 'success': return 'Checkout returned';
      case 'unpaid': return 'Payment not received';
      default: return 'Checkout status';
    }
  };
  const statusText = state => {
    switch (state) {
      case 'cancel': return 'No payment was recorded for this checkout.';
      case 'entitled': return 'The payment is recorded and the demo entitlement is active.';
      case 'expired': return 'This checkout is no longer active. Start a fresh checkout when you want to test again.';
      case 'paid': return 'The payment is recorded. Entitlement projection may still be reconciling.';
      case 'pending': return 'The checkout is still waiting for payment confirmation.';
      case 'success': return 'The checkout returned successfully. Payment confirmation may still be pending.';
      case 'unpaid': return 'No payment has been recorded yet.';
      default: return 'OpenAgents could not read a safe checkout status for this browser session.';
    }
  };
  const shell = inner => '<section class="${panelClass} p-4 sm:p-5">' + inner + '</section>';
  const renderUnavailable = (title, message) => {
    main.innerHTML = shell(
      '<p class="${eyebrowClass}">' + escapeHtml(title) + '</p>' +
      '<p class="mt-3 ${mutedClass}">' + escapeHtml(message) + '</p>' +
      '<div class="mt-5"><button type="button" class="${secondaryButtonClass}" data-action="retry">Retry</button></div>'
    );
    const retry = main.querySelector('[data-action="retry"]');
    if (retry) retry.addEventListener('click', loadDiscovery);
  };
  const renderProduct = discovery => {
    const items = discovery.items || [];
    const item = items.find(candidate => candidate.itemKind === 'product' && candidate.status === 'active') || items[0];
    if (!item) {
      renderUnavailable('Checkout unavailable', 'No active demo checkout product is listed for this Site.');
      return;
    }
    const storedIntent = readStorage(intentStorageKey);
    main.innerHTML = shell(
      '<div class="grid gap-4 sm:grid-cols-[1fr_auto] sm:items-start">' +
      '<div><p class="${eyebrowClass}">Demo product</p>' +
      '<h2 class="m-0 mt-2 text-2xl font-medium tracking-normal text-white sm:text-3xl">' + escapeHtml(productLabel(item)) + '</h2>' +
      '<p class="mt-2 ${mutedClass}">Start a hosted checkout session for the demo Site product.</p></div>' +
      '<div class="border border-khala-border px-3 py-2 text-right font-mono text-base text-white sm:text-sm">' + escapeHtml(priceText(item.price)) + '</div>' +
      '</div>' +
      '<div class="mt-5 flex flex-wrap items-center gap-3">' +
      '<button type="button" class="${primaryButtonClass}" data-action="start-checkout">Start checkout</button>' +
      (storedIntent ? '<a class="${secondaryButtonClass}" href="/sites/demo-checkout/status">Check status</a>' : '') +
      '</div>' +
      '<p class="mt-4 text-base/7 text-khala-text-faint sm:text-sm/6">Use this to test the demo checkout path before attaching it to a generated Site.</p>'
    );
    const start = main.querySelector('[data-action="start-checkout"]');
    if (start) start.addEventListener('click', () => startCheckout(discovery, item, start));
  };
  const renderLaunch = (intent, implementationState) => {
    const launchPath = intent.checkoutLaunchPath || (intent.hostedCheckout && intent.hostedCheckout.checkoutLaunchPath);
    const isLive = implementationState === 'live_provider_configured';
    main.innerHTML = shell(
      '<p class="${eyebrowClass}">' + (isLive ? 'Hosted checkout ready' : 'Demo checkout created') + '</p>' +
      '<h2 class="m-0 mt-2 text-2xl font-medium tracking-normal text-white sm:text-3xl">' + (isLive ? 'Continue to payment' : 'Checkout provider not live here') + '</h2>' +
      '<p class="mt-3 ${mutedClass}">' + (isLive ? 'Open the hosted checkout to complete the demo purchase.' : 'The intent was created, but this environment is not connected to a live hosted payment page.') + '</p>' +
      '<div class="mt-5 flex flex-wrap gap-3">' +
      (launchPath ? '<a class="${primaryButtonClass}" href="' + escapeHtml(launchPath) + '">Open hosted checkout</a>' : '') +
      '<a class="${secondaryButtonClass}" href="/sites/demo-checkout/status">Check status</a>' +
      '<a class="${secondaryButtonClass}" href="/sites/demo-checkout">Back</a>' +
      '</div>'
    );
    if (isLive && launchPath) window.setTimeout(() => window.location.assign(launchPath), 450);
  };
  const renderReturn = projection => {
    const state = projection.returnState || 'blocked';
    main.innerHTML = shell(
      '<p class="${eyebrowClass}">Checkout return</p>' +
      '<h2 class="m-0 mt-2 text-2xl font-medium tracking-normal text-white sm:text-3xl">' + escapeHtml(titleForState(state)) + '</h2>' +
      '<p class="mt-3 ${mutedClass}">' + escapeHtml(statusText(state)) + '</p>' +
      '<div class="mt-5 flex flex-wrap gap-3">' +
      '<a class="${secondaryButtonClass}" href="/sites/demo-checkout/status">Refresh status</a>' +
      '<a class="${secondaryButtonClass}" href="/sites/demo-checkout">Back to demo checkout</a>' +
      '</div>'
    );
  };
  const loadDiscovery = async () => {
    main.innerHTML = shell('<p class="${eyebrowClass}">Loading</p><p class="mt-3 ${mutedClass}">Reading the Site commerce contract.</p>');
    try {
      const payload = await fetchJson(discoveryEndpoint, { method: 'GET' });
      renderProduct(payload.siteCommerce && payload.siteCommerce.discovery ? payload.siteCommerce.discovery : {});
    } catch (error) {
      renderUnavailable('Checkout unavailable', error.message || 'The demo checkout contract could not be loaded.');
    }
  };
  const startCheckout = async (discovery, item, button) => {
    const siteVersionId = (discovery.siteVersionIds || [])[0];
    if (!siteVersionId) {
      renderUnavailable('Checkout unavailable', 'The demo checkout product is missing a public Site version.');
      return;
    }
    button.setAttribute('disabled', 'disabled');
    button.textContent = 'Creating checkout';
    try {
      const payload = await fetchJson(item.checkoutIntentEndpoint || discovery.endpoints.checkoutIntent, {
        body: JSON.stringify({
          cancelReturnPath: '/sites/demo-checkout/cancel',
          catalogRef: item.catalogRef,
          customerDataRefs: item.customerDataRequirementRefs || [],
          expectedPrice: item.price,
          itemKind: item.itemKind,
          productId: item.productId,
          siteVersionId,
          successReturnPath: '/sites/demo-checkout/success',
        }),
        headers: { 'Idempotency-Key': idempotencyKey() },
        method: 'POST',
      });
      const commerce = payload.siteCommerce || {};
      const intent = commerce.checkoutIntent || {};
      const intentRef = intent.id || intent.checkoutIntentRef || commerce.checkoutIntentRef;
      if (intentRef) writeStorage(intentStorageKey, intentRef);
      renderLaunch(intent, commerce.implementationState || 'missing_configuration');
    } catch (error) {
      const reason = error.payload && error.payload.reason ? error.payload.reason : '';
      if (error.status === 503 || reason === 'missing_configuration') {
        renderUnavailable('Checkout not live', 'The demo product is ready, but the hosted payment provider is not configured on this environment.');
        return;
      }
      renderUnavailable('Checkout failed', error.message || 'OpenAgents could not create a safe checkout intent.');
    }
  };
  const loadReturn = async action => {
    const normalizedAction = action === 'cancel' || action === 'success' || action === 'status' ? action : 'status';
    const intentRef = readStorage(intentStorageKey);
    if (!intentRef) {
      renderUnavailable('No checkout session', 'This browser does not have a public checkout intent to inspect. Start a checkout first.');
      return;
    }
    main.innerHTML = shell('<p class="${eyebrowClass}">Loading</p><p class="mt-3 ${mutedClass}">Reading the clean checkout status.</p>');
    try {
      const path = '/api/sites/' + encodeURIComponent(siteId) + '/commerce/checkout-returns/' + encodeURIComponent(intentRef) + '/' + normalizedAction;
      const payload = await fetchJson(path, { method: 'GET' });
      const projection = payload.siteCommerce && payload.siteCommerce.returnProjection ? payload.siteCommerce.returnProjection : {};
      renderReturn(projection);
    } catch (error) {
      renderUnavailable('Checkout status unavailable', error.message || 'OpenAgents could not read this checkout status.');
    }
  };
  if (initial.action === 'success' || initial.action === 'cancel' || initial.action === 'status') {
    loadReturn(initial.action);
  } else {
    loadDiscovery();
  }
})();`

export function SiteCheckoutDemoPage({
  returnAction,
}: Readonly<{ returnAction?: string }>) {
  const action = normalizeCheckoutAction(returnAction)

  return (
    <main
      className="min-h-dvh bg-black text-khala-text"
      data-route="site-checkout-demo"
      data-site-checkout-demo=""
    >
      <div className="mx-auto grid w-full max-w-5xl gap-5 px-4 py-6 font-mono">
        <Card className="p-4 sm:p-5">
          <p className={eyebrowClass}>Autopilot Sites</p>
          <h1 className="m-0 mt-2 text-3xl font-medium tracking-normal text-white sm:text-4xl">
            Demo checkout
          </h1>
          <p className={`m-0 mt-3 max-w-2xl ${mutedClass}`}>
            Start a demo checkout for an Omega Site product and inspect the
            clean return status.
          </p>
        </Card>
        <div data-site-checkout-main="">
          <Card className="p-4 sm:p-5">
            <p className={eyebrowClass}>Loading</p>
            <p className={`mt-3 ${mutedClass}`}>
              Reading the Site commerce contract.
            </p>
            <Button className="mt-5" disabled type="button">
              Start checkout
            </Button>
          </Card>
        </div>
      </div>
      <script
        dangerouslySetInnerHTML={{ __html: checkoutDemoScript(action) }}
      />
    </main>
  )
}
