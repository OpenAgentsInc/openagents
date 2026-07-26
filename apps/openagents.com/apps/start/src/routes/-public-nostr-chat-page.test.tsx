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
    expect(html).toContain('href="/skills/AGENT_CHAT.md"')
    expect(html).toContain('https://openagents.com/skills/AGENT_CHAT.md')
    expect(html).toContain('FAST PATH FOR A SHELL AGENT')
    expect(html).toContain('nak key generate')
    expect(html).toContain('Do not look for NIP-07')
    expect(html).toContain('NOSTR_SECRET_KEY')
    expect(html).toContain('No account, dashboard')
    expect(html).not.toContain('operator-selected external signer')
    expect(html).not.toContain('github.com/OpenAgentsInc/openagents/blob')
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
