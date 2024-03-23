import './bootstrap';
import {webln} from "@getalby/sdk";

async function experiment() {
    const weblnProvider = webln.NostrWebLNProvider.withNewSecret()
    // const weblnProvider = new webln.NostrWebLNProvider(); // use defaults (connects to Alby's relay, will use window.nostr to sign the request)

    const client = weblnProvider.client;

    await client.initNWC({name: 'OpenAgents'})

    // try {
    //     await weblnProvider.initNWC({name: 'ACME app' );
    // } catch(e) {
    //         console.warn("Prompt closed");
    //     }

    await weblnProvider.enable(); // connect to the relay

    const nwcUrl = weblnProvider.client.getNostrWalletConnectUrl();
    console.log(nwcUrl);

    // const response = await weblnProvider.sendPayment(invoice);
    // console.log(response.preimage);

    // const nwcUrl = weblnProvider.client.getNostrWalletConnectUrl(true);
    // console.log(nwcUrl);

    weblnProvider.close();
}

experiment()

//
// async function requestUserConsent() {
//     // Check if WebLN is supported (e.g., Alby extension is installed)
//     if (window.webln) {
//         try {
//             // Request user consent through the Alby extension
//             await window.webln.enable();
//             console.log("User consent received.");
//
//             // After obtaining consent, you can proceed with operations like getBalance, sendPayment, etc.
//
//             // Check user's lightning address?
//             console.log(window.webln)
//
//             generateNWCUrl()
//         } catch (error) {
//             console.error("User consent denied or an error occurred:", error);
//         }
//     } else {
//         console.log("WebLN (e.g., Alby extension) is not detected.");
//     }
// }
//
// async function generateNWCUrl() {
//     const nwc = webln.NostrWebLNProvider.withNewSecret();
//     const nwcUrl = weblnProvider.client.getNostrWalletConnectUrl();
//     console.log(nwcUrl);
//
//
//     await weblnProvider.client.initNWC();
// }
//
// requestUserConsent();
