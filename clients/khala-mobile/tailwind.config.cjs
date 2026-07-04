const { openAgentsNativeWindTokens } = require("@openagentsinc/ui/nativewind-tokens.cjs")

module.exports = {
  content: [
    "./app/**/*.{js,jsx,ts,tsx}",
    "./src/**/*.{js,jsx,ts,tsx}"
  ],
  presets: [require("nativewind/preset")],
  theme: {
    extend: openAgentsNativeWindTokens
  },
  plugins: []
}
