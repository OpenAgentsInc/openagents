
// Presentation glue for Kiln's panel (#12440): the terminal's Kiln control,
// drawn only for site admins, opens /admin/kiln/ over the Grid. The server
// refuses that page to everyone else.
const kilnPanel = document.getElementById('kiln-panel');
const kilnFrame = document.getElementById('kiln-frame');
globalThis.homeKiln = open => {
  if (!open) {
    kilnPanel.hidden = true;
    world.focus({preventScroll:true});
    return;
  }
  if (globalThis.openagentsAdmin !== true) return;
  if (!kilnFrame.getAttribute('src')) kilnFrame.src = '/admin/kiln/';
  kilnPanel.hidden = false;
  if (chatField && document.activeElement === chatField) chatField.blur();
  kilnFrame.focus();
};
document.getElementById('kiln-close').addEventListener('click', () => homeKiln(false));
