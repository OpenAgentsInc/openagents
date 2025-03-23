import "@/utils/polyfill-crypto"
import "text-encoding-polyfill"
import { Buffer } from "buffer"

global.Buffer = Buffer
