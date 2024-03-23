import './bootstrap';

import {webln} from '@getalby/sdk';

console.log('test')

async function generateNWCUrl() {
    const weblnProvider = webln.NostrWebLNProvider.withNewSecret();
    const nwcUrl = await weblnProvider.getNostrWalletConnectUrl();
    console.log(nwcUrl);
}

generateNWCUrl()