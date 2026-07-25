import {
  NostrEvent,
  PUBLIC_CHAT_SIGNER_KINDS,
  type PublicChatSigner,
} from '@openagentsinc/public-nostr-chat'
import { Schema as S } from 'effect'
import { verifyEvent } from 'nostr-effect/pure'

const storageKey = 'openagents.public_chat.nip55.v1'

type Nip55Identity = Readonly<{ packageName?: string; pubkey: string }>

const request = async (input: Readonly<{
  currentUser?: string
  packageName?: string
  payload?: string
  type: 'get_public_key' | 'sign_event'
}>): Promise<URLSearchParams> => {
  const requestId = crypto.randomUUID()
  const callback = new URL('/agentchat/signer-callback', window.location.origin)
  callback.searchParams.set('requestId', requestId)
  const query = new URLSearchParams({
    callbackUrl: `${callback.toString()}&${
      input.type === 'sign_event' ? 'event' : 'result'
    }=`,
    compressionType: 'none',
    returnType: 'event',
    type: input.type,
  })
  if (input.currentUser !== undefined) query.set('current_user', input.currentUser)
  if (input.packageName !== undefined) query.set('package', input.packageName)
  if (input.type === 'get_public_key') {
    query.set(
      'permissions',
      JSON.stringify(
        PUBLIC_CHAT_SIGNER_KINDS.filter(
          kind => ![9002, 9005, 9010].includes(kind),
        ).map(kind => ({ kind, type: 'sign_event' })),
      ),
    )
  }
  const uri = `nostrsigner:${encodeURIComponent(input.payload ?? '')}?${query}`

  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => {
      window.removeEventListener('message', onMessage)
      reject(new Error('nip55-timeout'))
    }, 60_000)
    const onMessage = (message: MessageEvent) => {
      if (message.origin !== window.location.origin) return
      if (
        typeof message.data !== 'object' ||
        message.data === null ||
        message.data.type !== 'openagents-nip55-result' ||
        message.data.requestId !== requestId
      ) {
        return
      }
      window.clearTimeout(timer)
      window.removeEventListener('message', onMessage)
      const params = new URLSearchParams(String(message.data.query))
      if (params.get('rejected') === 'true') {
        reject(new Error('nip55-refused'))
        return
      }
      resolve(params)
    }
    window.addEventListener('message', onMessage)
    const popup = window.open(uri, `nip55-${requestId}`)
    if (popup === null) {
      window.clearTimeout(timer)
      window.removeEventListener('message', onMessage)
      reject(new Error('nip55-popup-blocked'))
    }
  })
}

export const makeNip55WebSigner = (
  identity: Nip55Identity,
): PublicChatSigner => ({
  getPublicKey: async () => identity.pubkey,
  signEvent: async template => {
    const response = await request({
      currentUser: identity.pubkey,
      ...(identity.packageName === undefined
        ? {}
        : { packageName: identity.packageName }),
      payload: JSON.stringify(template),
      type: 'sign_event',
    })
    const raw = response.get('event')
    if (raw === null) throw new Error('nip55-invalid-result')
    const event = S.decodeUnknownSync(NostrEvent)(JSON.parse(raw))
    if (
      event.pubkey !== identity.pubkey ||
      event.kind !== template.kind ||
      event.content !== template.content ||
      event.created_at !== template.created_at ||
      JSON.stringify(event.tags) !== JSON.stringify(template.tags) ||
      !verifyEvent({ ...event, tags: event.tags.map(tag => [...tag]) })
    ) {
      throw new Error('nip55-invalid-signature')
    }
    return event
  },
})

export const connectNip55WebSigner = async (): Promise<PublicChatSigner> => {
  const response = await request({ type: 'get_public_key' })
  const pubkey = response.get('result')
  if (pubkey === null || !/^[0-9a-f]{64}$/.test(pubkey)) {
    throw new Error('nip55-invalid-pubkey')
  }
  const identity: Nip55Identity = {
    ...(response.get('package') === null
      ? {}
      : { packageName: response.get('package')! }),
    pubkey,
  }
  sessionStorage.setItem(storageKey, JSON.stringify(identity))
  return makeNip55WebSigner(identity)
}

export const restoreNip55WebSigner = (): PublicChatSigner | undefined => {
  try {
    const value: unknown = JSON.parse(sessionStorage.getItem(storageKey) ?? 'null')
    if (
      typeof value !== 'object' ||
      value === null ||
      !('pubkey' in value) ||
      typeof value.pubkey !== 'string' ||
      !/^[0-9a-f]{64}$/.test(value.pubkey)
    ) {
      return undefined
    }
    return makeNip55WebSigner({
      ...('packageName' in value && typeof value.packageName === 'string'
        ? { packageName: value.packageName }
        : {}),
      pubkey: value.pubkey,
    })
  } catch {
    return undefined
  }
}
