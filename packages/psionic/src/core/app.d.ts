import { Elysia } from "elysia";
import type { PsionicConfig, RouteHandler } from "../types";
export declare class PsionicApp {
    private app;
    private config;
    constructor(config: PsionicConfig);
    route(path: string, handler: RouteHandler): this;
    start(): this;
    get elysia(): Elysia<"", {
        decorator: {};
        store: {};
        derive: {};
        resolve: {};
    }, {
        typebox: {};
        error: {};
    }, {
        schema: {};
        standaloneSchema: {};
        macro: {};
        macroFn: {};
        parser: {};
    }, {}, {
        derive: {};
        resolve: {};
        schema: {};
        standaloneSchema: {};
    }, {
        derive: {};
        resolve: {};
        schema: {};
        standaloneSchema: {};
    }>;
}
export declare function createPsionicApp(config: PsionicConfig): PsionicApp;
//# sourceMappingURL=app.d.ts.map