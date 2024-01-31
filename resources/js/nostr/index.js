import NDK, { NDKEvent } from "@nostr-dev-kit/ndk";
import NDKCacheAdapterDexie from "@nostr-dev-kit/ndk-cache-dexie";
import Alpine from "alpinejs";
import { getPlugin, getPlugins } from "./nostr.js";
import { checkPluginEvent, formatSats } from "./utils.js";
import { relays, nostr_home_relay, pluginKind } from "./consts.js";

const dexieAdapter = new NDKCacheAdapterDexie({ dbName: 'ndk-cache' });
export const createplugin_res = Alpine.reactive({message: ''})

window.formatSats = formatSats;
window.plugins = getPlugins;
window.plugin = getPlugin;

export const ndk = new NDK({
  explicitRelayUrls: relays,
  cacheAdapter: dexieAdapter,
});

/*
  * Alpine form for creating a new plugin and publishing it to the nostr network
  */
  Alpine.data('createplugin', () => ({
    title: '',
    description: '',
    wasm_url: '',
    fee: '',
    async submitForm() {
      createplugin_res.message = 'loading...'
      const data = {
        title: this.title,
        description: this.description,
        wasm_url: this.wasm_url,
        fee: Number(this.fee),
      };

      const event = new NDKEvent(ndk);
      event.kind = pluginKind;
      event.content = data.wasm_url;
      event.tags.push(['d', data.title.toLowerCase().replaceAll(" ", "-")]);
      event.tags.push(['title', data.title]);
      event.tags.push(['summary', data.description]);
      event.tags.push(['published_at', String(Date.now())]);
      if (data.fee > 0) {
        event.tags.push(['price', String(data.fee/100000000), "btc"]);
      }

      const res = checkPluginEvent(event)
      if (res === false) {
        createplugin_res.message = 'please fill in all fields correctly'
        return
      }

      console.log('created event', event)

      try {

        if (!ndk.signer) {
          ndk.signer = new NDKNip07Signer()
        }

        createplugin_res.message = 'publishing...';
        await event.publish();

        createplugin_res.message = '<br/>success!';
        window.location.href = `/nostr/plugin/${event.pubkey}/${data.title.toLowerCase().replaceAll(" ", "-")}`
        console.log('published event', event)

      } catch (err) {
        createplugin_res.message = `error: ${err.message}`
      }
    }
  }
  ));

/*
  * Alpine form for searching plugins
  */
  Alpine.data('searchplugins', () => ({
    query: '',
    async submitForm() {
      const data = {
        query: this.query,
      };
      if (data.query == '') {
        localStorage.removeItem('nostr_home_relay');
        window.location.href = ""
      } else {
        localStorage.setItem('nostr_home_relay', `https://api.nostr.wine/search?query=${data.query}&kind=${pluginKind}`)
        console.log(data)
        window.location.href = ""
      }
    }
  }))

/*
  * Alpine form for setting new relays
  */
  Alpine.data('setrelays', () => ({
    newrelays: '',
    async submitForm() {
      const data = {
        newrelays: this.newrelays,
      };
      if (data.newrelays !== '') {
        localStorage.setItem('nostr_relays', JSON.stringify(data.newrelays.split(',')));
      } else {
        localStorage.removeItem('nostr_relays')
      }
    }
  }))


// document connect mess
Alpine.data('createplugin_res', () => {return createplugin_res});
if (nostr_home_relay && document.getElementById("changepluginssource_select") && nostr_home_relay?.startsWith("https://api.nostr.wine/search")) {
  document.getElementById("changepluginssource_select").value = "wss://nostr.wine"
}
if (nostr_home_relay && nostr_home_relay?.startsWith("https://api.nostr.wine/search?query")) {
  localStorage.removeItem('nostr_home_relay')
}
document.getElementById("changepluginssource_select")?.addEventListener("change", function() {
  const data = {
    specificrelay: document.getElementById("changepluginssource_select").value
  };
  console.log('change to', data.specificrelay)

  if (data.specificrelay === ":settings") {
    window.location.href = "/nostr/settings"
    //document.getElementById("changepluginssource_select").value = "*"
  } else if (data.specificrelay === "wss://nostr.wine") {
    localStorage.setItem('nostr_home_relay', "https://api.nostr.wine/search?kind=30514");
    window.location.href = ""
  } else {
    localStorage.removeItem('nostr_home_relay');
    window.location.href = ""
  }
});
setTimeout(() => {
  if (document.getElementById("newrelays")) {
    document.getElementById("newrelays").value = ndk.explicitRelayUrls.join(',');
  }
}, 100)

await ndk.connect();

