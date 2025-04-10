// For when we need a unique ID. We'll reuse the one from AI SDK

import { generateId as generateIdAi } from "ai";

export function generateId() {
  return generateIdAi();
}
