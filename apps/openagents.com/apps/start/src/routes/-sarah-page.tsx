/**
 * Thin host for the zero-React Sarah shell (#8594).
 * The product UI itself lives in public/sarah (no React). This route only
 * frames that shell at openagents.com/sarah.
 */
export function SarahHostPage() {
  return (
    <main className="m-0 min-h-dvh bg-black p-0" data-route="sarah">
      <iframe
        title="Sarah — OpenAgents sales assistant"
        src="/sarah/index.html"
        className="block h-dvh w-full border-0"
        allow="microphone"
      />
    </main>
  )
}
