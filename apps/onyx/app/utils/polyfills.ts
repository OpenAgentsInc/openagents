import "@/utils/polyfill-crypto"
import "text-encoding-polyfill"
import { Buffer } from "buffer"
import { Platform } from "react-native"
import structuredClone from "@ungap/structured-clone"

global.Buffer = Buffer

if (Platform.OS !== "web") {
  const setupPolyfills = async () => {
    const { polyfillGlobal } = await import(
      "react-native/Libraries/Utilities/PolyfillFunctions"
    )
    const { TextEncoderStream, TextDecoderStream } = await import(
      "@stardazed/streams-text-encoding"
    )

    if (!("structuredClone" in global)) {
      polyfillGlobal("structuredClone", () => structuredClone)
    }
    polyfillGlobal("TextEncoderStream", () => TextEncoderStream)
    polyfillGlobal("TextDecoderStream", () => TextDecoderStream)
  }

  setupPolyfills()
}

export { }
