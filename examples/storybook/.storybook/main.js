import { dirname, join } from "path"
import { fileURLToPath } from "url"

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

/**
 * This function is used to resolve the absolute path of a package.
 */
function getAbsolutePath(value) {
  return dirname(require.resolve(join(value, "package.json")))
}

/** @type {import('@openagentsinc/storybook').StorybookConfig} */
const config = {
  stories: ["../stories/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  
  addons: [
    getAbsolutePath("@storybook/addon-links"),
    getAbsolutePath("@storybook/addon-essentials"),
    getAbsolutePath("@storybook/addon-interactions")
  ],
  
  framework: {
    name: getAbsolutePath("@openagentsinc/storybook"),
    options: {}
  },
  
  typescript: {
    check: false,
    reactDocgen: "react-docgen-typescript",
    reactDocgenTypescriptOptions: {
      shouldExtractLiteralValuesFromEnum: true,
      propFilter: (prop) => (prop.parent ? !/node_modules/.test(prop.parent.fileName) : true)
    }
  },
  
  docs: {
    autodocs: "tag"
  },
  
  viteFinal: (config, { configType }) => {
    // Customize Vite config here if needed
    return config
  }
}

export default config