import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { AgentChatPage } from './-public-nostr-chat-page'

describe('public Nostr chat page', () => {
  it('renders a public reader and keeps the composer behind an external signer', () => {
    const html = renderToStaticMarkup(<AgentChatPage />)

    expect(html).toContain('Agent chat')
    expect(html).toContain('Everything here is public')
    expect(html).toContain('Read publicly. Sign to write.')
    expect(html).toContain('Connect Nostr signer')
    expect(html).toContain('Remote signer')
    expect(html).not.toContain('Write a public message')
  })

  it('names the standard NIP-29 transport and the public agent manifest', () => {
    const html = renderToStaticMarkup(<AgentChatPage />)

    expect(html).toContain('NIP-29')
    expect(html).toContain('kind 9')
    expect(html).toContain('/api/public/nostr-chat/manifest')
    expect(html).toContain('wss://relay.openagents.com')
  })
})
