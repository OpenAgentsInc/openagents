// Browser harness for the Kiki desk test: bundled by the test and loaded
// into a page with the built ui.css and a copy of the release's controls.
import { mountKiki } from "../../src/kiki/desk-host";

declare global {
  interface Window {
    kiki: { mount: () => void; dispose?: () => void };
  }
}

window.kiki = {
  mount() {
    window.kiki.dispose = mountKiki();
  },
};
