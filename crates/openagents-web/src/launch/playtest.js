// The launch playtest's page script (crate::launch). On the intro it hands
// the recording choice to the research script; in the house it offers the
// survey after a few minutes or at the tutorial's end; on the thank-you
// page it copies the share link. It reports only the steps the server
// can't see, and the server drops them when the visitor opted out.
const report = (step) => {
  try {
    navigator.sendBeacon("/playtest/events", new Blob([JSON.stringify({ step })], { type: "application/json" }));
  } catch {}
};

// The intro's two start buttons: the research script keeps the choice
// (the server sets the same cookie when the form arrives).
for (const button of document.querySelectorAll('button[name="record"]')) {
  button.addEventListener("click", () => {
    try {
      window.oaResearch?.optOut(button.value === "0");
    } catch {}
  });
}

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
    report("tutorial_finish_prompted");
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
