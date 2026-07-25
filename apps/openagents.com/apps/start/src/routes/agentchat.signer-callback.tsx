import { createFileRoute } from '@tanstack/react-router'
import { useEffect } from 'react'

function Nip55CallbackPage() {
  useEffect(() => {
    const url = new URL(window.location.href)
    window.opener?.postMessage(
      {
        query: url.searchParams.toString(),
        requestId: url.searchParams.get('requestId'),
        type: 'openagents-nip55-result',
      },
      window.location.origin,
    )
    window.close()
  }, [])

  return (
    <main className="grid min-h-screen place-items-center bg-black p-6 text-white">
      <p className="font-mono text-sm">Returning the signer result…</p>
    </main>
  )
}

export const Route = createFileRoute('/agentchat/signer-callback')({
  component: Nip55CallbackPage,
})
