// Minimal config plugin to force CODE_SIGN_IDENTITY for Release builds.
// This helps when Xcode picks the wrong identity or when the CN label
// differs from what EAS assumes (e.g., "iPhone Distribution" vs "Apple Distribution").

const { withXcodeProject, createRunOncePlugin } = require('@expo/config-plugins');

const withIOSCodeSignIdentity = (config, props = {}) => {
  const identity = props.identity;
  const teamId = props.teamId; // optional, if you want to force DEVELOPMENT_TEAM too

  if (!identity) {
    return config;
  }

  return withXcodeProject(config, (cfg) => {
    const project = cfg.modResults;
    const xcConfigs = project.pbxXCBuildConfigurationSection();

    Object.entries(xcConfigs)
      .filter(([key, item]) => typeof item === 'object' && item.buildSettings)
      .forEach(([key, item]) => {
        const name = item.name || (item.buildSettings && item.buildSettings.NAME);
        const isRelease = String(name || '').toLowerCase().includes('release');
        if (!isRelease) return;

        // Prefer manual signing to avoid auto selection surprises
        item.buildSettings.CODE_SIGN_STYLE = 'Manual';
        // Set identity (avoid bracketed sdk-specific key to keep xcodeproj parser happy)
        const needsQuoting = /[,:()\s]/.test(identity);
        const quotedIdentity = needsQuoting && !/^\".*\"$/.test(identity) ? `"${identity}"` : identity;
        item.buildSettings.CODE_SIGN_IDENTITY = quotedIdentity;

        if (teamId) {
          item.buildSettings.DEVELOPMENT_TEAM = teamId;
        }
      });

    return cfg;
  });
};

module.exports = createRunOncePlugin(withIOSCodeSignIdentity, 'withIOSCodeSignIdentity', '1.0.0');
