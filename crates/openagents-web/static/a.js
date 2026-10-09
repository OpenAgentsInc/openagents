// First-party, cookieless counts (#11153): a few clicks, sent only to this
// site's /a. Nothing is stored in the browser. Off when the browser asks
// not to be tracked (Do Not Track or Global Privacy Control).
(() => {
  const nav = navigator;
  if (nav.doNotTrack === "1" || window.doNotTrack === "1" || nav.globalPrivacyControl === true) return;
  if (typeof nav.sendBeacon !== "function") return;
  const send = (e, d) => {
    try {
      nav.sendBeacon("/a", new URLSearchParams({ e, d: d || "" }));
    } catch (_) {
      // Counting never gets in the way.
    }
  };
  document.addEventListener(
    "click",
    (event) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target) return;
      const chip = target.closest(".oa-suggestions button");
      if (chip) {
        send("starter_clicked", chip.textContent.trim());
        return;
      }
      const link = target.closest("a[href]");
      if (!link) return;
      if (link.closest(".oa-home-stage")) {
        send("card_clicked", link.getAttribute("href"));
      } else if (/\/openagentsgemini-(oa-updates|cli-releases)\//.test(link.href)) {
        send("download_clicked", link.href);
      }
    },
    true,
  );
  document.addEventListener("copy", () => {
    const text = String(document.getSelection() || "");
    if (text.includes("/cli/install.sh")) send("install_copied", "shell");
    else if (text.includes("/cli/install.ps1")) send("install_copied", "powershell");
  });
})();
