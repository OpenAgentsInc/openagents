// Firm reason: 1 (browser glue: makes the sandboxed frame a realm runs in and carries text to and from it)
// A realm that runs someone else's code is shown in a frame on the realm origin, never on the
// site's (GitHub #13432; docs/metaverse/realm-origin.md). The frame's sandbox gives the realm no
// origin: it has no cookies, no storage and no way to read this page, and this page cannot read
// it. The two speak only by window.postMessage, and only the messages of
// metaverse-engine/host/realm-frame.coax (realm-frame.v1). This file decides nothing: it makes the
// frame, hands every text the frame posts to `onText`, and posts the texts it is given. The page's
// module writes and reads them with the codec.
//
//   const realm = openRealmFrame(holder, "https://REALM-ORIGIN", "/worlds/splice", (text) => ...);
//   realm.send("realm-frame.v1 hello");
//   realm.close();
const TEXT_MAX = 33792;   // a message at most: realm-frame.coax RF_BODY and its line

export function openRealmFrame(holder, realmOrigin, path, onText) {
  const origin = new URL(realmOrigin).origin, to = new URL(path, origin);
  // The whole point: another origin than this page's, and the realm's own.
  if (to.origin !== origin || origin === location.origin) throw new Error("a realm's frame is on the realm origin, never the site's");
  to.searchParams.set("frame", location.origin);
  const frame = document.createElement("iframe");
  // No allow-same-origin, ever: with it the realm would have the realm origin's storage and could lift the sandbox.
  frame.setAttribute("sandbox", "allow-scripts allow-pointer-lock allow-orientation-lock");
  frame.setAttribute("credentialless", "");   // where the browser has it: no cookie of any site goes with the frame's requests
  frame.allow = "cross-origin-isolated; fullscreen; gamepad; autoplay";
  frame.referrerPolicy = "no-referrer";
  frame.title = to.pathname;
  frame.dataset.realmFrame = "1";
  frame.style.cssText = "display:block;width:100%;height:100%;border:0;margin:0;padding:0;background:#000";
  const heard = (e) => {
    // A sandboxed frame's origin is "null", so the frame is known by its window, not by an origin.
    if (e.source !== frame.contentWindow || typeof e.data !== "string" || e.data.length > TEXT_MAX) return;
    onText(e.data);
  };
  addEventListener("message", heard);
  frame.src = to.href;
  holder.append(frame);
  return {
    frame,
    // The frame has no origin to name, so the text goes to whatever is in it: only typed commands, nothing secret.
    send: (text) => { const s = String(text); if (s.length <= TEXT_MAX) frame.contentWindow?.postMessage(s, "*"); },
    close: () => { removeEventListener("message", heard); frame.remove(); },
  };
}
