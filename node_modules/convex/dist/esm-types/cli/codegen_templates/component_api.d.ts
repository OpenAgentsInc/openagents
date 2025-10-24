import { Context } from "../../bundler/context.js";
import { ComponentDirectory } from "../lib/components/definition/directoryStructure.js";
import { StartPushResponse } from "../lib/deployApi/startPush.js";
import { EvaluatedComponentDefinition } from "../lib/deployApi/componentDefinition.js";
import { Reference } from "../lib/deployApi/types.js";
export declare function componentApiJs(): string;
export declare function rootComponentApiCJS(): string;
export declare function componentApiStubDTS(): string;
export declare function componentApiDTS(ctx: Context, startPush: StartPushResponse, rootComponent: ComponentDirectory, componentDirectory: ComponentDirectory, opts: {
    staticApi: boolean;
}): Promise<string>;
export declare function resolveFunctionReference(ctx: Context, analysis: EvaluatedComponentDefinition, reference: Reference, visibility: "public" | "internal"): Promise<string>;
//# sourceMappingURL=component_api.d.ts.map