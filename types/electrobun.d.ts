declare module "electrobun/bun" {
  import type { RPCSchema, RPC } from "rpc-anywhere";

  export interface BrowserWindowFrame {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  export interface BrowserWindowOptions<T = unknown> {
    title?: string;
    frame?: Partial<BrowserWindowFrame>;
    url?: string | null;
    html?: string | null;
    preload?: string | null;
    renderer?: "native" | "cef";
    rpc?: T;
    styleMask?: Record<string, boolean>;
    titleBarStyle?: "hiddenInset" | "default";
    navigationRules?: string | null;
  }

  export interface ElectrobunWebviewRpcSchema {
    bun: RPCSchema;
    webview: RPCSchema;
  }

  export interface BrowserViewRpcConfig<Schema extends ElectrobunWebviewRpcSchema> {
    maxRequestTime?: number;
    handlers: {
      requests?: Record<string, (...args: any[]) => any>;
      messages?: Record<string, (payload: any) => void>;
    };
  }

  export class BrowserView<T = unknown> {
    id: number;
    rpc: T;
    constructor(options?: Partial<BrowserWindowOptions<T>> & { windowId?: number | null });
    static getById<U>(id: number): BrowserView<U> | undefined;
    static defineRPC<Schema extends ElectrobunWebviewRpcSchema>(
      config: BrowserViewRpcConfig<Schema>
    ): RPC<Schema["webview"], Schema["bun"]> & { send: Record<string, (payload: any) => void> };
    loadURL(url: string): void;
    loadHTML(html: string): void;
  }

  export class BrowserWindow<T = unknown> {
    constructor(options?: Partial<BrowserWindowOptions<T>>);
    id: number;
    title: string;
    frame: BrowserWindowFrame;
    url: string | null;
    html: string | null;
    preload: string | null;
    renderer: "native" | "cef";
    webviewId: number;
    get webview(): BrowserView<T>;
    setTitle(title: string): unknown;
    close(): unknown;
    focus(): unknown;
    on(name: string, handler: (...args: any[]) => void): void;
    static getById<U>(id: number): BrowserWindow<U> | undefined;
  }

  export class Tray {
    constructor(icon: string);
    setMenu(menu: unknown): void;
  }

  export const ApplicationMenu: Record<string, unknown>;
  export const ContextMenu: Record<string, unknown>;
  export const PATHS: Record<string, unknown>;
  export const Socket: Record<string, unknown>;
  export type ElectrobunEvent = unknown;

  export { type RPCSchema };

  const Electrobun: {
    BrowserWindow: typeof BrowserWindow;
    BrowserView: typeof BrowserView;
    Tray: typeof Tray;
    ApplicationMenu: typeof ApplicationMenu;
    ContextMenu: typeof ContextMenu;
    PATHS: typeof PATHS;
    Socket: typeof Socket;
  };

  export default Electrobun;
  export {
    BrowserWindow,
    BrowserView,
    Tray,
    ApplicationMenu,
    ContextMenu,
    PATHS,
    Socket,
    BrowserWindowOptions,
    BrowserWindowFrame,
    Electrobun as ElectrobunNamespace
  };
}

declare module "electrobun" {
  export * from "electrobun/bun";
}

declare module "electrobun/view" {
  import type { RPCSchema, RPC } from "rpc-anywhere";

  export interface ElectrobunWebviewRpcSchema {
    bun: RPCSchema;
    webview: RPCSchema;
  }

  export interface ElectroviewRpcConfig<Schema extends ElectrobunWebviewRpcSchema> {
    maxRequestTime?: number;
    handlers: {
      requests?: Record<string, (...args: any[]) => any>;
      messages?: Record<string, (payload: any) => void>;
    };
  }

  export class Electroview<T = unknown> {
    rpc?: T;
    constructor(config: { rpc: T });
    static defineRPC<Schema extends ElectrobunWebviewRpcSchema>(
      config: ElectroviewRpcConfig<Schema>
    ): RPC<Schema["bun"], Schema["webview"]> & { send: Record<string, (payload: any) => void> };
  }

  const Electrobun: {
    Electroview: typeof Electroview;
  };

  export default Electrobun;
  export { Electroview };
}

declare module "electrobun/*" {
  export * from "electrobun/bun";
}
