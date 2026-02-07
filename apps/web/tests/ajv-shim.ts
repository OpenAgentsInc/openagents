// apps/web currently pulls in Ajv v6 (via Wrangler / Workers SDK),
// which does not expose `dist/ajv.js` (Ajv v8 does). Import the v6 entry.
import AjvDefault from "ajv/lib/ajv.js";

export const Ajv = AjvDefault;
export default AjvDefault;
