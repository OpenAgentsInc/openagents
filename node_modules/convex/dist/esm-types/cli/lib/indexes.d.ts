import { Context } from "../../bundler/context.js";
import { DeveloperIndexConfig } from "./deployApi/finishPush.js";
export type IndexMetadata = {
    table: string;
    name: string;
    fields: string[] | {
        searchField: string;
        filterFields: string[];
    } | {
        dimensions: number;
        vectorField: string;
        filterFields: string[];
    };
    backfill: {
        state: "in_progress" | "done";
    };
    staged: boolean;
};
type SchemaState = {
    state: "pending";
} | {
    state: "validated";
} | {
    state: "active";
} | {
    state: "overwritten";
} | {
    state: "failed";
    error: string;
    tableName?: string;
};
export declare function pushSchema(ctx: Context, origin: string, adminKey: string, schemaDir: string, dryRun: boolean, deploymentName: string | null): Promise<{
    schemaId?: string;
    schemaState?: SchemaState;
}>;
export declare function addProgressLinkIfSlow(msg: string, deploymentName: string | null, start: number): string;
export declare function toIndexMetadata(index: DeveloperIndexConfig): IndexMetadata;
export declare function toDeveloperIndexConfig(index: IndexMetadata): DeveloperIndexConfig;
export declare function formatIndex(index: DeveloperIndexConfig): string;
export {};
//# sourceMappingURL=indexes.d.ts.map