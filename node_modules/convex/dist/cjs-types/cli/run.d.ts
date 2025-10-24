import { Command } from "@commander-js/extra-typings";
export declare const run: Command<[string, string | undefined], {
    watch?: boolean;
    push?: boolean;
    identity?: string;
    typecheck: "try" | "enable" | "disable";
    typecheckComponents: boolean;
    codegen: "enable" | "disable";
    component?: string;
    liveComponentSources?: boolean;
} & {
    envFile?: string;
    url?: string;
    adminKey?: string;
    prod?: boolean;
    previewName?: string;
    deploymentName?: string;
}>;
//# sourceMappingURL=run.d.ts.map