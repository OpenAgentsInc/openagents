export default {
  providers: [
    {
      domain: process.env.OPENAUTH_DOMAIN || "http://localhost:8787",
      applicationID: "openagents",
    },
  ],
};