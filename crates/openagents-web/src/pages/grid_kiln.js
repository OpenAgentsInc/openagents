// Presentation glue for Kiln's panel (#12440, #12459): the terminal's Kiln
// control, drawn only for site admins, opens it. The GPUI chat draws the
// panel and asks `homeOverlay` to stand /admin/kiln/ in it. The server
// refuses that page to everyone else.
globalThis.homeKiln = open => {
  open = !!open && globalThis.openagentsAdmin === true;
  globalThis.openagentsKilnOpen = open;
  if (open) {
    if (chatField && document.activeElement === chatField) chatField.blur();
    return;
  }
  world.focus({preventScroll: true});
};
