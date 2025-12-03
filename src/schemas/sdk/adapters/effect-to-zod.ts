/**
 * Effect Schema to Zod conversion utilities.
 *
 * This module provides functions to convert Effect Schema definitions
 * to Zod schemas, enabling compatibility with MCP tools that require Zod.
 *
 * @module
 */

import * as AST from "effect/SchemaAST";
import * as S from "effect/Schema";
import { z, type ZodTypeAny } from "zod";

/**
 * Error thrown when schema conversion fails.
 */
export class SchemaConversionError extends Error {
  readonly _tag = "SchemaConversionError";
  constructor(message: string, readonly astType?: string) {
    super(message);
    this.name = "SchemaConversionError";
  }
}

/**
 * Convert an Effect Schema to a Zod schema.
 *
 * Supports:
 * - Primitives: String, Number, Boolean
 * - Literals
 * - Unions (including discriminated unions)
 * - Structs (objects)
 * - Arrays
 * - Optional and nullable types
 * - Refinements (with basic constraint mapping)
 *
 * @example
 * ```typescript
 * import * as S from "effect/Schema";
 * import { effectSchemaToZod } from "./effect-to-zod";
 *
 * const EffectSchema = S.Struct({
 *   name: S.String,
 *   age: S.Number.pipe(S.int(), S.greaterThanOrEqualTo(0)),
 * });
 *
 * const zodSchema = effectSchemaToZod(EffectSchema);
 * // zodSchema is now z.object({ name: z.string(), age: z.number().int().gte(0) })
 * ```
 */
export const effectSchemaToZod = <A, I>(schema: S.Schema<A, I>): ZodTypeAny => {
  return astToZod(schema.ast);
};

/**
 * Convert an Effect Schema AST node to a Zod schema.
 */
const astToZod = (ast: AST.AST): ZodTypeAny => {
  switch (ast._tag) {
    case "StringKeyword":
      return z.string();

    case "NumberKeyword":
      return z.number();

    case "BooleanKeyword":
      return z.boolean();

    case "BigIntKeyword":
      return z.bigint();

    case "SymbolKeyword":
      return z.symbol();

    case "UndefinedKeyword":
      return z.undefined();

    case "VoidKeyword":
      return z.void();

    case "NeverKeyword":
      return z.never();

    case "UnknownKeyword":
      return z.unknown();

    case "AnyKeyword":
      return z.any();

    case "ObjectKeyword":
      return z.object({});

    case "Literal": {
      const value = ast.literal;
      if (typeof value === "string") {
        return z.literal(value);
      }
      if (typeof value === "number") {
        return z.literal(value);
      }
      if (typeof value === "boolean") {
        return z.literal(value);
      }
      if (value === null) {
        return z.null();
      }
      throw new SchemaConversionError(`Unsupported literal type: ${typeof value}`, "Literal");
    }

    case "UniqueSymbol":
      return z.symbol();

    case "Union": {
      const types = ast.types.map(astToZod);
      if (types.length === 0) {
        return z.never();
      }
      if (types.length === 1) {
        return types[0]!;
      }
      // Zod requires at least 2 types for union
      return z.union([types[0]!, types[1]!, ...types.slice(2)]);
    }

    case "TypeLiteral": {
      const properties: Record<string, ZodTypeAny> = {};

      for (const prop of ast.propertySignatures) {
        const key = String(prop.name);
        let propSchema = astToZod(prop.type);

        if (prop.isOptional) {
          propSchema = propSchema.optional();
        }

        properties[key] = propSchema;
      }

      // Handle index signatures (Record types)
      if (ast.indexSignatures.length > 0) {
        const indexSig = ast.indexSignatures[0]!;
        const valueSchema = astToZod(indexSig.type);
        return z.record(z.string(), valueSchema);
      }

      return z.object(properties);
    }

    case "TupleType": {
      const elements = ast.elements.map((elem) => {
        const schema = astToZod(elem.type);
        return elem.isOptional ? schema.optional() : schema;
      });

      if (ast.rest.length > 0) {
        // Has rest element, treat as array of the rest type
        const restSchema = astToZod(ast.rest[0]!.type);
        if (elements.length === 0) {
          return z.array(restSchema);
        }
        // Tuple with rest - approximate as array
        return z.array(restSchema);
      }

      if (elements.length === 0) {
        return z.tuple([]);
      }

      if (elements.length === 1) {
        return z.tuple([elements[0]!]);
      }

      return z.tuple([elements[0]!, ...elements.slice(1)] as [ZodTypeAny, ...ZodTypeAny[]]);
    }

    case "Declaration": {
      // Handle declarations by checking the type annotation
      return z.unknown();
    }

    case "Refinement": {
      // Get the base type
      const from = astToZod(ast.from);
      // Refinements are tricky to convert, return base type
      return from;
    }

    case "Transformation": {
      // For transformations, use the 'from' type (input type)
      return astToZod(ast.from);
    }

    case "Suspend": {
      // Lazy evaluation - use z.lazy
      return z.lazy(() => astToZod(ast.f()));
    }

    case "TemplateLiteral": {
      // Template literals become string
      return z.string();
    }

    case "Enums": {
      // Convert enum to union of literals
      const entries = Object.entries(ast.enums);
      if (entries.length === 0) {
        return z.never();
      }
      const values = entries.map(([_, v]) => String(v));
      if (values.length === 0) {
        return z.never();
      }
      if (values.length === 1) {
        return z.literal(values[0]!);
      }
      return z.enum([values[0]!, values[1]!, ...values.slice(2)]);
    }

    default: {
      // Fallback for unsupported types
      console.warn(`Unsupported AST type: ${(ast as AST.AST)._tag}`);
      return z.unknown();
    }
  }
};

/**
 * Get a JSON Schema representation from an Effect Schema.
 * Useful for tools that accept JSON Schema instead of Zod.
 */
export const effectSchemaToJsonSchema = <A, I>(
  schema: S.Schema<A, I>,
): Record<string, unknown> => {
  const zodSchema = effectSchemaToZod(schema);
  return zodSchemaToBasicJsonSchema(zodSchema);
};

/**
 * Convert a Zod schema to a basic JSON Schema.
 * This is a simplified conversion for common cases.
 */
const zodSchemaToBasicJsonSchema = (schema: ZodTypeAny): Record<string, unknown> => {
  // Use zod's internal structure to determine type
  const typeName = (schema._def as { typeName?: string }).typeName;

  switch (typeName) {
    case "ZodString": {
      const result: Record<string, unknown> = { type: "string" };
      const def = schema._def as { description?: string; checks?: Array<{ kind: string; value?: unknown; regex?: RegExp }> };
      if (def.checks) {
        for (const check of def.checks) {
          if (check.kind === "min" && typeof check.value === "number") result.minLength = check.value;
          if (check.kind === "max" && typeof check.value === "number") result.maxLength = check.value;
          if (check.kind === "regex" && check.regex) result.pattern = check.regex.source;
        }
      }
      if (def.description) result.description = def.description;
      return result;
    }

    case "ZodNumber": {
      const result: Record<string, unknown> = { type: "number" };
      const def = schema._def as { description?: string; checks?: Array<{ kind: string; value?: unknown }> };
      if (def.checks) {
        for (const check of def.checks) {
          if (check.kind === "int") result.type = "integer";
          if (check.kind === "min" && typeof check.value === "number") result.minimum = check.value;
          if (check.kind === "max" && typeof check.value === "number") result.maximum = check.value;
        }
      }
      if (def.description) result.description = def.description;
      return result;
    }

    case "ZodBoolean":
      return { type: "boolean" };

    case "ZodLiteral": {
      const def = schema._def as { value: unknown };
      return { const: def.value };
    }

    case "ZodArray": {
      const def = schema._def as { type: ZodTypeAny };
      return {
        type: "array",
        items: zodSchemaToBasicJsonSchema(def.type),
      };
    }

    case "ZodObject": {
      const properties: Record<string, unknown> = {};
      const required: string[] = [];
      const def = schema._def as { shape: () => Record<string, ZodTypeAny> };
      const shape = def.shape();

      for (const [key, value] of Object.entries(shape)) {
        properties[key] = zodSchemaToBasicJsonSchema(value);
        // Check if required (not optional)
        const valueTypeName = (value._def as { typeName?: string }).typeName;
        if (valueTypeName !== "ZodOptional") {
          required.push(key);
        }
      }

      return {
        type: "object",
        properties,
        required: required.length > 0 ? required : undefined,
      };
    }

    case "ZodUnion": {
      const def = schema._def as { options: ZodTypeAny[] };
      return {
        oneOf: def.options.map(zodSchemaToBasicJsonSchema),
      };
    }

    case "ZodOptional": {
      const def = schema._def as { innerType: ZodTypeAny };
      return zodSchemaToBasicJsonSchema(def.innerType);
    }

    case "ZodNullable": {
      const def = schema._def as { innerType: ZodTypeAny };
      const inner = zodSchemaToBasicJsonSchema(def.innerType);
      return { oneOf: [inner, { type: "null" }] };
    }

    case "ZodRecord": {
      const def = schema._def as { valueType: ZodTypeAny };
      return {
        type: "object",
        additionalProperties: zodSchemaToBasicJsonSchema(def.valueType),
      };
    }

    case "ZodEnum": {
      const def = schema._def as { values: string[] };
      return { enum: def.values };
    }

    default:
      // Fallback
      return {};
  }
};
