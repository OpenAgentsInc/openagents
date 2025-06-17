export interface PsionicConfig {
    name?: string;
    port?: number;
    catchAllRedirect?: boolean;
}
export type RouteHandler = (context: any) => string | Promise<string>;
export interface PsionicComponent {
    render(): string;
}
export interface PsionicEvent {
    id: string;
    type: string;
    data: any;
    timestamp: number;
}
//# sourceMappingURL=index.d.ts.map