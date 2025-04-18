const { MakerSquirrel } = require("@electron-forge/maker-squirrel");
const { MakerZIP } = require("@electron-forge/maker-zip");
const { MakerDeb } = require("@electron-forge/maker-deb");
const { MakerRpm } = require("@electron-forge/maker-rpm");
const { VitePlugin } = require("@electron-forge/plugin-vite");
const { FusesPlugin } = require("@electron-forge/plugin-fuses");
const { FuseV1Options, FuseVersion } = require("@electron/fuses");

const config = {
  publishers: [
    {
      name: '@electron-forge/publisher-github',
      config: {
        repository: {
          owner: 'OpenAgentsInc',
          name: 'openagents'
        },
        prerelease: true
      },
      generateReleaseNotes: true,
      authToken: process.env.GITHUB_TOKEN,
    }
  ],
  packagerConfig: {
    // Try boolean value for asar instead of pattern
    asar: true,
    extraResource: ['src/images'],
    icon: 'src/images/icon', // Base name without extension
  },
  rebuildConfig: {},
  makers: [
    new MakerSquirrel({
      iconUrl: 'https://raw.githubusercontent.com/openagents/coder/main/src/images/icon.ico',
      setupIcon: 'src/images/icon.ico'
    }),
    new MakerZIP({}, ["darwin"]),
    new MakerRpm({}),
    new MakerDeb({}),
  ],
  plugins: [
    new VitePlugin({
      // `build` can specify multiple entry builds, which can be Main process, Preload scripts, Worker process, etc.
      // If you are familiar with Vite configuration, it will look really familiar.
      build: [
        {
          // `entry` is just an alias for `build.lib.entry` in the corresponding file of `config`.
          entry: "src/main.ts",
          config: "vite.main.config.ts",
          target: "main",
        },
        {
          entry: "src/preload.ts",
          config: "vite.preload.config.ts",
          target: "preload",
        },
      ],
      renderer: [
        {
          name: "main_window",
          config: "vite.renderer.config.mts",
        },
      ],
    }),
    // Fuses are used to enable/disable various Electron functionality
    // at package time, before code signing the application
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
    }),
  ],
};

// Add some debug logging to see what's happening during the build
console.log('Electron Forge config loaded:');
console.log('- packagerConfig.asar:', config.packagerConfig.asar);
console.log('- plugins:', config.plugins.map(p => p.constructor.name));

// Add an error handler to the FusesPlugin
const originalFusesPlugin = config.plugins.find(p => p.constructor.name === 'FusesPlugin');
if (originalFusesPlugin) {
  const originalStartup = originalFusesPlugin.startupHook;
  originalFusesPlugin.startupHook = async (config) => {
    try {
      console.log('Running FusesPlugin startupHook...');
      await originalStartup(config);
      console.log('FusesPlugin startupHook completed successfully');
    } catch (error) {
      console.error('Error in FusesPlugin startupHook:', error);
      // Continue without failing the build
      return;
    }
  };
}

module.exports = config;
