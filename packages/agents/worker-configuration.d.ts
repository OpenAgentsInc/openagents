// Extended Env interface to support our needs
interface Env extends Cloudflare.Env {
  // API keys from settings
  apiKeys?: Record<string, string>;
  
  // GitHub token
  GITHUB_TOKEN?: string;
  
  // Allow other properties
  [key: string]: any;
}