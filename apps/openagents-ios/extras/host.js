// Firm reason: 1 (WebKit service transport for the independent Coax extras module).
addEventListener("coaxial-dom-ready", event => {
  if (event.detail?.app !== "extras") return;
  const dom = event.detail.host;
  dom.after = () => {
    const x = dom.exports(), state = dom.state();
    if (x.dom_act_kind(state) !== 1) return;
    x.dom_act_done(state);
    coaxNative.call("wallet.balance").then(r => dom.send(21, 0, r.record))
      .catch(() => dom.send(21, 0, ""));
  };
});
