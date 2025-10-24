import { Context } from "../../bundler/context.js";
import { ProjectConfig } from "./config.js";
import { LogManager } from "./logs.js";
export type PushOptions = {
    adminKey: string;
    verbose: boolean;
    dryRun: boolean;
    typecheck: "enable" | "try" | "disable";
    typecheckComponents: boolean;
    debug: boolean;
    debugBundlePath?: string | undefined;
    debugNodeApis: boolean;
    codegen: boolean;
    url: string;
    deploymentName: string | null;
    writePushRequest?: string | undefined;
    liveComponentSources: boolean;
    logManager?: LogManager | undefined;
};
export declare function runNonComponentsPush(ctx: Context, options: PushOptions, configPath: string, projectConfig: ProjectConfig): Promise<void>;
//# sourceMappingURL=push.d.ts.map