// The playtest's page script (crate::playtest). In the house it offers the
// survey after a few minutes or at the tutorial's end; on the thank-you
// page it copies the share link. It reports only the steps the server
// can't see, and only for a playtest the visitor agreed to have recorded
// (the server checks the cookie).
const report = (step) => {
  try {
    navigator.sendBeacon("/playtest/events", new Blob([JSON.stringify({ step })], { type: "application/json" }));
  } catch {}
};

const finish = document.getElementById("playtest-finish");
const prompt = document.getElementById("playtest-prompt");
if (finish && prompt) {
  report("tutorial_opened");
  const opened = Date.now();
  let offered = false;
  const offer = () => {
    if (offered) return;
    offered = true;
    prompt.hidden = false;
    finish.classList.add("playtest-ready");
    report("finish_prompted");
  };
  setTimeout(offer, Number(finish.dataset.promptAfter || 240) * 1000);
  // The tutorial's last step shows the house's Save, Publish, and Share
  // buttons; seeing one after the first half minute counts as the end.
  const done = () => document.querySelector('[data-tutorial-option="ready"]:not([hidden])');
  new MutationObserver(() => {
    if (Date.now() - opened > 30000 && done()) offer();
  }).observe(document.body, { subtree: true, attributes: true, attributeFilter: ["hidden"] });
  document.getElementById("playtest-later")?.addEventListener("click", () => {
    prompt.hidden = true;
  });
}

const copy = document.getElementById("playtest-copy");
const share = document.getElementById("playtest-share");
if (copy && share) {
  copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(share.value);
    } catch {
      share.select();
    }
    copy.textContent = "Copied";
    report("share_copied");
  });
}
