/// <reference path="./.sst/platform/config.d.ts" />

export default $config({
  app(input) {
    return {
      name: "openagents-auth",
      removal: input?.stage === "production" ? "retain" : "remove",
      home: "cloudflare",
      providers: {
        cloudflare: true,
      },
    };
  },
  async run() {
    // KV namespace for OpenAuth storage
    const authStorage = new sst.cloudflare.Kv("AuthStorage", {});

    // Cloudflare Worker for OpenAuth
    const auth = new sst.cloudflare.Worker("OpenAuth", {
      handler: "./apps/auth/src/index.ts",
      url: true,
      link: [authStorage],
      environment: {
        // Environment variables will be set via wrangler secrets
        // GITHUB_CLIENT_ID: process.env.GITHUB_CLIENT_ID!,
        // GITHUB_CLIENT_SECRET: process.env.GITHUB_CLIENT_SECRET!,
      },
    });

    return {
      auth: auth.url,
      storage: authStorage.id,
    };
  },
});