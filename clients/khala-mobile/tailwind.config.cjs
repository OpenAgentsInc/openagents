const { openAgentsNativeWindTokens } = require("@openagentsinc/ui/nativewind-tokens.cjs")

const khalaMobileNativeWindTokens = {
  ...openAgentsNativeWindTokens,
  colors: {
    ...openAgentsNativeWindTokens.colors,
    bg: "#02060d"
  }
}

module.exports = {
  content: [
    "./index.{js,jsx,ts,tsx}",
    "./src/**/*.{js,jsx,ts,tsx}"
  ],
  presets: [require("nativewind/preset")],
  theme: {
    extend: khalaMobileNativeWindTokens
  },
  plugins: []
}
