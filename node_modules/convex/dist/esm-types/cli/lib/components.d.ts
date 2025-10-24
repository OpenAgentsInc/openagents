import { Context } from "../../bundler/context.js";
import { ProjectConfig } from "./config.js";
import { PushOptions } from "./push.js";
import { CodegenOptions } from "./codegen.js";
import { DeploymentSelection } from "./deploymentSelection.js";
export declare function runCodegen(ctx: Context, deploymentSelection: DeploymentSelection, options: CodegenOptions): Promise<undefined>;
export declare function runPush(ctx: Context, options: PushOptions): Promise<void>;
export declare function runComponentsPush(ctx: Context, options: PushOptions, configPath: string, projectConfig: ProjectConfig): Promise<void>;
//# sourceMappingURL=components.d.ts.map