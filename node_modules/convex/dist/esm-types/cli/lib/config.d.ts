import { Context } from "../../bundler/context.js";
import { Bundle, BundleHash } from "../../bundler/index.js";
import { NodeDependency } from "./deployApi/modules.js";
import { ComponentDefinitionPath } from "./components/definition/directoryStructure.js";
export { productionProvisionHost, provisionHost } from "./utils/utils.js";
/** Type representing auth configuration. */
export interface AuthInfo {
    applicationID: string;
    domain: string;
}
/** Type representing Convex project configuration. */
export interface ProjectConfig {
    functions: string;
    node: {
        externalPackages: string[];
        nodeVersion?: string;
    };
    generateCommonJSApi: boolean;
    project?: string | undefined;
    team?: string | undefined;
    prodUrl?: string | undefined;
    authInfo?: AuthInfo[];
    codegen: {
        staticApi: boolean;
        staticDataModel: boolean;
    };
}
export interface Config {
    projectConfig: ProjectConfig;
    modules: Bundle[];
    nodeDependencies: NodeDependency[];
    schemaId?: string;
    udfServerVersion?: string;
    nodeVersion?: string | undefined;
}
export interface ConfigWithModuleHashes {
    projectConfig: ProjectConfig;
    moduleHashes: BundleHash[];
    nodeDependencies: NodeDependency[];
    schemaId?: string;
    udfServerVersion?: string;
}
/** Parse object to ProjectConfig. */
export declare function parseProjectConfig(ctx: Context, obj: any): Promise<ProjectConfig>;
export declare function configName(): string;
export declare function configFilepath(ctx: Context): Promise<string>;
export declare function getFunctionsDirectoryPath(ctx: Context): Promise<string>;
/** Read configuration from a local `convex.json` file. */
export declare function readProjectConfig(ctx: Context): Promise<{
    projectConfig: ProjectConfig;
    configPath: string;
}>;
export declare function enforceDeprecatedConfigField(ctx: Context, config: ProjectConfig, field: "team" | "project" | "prodUrl"): Promise<string>;
/**
 * Given a {@link ProjectConfig}, add in the bundled modules to produce the
 * complete config.
 */
export declare function configFromProjectConfig(ctx: Context, projectConfig: ProjectConfig, configPath: string, verbose: boolean): Promise<{
    config: Config;
    bundledModuleInfos: BundledModuleInfo[];
}>;
/**
 * Bundle modules one by one for good bundler errors.
 */
export declare function debugIsolateEndpointBundles(ctx: Context, projectConfig: ProjectConfig, configPath: string): Promise<void>;
/**
 * Read the config from `convex.json` and bundle all the modules.
 */
export declare function readConfig(ctx: Context, verbose: boolean): Promise<{
    config: Config;
    configPath: string;
    bundledModuleInfos: BundledModuleInfo[];
}>;
export declare function upgradeOldAuthInfoToAuthConfig(ctx: Context, config: ProjectConfig, functionsPath: string): Promise<ProjectConfig>;
/** Write the config to `convex.json` in the current working directory. */
export declare function writeProjectConfig(ctx: Context, projectConfig: ProjectConfig, { deleteIfAllDefault }?: {
    deleteIfAllDefault: boolean;
}): Promise<undefined>;
export declare function removedExistingConfig(ctx: Context, configPath: string, options: {
    allowExistingConfig?: boolean;
}): boolean;
/** Pull configuration from the given remote origin. */
export declare function pullConfig(ctx: Context, project: string | undefined, team: string | undefined, origin: string, adminKey: string): Promise<ConfigWithModuleHashes>;
interface BundledModuleInfo {
    name: string;
    platform: "node" | "convex";
}
/**
 * A component definition spec contains enough information to create bundles
 * of code that must be analyzed in order to construct a ComponentDefinition.
 *
 * Most paths are relative to the directory of the definitionPath.
 */
export type ComponentDefinitionSpec = {
    /** This path is relative to the app (root component) directory. */
    definitionPath: ComponentDefinitionPath;
    /** Dependencies are paths to the directory of the dependency component definition from the app (root component) directory */
    dependencies: ComponentDefinitionPath[];
    definition: Bundle;
    schema: Bundle;
    functions: Bundle[];
};
export type AppDefinitionSpec = Omit<ComponentDefinitionSpec, "definitionPath"> & {
    auth: Bundle | null;
};
export type ComponentDefinitionSpecWithoutImpls = Omit<ComponentDefinitionSpec, "schema" | "functions">;
export type AppDefinitionSpecWithoutImpls = Omit<AppDefinitionSpec, "schema" | "functions" | "auth">;
export declare function configJSON(config: Config, adminKey: string, schemaId?: string, pushMetrics?: PushMetrics, bundledModuleInfos?: BundledModuleInfo[]): {
    config: {
        projectSlug: string | undefined;
        teamSlug: string | undefined;
        functions: string;
        authInfo: AuthInfo[] | undefined;
    };
    modules: Bundle[];
    nodeDependencies: {
        name: string;
        version: string;
    }[];
    udfServerVersion: string | undefined;
    schemaId: string | undefined;
    adminKey: string;
    pushMetrics: PushMetrics | undefined;
    bundledModuleInfos: BundledModuleInfo[] | undefined;
    nodeVersion: string | undefined;
};
export type PushMetrics = {
    typecheck: number;
    bundle: number;
    schemaPush: number;
    codePull: number;
    totalBeforePush: number;
};
/** Push configuration to the given remote origin. */
export declare function pushConfig(ctx: Context, config: Config, options: {
    adminKey: string;
    url: string;
    deploymentName: string | null;
    pushMetrics?: PushMetrics | undefined;
    schemaId?: string | undefined;
    bundledModuleInfos?: BundledModuleInfo[];
}): Promise<void>;
type Files = {
    source: string;
    filename: string;
}[];
export type CodegenResponse = {
    success: true;
    files: Files;
} | {
    success: false;
    error: string;
};
type ModuleDiffStat = {
    count: number;
    size: number;
};
export type ModuleDiffStats = {
    updated: ModuleDiffStat;
    identical: ModuleDiffStat;
    added: ModuleDiffStat;
    numDropped: number;
};
/** Generate a human-readable diff between the two configs. */
export declare function diffConfig(oldConfig: ConfigWithModuleHashes, newConfig: Config, shouldDiffModules: boolean): {
    diffString: string;
    stats?: ModuleDiffStats | undefined;
};
export declare function handlePushConfigError(ctx: Context, error: unknown, defaultMessage: string, deploymentName: string | null, deployment?: {
    deploymentUrl: string;
    adminKey: string;
    deploymentNotice: string;
}): Promise<never>;
//# sourceMappingURL=config.d.ts.map