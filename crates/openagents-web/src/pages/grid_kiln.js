// Presentation glue for Kiln at the terminal (#12440, #12456, #12488): the
// terminal's Kiln control, drawn for signed-in players the site offers Kiln
// to (`/home/me`'s `kiln`), opens it. The GPUI chat draws Kiln's panel, its
// cards, and its run; this loads Kiln's module once, without its page
// (`start_headless`), so the chat can read `kilnState()` and call
// `kilnAgent` and the rest. The server refuses Kiln's files to everyone
// else.
let kilnModule;
const startKiln = () => {
  kilnModule ??= import('/kiln/pkg/kiln.js')
    .then(async kiln => {
      await kiln.default({module_or_path: '/kiln/pkg/kiln_bg.wasm'});
      kiln.start_headless('/kiln/');
    })
    .catch(error => {
      kilnModule = undefined;
      console.warn('Kiln did not start', error);
    });
  return kilnModule;
};
globalThis.homeKiln = open => {
  open = !!open && globalThis.openagentsKiln === true;
  globalThis.openagentsKilnOpen = open;
  document.documentElement.classList.toggle('kiln-open', open);
  if (open) {
    startKiln();
    if (chatField && document.activeElement === chatField) chatField.blur();
    return;
  }
  world.focus({preventScroll: true});
};
