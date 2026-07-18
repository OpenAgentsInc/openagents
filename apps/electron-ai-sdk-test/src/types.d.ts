interface Window {
  harnessDesktop: {
    getEndpoint(): Promise<string>;
  };
}

declare const MAIN_WINDOW_VITE_DEV_SERVER_URL: string;
declare const MAIN_WINDOW_VITE_NAME: string;

declare module "*.css";
