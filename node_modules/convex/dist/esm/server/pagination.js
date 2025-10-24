"use strict";
import { v } from "../values/validator.js";
export const paginationOptsValidator = v.object({
  numItems: v.number(),
  cursor: v.union(v.string(), v.null()),
  endCursor: v.optional(v.union(v.string(), v.null())),
  id: v.optional(v.number()),
  maximumRowsRead: v.optional(v.number()),
  maximumBytesRead: v.optional(v.number())
});
//# sourceMappingURL=pagination.js.map
