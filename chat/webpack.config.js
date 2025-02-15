const createExpoWebpackConfigAsync = require("@expo/webpack-config");
const path = require("path");

module.exports = async function (env, argv) {
  const config = await createExpoWebpackConfigAsync(
    {
      ...env,
      babel: {
        dangerouslyAddModulePathsToTranspile: [
          "@expo/webpack-config/web-default/index.html",
        ],
      },
    },
    argv,
  );

  // Customize the config for /chat path
  config.output.publicPath = "/chat/";

  // Fix static file handling
  config.output.path = path.resolve(__dirname, "web-build");

  // Handle HTML template variables
  config.plugins.forEach((plugin) => {
    if (plugin.constructor.name === "HtmlWebpackPlugin") {
      plugin.userOptions.template = path.resolve(__dirname, "web/index.html");
      plugin.userOptions.templateParameters = {
        ...plugin.userOptions.templateParameters,
        LANG_ISO_CODE: "en",
        WEB_TITLE: "OpenAgents Chat",
      };
    }
  });

  return config;
};
