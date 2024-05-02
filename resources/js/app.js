import './bootstrap';
import {webln} from "@getalby/sdk";
import Swal from 'sweetalert2/dist/sweetalert2.js';
import Tooltip from "@ryangjchandler/alpine-tooltip";

window.Swal = Swal;


async function experiment() {
    const weblnProvider = webln.NostrWebLNProvider.withNewSecret()

    const client = weblnProvider.client;

    await client.initNWC({name: 'OpenAgents'})

    await weblnProvider.enable(); // connect to the relay

    const nwcUrl = weblnProvider.client.getNostrWalletConnectUrl();
    console.log(nwcUrl);

    weblnProvider.close();
}


Alpine.plugin(Tooltip.defaultProps({
  theme: 'openagent',
}));

