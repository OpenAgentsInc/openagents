export const checkoutDemoScript = (_action: string): string =>
  'document.documentElement.dataset.moneySurfaceRetired = "true";'

export function SiteCheckoutDemoPage(_props: { returnAction?: string }) {
  return (
    <main
      className="mx-auto flex min-h-[70vh] max-w-2xl flex-col justify-center px-6 py-20 text-white"
      data-money-surface-retired
    >
      <p className="font-mono text-sm uppercase tracking-widest text-white/50">
        Retired
      </p>
      <h1 className="mt-3 text-4xl font-semibold tracking-tight">
        Sites checkout is no longer available
      </h1>
      <p className="mt-4 text-base/7 text-white/70">
        Sites, payments, credits, wallets, payouts, and settlement are outside
        the Codex Workroom MVP. Formerly paid capacity is disabled; it has not
        become free capacity.
      </p>
    </main>
  )
}
