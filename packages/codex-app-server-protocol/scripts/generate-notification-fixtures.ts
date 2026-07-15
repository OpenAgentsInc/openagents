import { writeFileSync } from "node:fs"
import { resolve } from "node:path"

import {
  SERVER_NOTIFICATION_DOCUMENTS,
} from "../src/_generated/current-source/wire.gen.ts"
import { decodeCurrentServerNotification } from "../src/decode.ts"

type JsonSchema = Readonly<Record<string, unknown>>
type JsonSchemaDocument = Readonly<{ schema: JsonSchema }>
const documents = SERVER_NOTIFICATION_DOCUMENTS as Readonly<Record<string, JsonSchemaDocument>>

const sample = (schema: JsonSchema | boolean, depth = 0): unknown => {
  if (depth > 80) return null
  if (schema === true) return null
  if (schema === false) throw new Error("cannot synthesize a false schema")
  if (typeof schema.$ref === "string") {
    throw new Error(`unexpected unresolved fixture ref: ${schema.$ref}`)
  }
  if ("const" in schema) return schema.const
  if (Array.isArray(schema.enum) && schema.enum.length > 0) return schema.enum[0]
  for (const union of [schema.oneOf, schema.anyOf]) {
    if (Array.isArray(union) && union.length > 0) return sample(union[0] as JsonSchema, depth + 1)
  }
  const type = Array.isArray(schema.type) ? schema.type[0] : schema.type
  if (type === "null") return null
  if (type === "boolean") return false
  if (type === "integer" || type === "number") {
    return typeof schema.minimum === "number" ? schema.minimum : 0
  }
  if (type === "string") return "x".repeat(Math.max(1, typeof schema.minLength === "number" ? schema.minLength : 1))
  if (type === "array") {
    const count = typeof schema.minItems === "number" ? schema.minItems : 0
    const item = typeof schema.items === "object" && schema.items !== null || typeof schema.items === "boolean"
      ? schema.items as JsonSchema | boolean
      : {}
    return Array.from({ length: count }, () => sample(item, depth + 1))
  }
  if (type === "object" || schema.properties !== undefined || schema.required !== undefined) {
    const properties = typeof schema.properties === "object" && schema.properties !== null
      ? schema.properties as Readonly<Record<string, JsonSchema | boolean>>
      : {}
    const required = Array.isArray(schema.required) ? schema.required.filter(key => typeof key === "string") : []
    return Object.fromEntries(required.map(key => [key, sample(properties[key] ?? {}, depth + 1)]))
  }
  if (Array.isArray(schema.allOf) && schema.allOf.length > 0) return sample(schema.allOf[0] as JsonSchema, depth + 1)
  return null
}

const fixtures = Object.fromEntries(Object.entries(documents).map(([method, document]) => {
  const payload = sample(document.schema)
  const decoded = decodeCurrentServerNotification(method, payload)
  if (decoded._tag === "DecodeFailure") throw new Error(`${method}: ${decoded.detail}`)
  return [method, payload]
}))

const output = resolve(import.meta.dirname, "..", "fixtures", "current-source-notifications.json")
writeFileSync(output, `${JSON.stringify(fixtures, null, 2)}\n`)

const completedSchema = documents["item/completed"]?.schema
const completedProperties = completedSchema?.properties as Readonly<Record<string, JsonSchema>> | undefined
const threadItemSchema = completedProperties?.item
const threadItemVariants = Array.isArray(threadItemSchema?.oneOf)
  ? threadItemSchema.oneOf.map(variant => sample(variant as JsonSchema))
  : []
const completed = fixtures["item/completed"] as Readonly<Record<string, unknown>>
for (const [index, item] of threadItemVariants.entries()) {
  const decoded = decodeCurrentServerNotification("item/completed", { ...completed, item })
  if (decoded._tag === "DecodeFailure") throw new Error(`ThreadItem ${index}: ${decoded.detail}`)
}
const itemOutput = resolve(import.meta.dirname, "..", "fixtures", "current-source-thread-items.json")
writeFileSync(itemOutput, `${JSON.stringify(threadItemVariants, null, 2)}\n`)
console.log(`Generated ${Object.keys(fixtures).length} notification and ${threadItemVariants.length} ThreadItem fixtures.`)
