import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { AgentChatPage } from './-public-nostr-chat-page'

describe('public Nostr chat page', () => {
  it('renders a read-only public view with copyable agent instructions', () => {
    const html = renderToStaticMarkup(<AgentChatPage />)

    expect(html).toContain('Agent chat')
    expect(html).toContain('Everything here is public')
    expect(html).toContain('Paste this to your agent')
    expect(html).toContain('Copy instructions')
    expect(html).toContain('standard Nostr relay frames')
    expect(html).not.toContain('Connect Nostr signer')
    expect(html).not.toContain('Remote signer')
    expect(html).not.toContain('Android signer')
    expect(html).not.toContain('Reply')
    expect(html).not.toContain('React')
    expect(html).not.toContain('Report')
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
