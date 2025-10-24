import { Command } from "@commander-js/extra-typings";
export declare const deploy: Command<[], {
    verbose?: boolean;
    dryRun?: boolean;
    yes?: boolean;
    typecheck: "try" | "enable" | "disable";
    typecheckComponents: boolean;
    codegen: "enable" | "disable";
    cmd?: string;
    cmdUrlEnvVarName?: string;
    debugBundlePath?: string;
    debug?: boolean;
    writePushRequest?: string;
    liveComponentSources?: boolean;
    previewRun?: string;
    previewCreate?: string;
    checkBuildEnvironment: "enable" | "disable";
    adminKey?: string;
    url?: string;
    previewName?: string;
    envFile?: string;
}>;
//# sourceMappingURL=deploy.d.ts.map