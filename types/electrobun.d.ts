declare module "electrobun/bun" {
  import type { RPCSchema } from "rpc-anywhere";

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

  export class BrowserView<T = unknown> {
    id: number;
    constructor(options?: Partial<BrowserWindowOptions<T>> & { windowId?: number | null });
    static getById<U>(id: number): BrowserView<U> | undefined;
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

declare module "electrobun/*" {
  export * from "electrobun/bun";
}
