// Config plugin: set PROVISIONING_PROFILE_SPECIFIER to the UUID from
// expo/credentials/ios/profile.mobileprovision and force Manual signing.
// Avoids brittle identity strings and lets Xcode pick the matching cert.

const fs = require('fs');
const path = require('path');
const { withXcodeProject, createRunOncePlugin } = require('@expo/config-plugins');
const child_process = require('child_process');

function getProfileUUID(projectRoot, relativePath) {
  try {
    const profilePath = path.resolve(projectRoot, relativePath || 'credentials/ios/profile.mobileprovision');
    if (!fs.existsSync(profilePath)) return null;
    const out = child_process.execSync(`security cms -D -i ${JSON.stringify(profilePath)}`, { stdio: ['ignore', 'pipe', 'ignore'] });
    // naive parse for <key>UUID</key><string>...</string>
    const m = out.toString().match(/<key>UUID<\/key>\s*<string>([^<]+)<\/string>/);
    return m ? m[1].trim() : null;
  } catch (e) {
    return null;
  }
}

const withIOSProvisioningProfile = (config, props = {}) => {
  const { teamId, profilePath } = props;
  return withXcodeProject(config, (cfg) => {
    const project = cfg.modResults;
    const xcConfigs = project.pbxXCBuildConfigurationSection();
    const uuid = getProfileUUID(cfg.modRequest.projectRoot, profilePath);

    Object.entries(xcConfigs)
      .filter(([_, item]) => typeof item === 'object' && item.buildSettings)
      .forEach(([_, item]) => {
        const name = item.name || (item.buildSettings && item.buildSettings.NAME);
        const isRelease = String(name || '').toLowerCase().includes('release');
        if (!isRelease) return;
        item.buildSettings.CODE_SIGN_STYLE = 'Manual';
        if (teamId) item.buildSettings.DEVELOPMENT_TEAM = teamId;
        if (uuid) item.buildSettings.PROVISIONING_PROFILE_SPECIFIER = uuid;
      });
    return cfg;
  });
};

module.exports = createRunOncePlugin(withIOSProvisioningProfile, 'withIOSProvisioningProfile', '1.0.0');

