// Some relays
export const defaultRelays = ["wss://nostr-pub.wellorder.net","wss://nos.lol","wss://relay.damus.io","wss://relay.snort.social","wss://nostr.wine"]

// Kind used for plugins
export const pluginKind = 30514

// Try to get relays from localStorage, if they don't exist, use the defaultRelays
export const relays = JSON.parse(localStorage.getItem('nostr_relays')) || defaultRelays

// Which relay is used as "home" relay
export const nostr_home_relay = localStorage.getItem('nostr_home_relay')
