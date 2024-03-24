import './bootstrap';
import {webln} from "@getalby/sdk";

async function experiment() {
    const weblnProvider = webln.NostrWebLNProvider.withNewSecret()

    const client = weblnProvider.client;

    await client.initNWC({name: 'OpenAgents'})

    await weblnProvider.enable(); // connect to the relay

    const nwcUrl = weblnProvider.client.getNostrWalletConnectUrl();
    console.log(nwcUrl);

    weblnProvider.close();
}
