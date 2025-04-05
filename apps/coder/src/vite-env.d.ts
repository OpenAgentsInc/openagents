/// <reference types="vite/client" />

// Vite environment variables
interface ImportMetaEnv {
  readonly MODE: string;
  readonly DEV: boolean;
  readonly PROD: boolean;
  // Add other environment variables here as needed
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

// Window interface extensions
interface Window {
  forceShowApp?: number; // Timeout ID
  hideInitialLoader?: () => void;
}
