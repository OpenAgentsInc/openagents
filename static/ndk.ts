import NDK, { NDKNip07Signer } from "@nostr-dev-kit/ndk"

const nip07signer = new NDKNip07Signer()

const ndk = new NDK({
  explicitRelayUrls: [
    'wss://nostr-pub.wellorder.net',
    'wss://nostr.mom',
    'wss://relay.nostr.band'
  ],
  signer: nip07signer
})

// Make NDK instance available globally
declare global {
  interface Window {
    ndk: NDK
  }
}
window.ndk = ndk

ndk.connect()

// No export needed in IIFE format