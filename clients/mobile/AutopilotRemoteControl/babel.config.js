module.exports = function (api) {
  api.cache(true)

  return {
    // unstable_transformImportMeta: effect (pulled in transitively via the
    // shared @openagentsinc/autopilot-control-protocol package) uses
    // `import.meta`, which Hermes doesn't support natively. This babel option
    // transforms it. JS-transform config only — does not change the native
    // fingerprint, so it ships OTA.
    presets: [["babel-preset-expo", { unstable_transformImportMeta: true }]],
  }
}
