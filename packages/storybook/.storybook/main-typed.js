/** @type {import('@storybook/core-common').StorybookConfig} */
const config = {
  stories: ["../stories/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  
  addons: [
    "@storybook/addon-links",
    "@storybook/addon-essentials",
    "@storybook/addon-interactions"
  ],
  
  framework: {
    // Use a custom framework that can handle both HTML and Typed
    name: "@storybook/html-vite",
    options: {
      builder: {
        viteConfigPath: undefined
      }
    }
  },
  
  // Custom Vite configuration to handle Typed imports
  async viteFinal(config) {
    return {
      ...config,
      optimizeDeps: {
        ...config.optimizeDeps,
        include: [
          ...(config.optimizeDeps?.include || []),
          "@typed/ui",
          "@typed/fx",
          "@typed/template",
          "@typed/core",
          "@typed/dom",
          "effect"
        ]
      },
      resolve: {
        ...config.resolve,
        alias: {
          ...config.resolve?.alias,
          "@openagentsinc/storybook": "@openagentsinc/storybook/src/index.ts"
        }
      }
    }
  },
  
  typescript: {
    check: false
  },
  
  docs: {
    autodocs: "tag"
  }
}

export default config