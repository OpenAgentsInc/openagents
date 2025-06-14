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
  }
}

export default config