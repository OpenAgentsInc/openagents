import './bootstrap';
import {webln} from "@getalby/sdk";
import Swal from 'sweetalert2/dist/sweetalert2.js';
import Tooltip from "@ryangjchandler/alpine-tooltip";
import * as FilePond from 'filepond';
import FilePondPluginImagePreview from 'filepond-plugin-image-preview';
import FilePondPluginFileValidateSize from 'filepond-plugin-file-validate-size';
import FilePondPluginFileValidateType from 'filepond-plugin-file-validate-type';


window.Swal = Swal;

window.FilePond = FilePond;

// Register the plugin
FilePond.registerPlugin(FilePondPluginImagePreview);
FilePond.registerPlugin(FilePondPluginFileValidateType);
FilePond.registerPlugin(FilePondPluginFileValidateSize);



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

