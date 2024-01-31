import { createplugin_res, ndk } from "./index.js"
import { nostr_home_relay, pluginKind } from "./consts.js"
import { checkPluginEvent } from "./utils.js"
import { NDKEvent } from "@nostr-dev-kit/ndk"

/*
  * Returns formatted plugins
  */
  export async function getPlugins () {
    const specificrelay = nostr_home_relay
    if (specificrelay && specificrelay?.startsWith("https://api.nostr.wine/search")) {
      const res = await fetch(specificrelay)
      const formatted = [];
      const resjson = await res.json()
      resjson.data.forEach((item) => {
        const data = checkPluginEvent(item)
        console.log(data)
        if (data !== false) {
          formatted.push(data)}
      })
      return formatted
    }
    const events = await ndk.fetchEvents({kinds: [pluginKind]});
    const array = Array.from(events);
    const formatted = [];
    array.forEach((item) => {
      const data = checkPluginEvent(item)
      console.log(data)
      if (data !== false) {
        formatted.push(data);
      }
    })

    return formatted
  }

/*
  * Get specific plugin from user pubkey and d tag
  */
  export async function getPlugin (pubkey, slug) {
    const filter = { kinds: [pluginKind], authors: [pubkey], '#d': [`${slug}`]}
    const event = await ndk.fetchEvent(filter);
    if (!event) {
      return null
    } else {
      const data = checkPluginEvent(event);
      console.log('plugin>data', data)
      if (!data) {
        return null
      }
      return data
    }
  }

