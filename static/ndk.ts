import NDK, {NDKNip07Signer} from '@nostr-dev-kit/ndk'

// const nip07signer = new NDKNip07Signer()

const ndk =
  // @ts-ignore
  window.ndk ||
  new NDK({
    explicitRelayUrls: [
      'wss://public.relaying.io',
      'wss://nostr-pub.wellorder.net',
      'wss://nostr.mom',
      'wss://relay.nostr.band'
    ]
    // signer: nip07signer
  })

// @ts-ignore
window.ndk = ndk

ndk.connect()

export default ndk
