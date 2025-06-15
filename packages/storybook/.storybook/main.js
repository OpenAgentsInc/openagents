/** @type {import('@openagentsinc/storybook').StorybookConfig} */
const config = {
  stories: ["../stories/**/*.stories.@(js|jsx|mjs|ts|tsx)"],
  
  addons: [
    "@storybook/addon-links",
    "@storybook/addon-essentials",
    "@storybook/addon-interactions"
  ],
  
  framework: {
    name: "@storybook/html-vite",
    options: {}
  },
  
  typescript: {
    check: false
  },
  
  docs: {
    autodocs: "tag"
  },
  
  async viteFinal(config) {
    return {
      ...config,
      resolve: {
        ...config.resolve,
        alias: {
          ...config.resolve?.alias,
          '@storybook/html/dist/entry-preview.mjs': '@storybook/html/dist/entry-preview.mjs',
          '@storybook/html/dist/entry-preview-docs.mjs': '@storybook/html/dist/entry-preview-docs.mjs'
        }
      },
      optimizeDeps: {
        ...config.optimizeDeps,
        include: [
          ...(config.optimizeDeps?.include || []),
          '@storybook/html'
        ]
      }
    }
  }
}

export default config