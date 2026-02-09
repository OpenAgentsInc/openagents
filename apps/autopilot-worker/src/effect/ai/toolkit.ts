import { Tool as AiTool, Toolkit } from "@effect/ai"
import { Schema } from "effect"
import type { DseToolContract } from "@openagentsinc/dse"

export const aiToolFromContract = <I, O>(contract: DseToolContract<I, O>) => {
  // We keep tool failureMode="return" + failure=unknown so tool handler failures
  // can be surfaced as tool-result(isFailure=true) parts instead of failing the
  // entire model stream.
  const tool = AiTool.make(contract.name, {
    description: contract.description,
    failureMode: "return",
    failure: Schema.Unknown,
  })
    // DSE tool contracts use `Schema.Struct(...)` for tool inputs. Cast to
    // `Schema.Struct<any>` to avoid TS inferring the "fields" overload and
    // collapsing parameters to `{}` under `exactOptionalPropertyTypes`.
    .setParameters(contract.input as unknown as Schema.Struct<any>)
    .setSuccess((contract.output ?? Schema.Unknown) as Schema.Schema<unknown>)

  return tool
}

export const makeToolkitFromContracts = (
  contracts: ReadonlyArray<DseToolContract<any, any>>,
) => Toolkit.make(...contracts.map(aiToolFromContract))
