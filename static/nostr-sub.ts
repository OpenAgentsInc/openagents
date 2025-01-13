import * as nip19 from "nostr-tools/nip19";
import { NDKSubscription, NDKEvent } from "@nostr-dev-kit/ndk";

type HTMXInternalAPI = {
  getInternalData(elt: HTMLElement): HTMXElementData;
  getAttributeValue(elt: Element, attr: string): string | undefined;
  withExtensions(elt: HTMLElement, fn: (ext: any) => void): void;
  makeSettleInfo(elt: HTMLElement): SettleInfo;
  makeFragment(s: string): HTMLElement;
  oobSwap(arg: string, elt: Element, si: SettleInfo): void;
  settleImmediately(tasks: any[]): void;
};

type SettleInfo = {
  tasks: any[];
};

type HTMXElementData = {
  sub: NDKSubscription;
};

declare global {
  interface Window {
    htmx: any;
    ndk: any;
  }
}

let element: HTMLElement;
let api: HTMXInternalAPI;
const htmx: any = window.htmx;

htmx.defineExtension("nostr-sub", {
  init(apiRef: HTMXInternalAPI) {
    api = apiRef;
  },
  onEvent(name: string, evt: CustomEvent) {
    switch (name) {
      case "htmx:trigger": {
        subscribe(evt.target as HTMLElement);
        break;
      }
      case "htmx:beforeCleanupElement": {
        console.log("canceling subscription");
        const data = api.getInternalData(element);
        data.sub.stop();
        break;
      }
    }
  },
});

function subscribe(element: HTMLElement) {
  const data = api.getInternalData(element);
  let filter: any = {};
  try {
    filter = JSON.parse(api.getAttributeValue(element, "nostr-filter") || "{}");
  } catch (err) {
    filter = {};
  }

  if (element instanceof HTMLFormElement) {
    ["since", "until"].forEach((f) => {
      if (element[f]) {
        filter[f] = parseInt(element[f].value, 10);
      }
    });
    ["author", "kind", "id"].forEach((f) => {
      if (element[f]) {
        const fs = f + "s";
        const current = filter[fs] || [];
        const fieldValue = processValue(element[f].value);
        if (fieldValue) {
          current.push(fieldValue);
          filter[fs] = current;
        }
      }
    });
    ["authors", "kinds", "ids"].forEach((fs) => {
      if (element[fs]) {
        const current = filter[fs] || [];
        const fieldValues = element[fs].value
          .split(",")
          .map(processValue)
          .filter((v: any) => v);
        if (fieldValues.length) {
          current.push(...fieldValues);
          filter[fs] = current;
        }
      }
    });
  }

  function processValue(v: string | null): string | null {
    if (!v) return null;
    v = v.trim();
    try {
      const { type, data } = nip19.decode(v);
      switch (type) {
        case "npub":
          v = data as string;
          break;
        case "nprofile":
          v = (data as nip19.ProfilePointer).pubkey;
          break;
        case "nevent":
          v = (data as nip19.EventPointer).id;
          break;
        case "note":
          v = data as string;
          break;
        default:
          v = null;
          break;
      }
    } catch (err) {
      /* -- */
    }
    if (v && v.match(/^[a-f0-9]{64}$/)) return v;
    return null;
  }

  console.log("triggering subscription", filter);
  data.sub = window.ndk.subscribe(filter, { closeOnEose: false });
  data.sub.start();
  data.sub.on(
    "event",
    (response: NDKEvent) => {
      response.ndk = window.ndk;
      let raw: any = response.rawEvent();
      raw.author = response.author;
      let html = JSON.stringify(raw);

      api.withExtensions(element, function (extension) {
        html = extension.transformResponse(html, null, element);
      });

      const settleInfo = api.makeSettleInfo(element);
      const fragment = api.makeFragment(html);
      if (fragment.children.length) {
        const children = Array.from(fragment.children);
        for (let i = 0; i < children.length; i++) {
          api.oobSwap(
            api.getAttributeValue(children[i], "hx-swap-oob") || "true",
            children[i],
            settleInfo,
          );
        }
      }
      api.settleImmediately(settleInfo.tasks);
    },
    5000,
  );
}
