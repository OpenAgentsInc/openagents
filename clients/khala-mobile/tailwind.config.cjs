const { khalaMobileNativeWindTokens } = require("./src/theme/nativewind-tokens.cjs")

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
