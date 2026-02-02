#!/usr/bin/env node
/**
 * List custom domains for all Cloudflare Pages projects.
 * Finds which project has a given domain (e.g. openagents.com).
 *
 * Usage:
 *   CLOUDFLARE_API_TOKEN=xxx node scripts/list-pages-domains.mjs
 *   CLOUDFLARE_API_TOKEN=xxx node scripts/list-pages-domains.mjs openagents.com
 *
 * Get API token: https://dash.cloudflare.com/profile/api-tokens
 * Create token with "Cloudflare Pages" Read. Account ID from: wrangler whoami
 */
const ACCOUNT_ID = process.env.CLOUDFLARE_ACCOUNT_ID || "54fac8b750a29fdda9f2fa0f0afaed90";
const API_TOKEN = process.env.CLOUDFLARE_API_TOKEN;
const FIND_DOMAIN = process.argv[2]?.toLowerCase(); // e.g. openagents.com

if (!API_TOKEN) {
  console.error("Set CLOUDFLARE_API_TOKEN (create at https://dash.cloudflare.com/profile/api-tokens, scope: Cloudflare Pages Read)");
  process.exit(1);
}

const headers = { Authorization: `Bearer ${API_TOKEN}` };
const base = `https://api.cloudflare.com/client/v4/accounts/${ACCOUNT_ID}/pages/projects`;

async function main() {
  const listRes = await fetch(base, { headers });
  if (!listRes.ok) {
    console.error("List projects failed:", listRes.status, await listRes.text());
    process.exit(1);
  }
  const list = await listRes.json();
  if (!list.success || !list.result) {
    console.error("List response:", list);
    process.exit(1);
  }

  const projects = Array.isArray(list.result) ? list.result : [list.result];
  console.log("Pages projects and custom domains:\n");

  let foundProject = null;

  for (const proj of projects) {
    const name = proj.name;
    const subdomain = proj.subdomain || proj.domains?.[0] || "";
    console.log(`  ${name} (${subdomain})`);

    const domainsRes = await fetch(`${base}/${encodeURIComponent(name)}/domains`, { headers });
    if (!domainsRes.ok) {
      console.log("    custom: (could not list)");
      console.log("");
      continue;
    }
    const domainsJson = await domainsRes.json();
    const customDomains = domainsJson.result ?? (Array.isArray(domainsJson) ? domainsJson : []);
    const domainList = Array.isArray(customDomains) ? customDomains : (customDomains.items ?? []);
    const names = domainList.map((d) => (typeof d === "string" ? d : d?.name ?? d?.domain ?? "")).filter(Boolean);
    if (names.length) {
      console.log(`    custom: ${names.join(", ")}`);
      if (FIND_DOMAIN && names.some((n) => n.toLowerCase() === FIND_DOMAIN)) {
        foundProject = name;
        console.log(`    >>> "${FIND_DOMAIN}" is attached to this project`);
      }
    } else {
      console.log("    custom: (none)");
    }
    console.log("");
  }

  if (FIND_DOMAIN) {
    if (foundProject) {
      console.log(`To use ${FIND_DOMAIN} on "website":`);
      console.log(`  1. Remove it from "${foundProject}": Dashboard → Workers & Pages → ${foundProject} → Custom domains → remove ${FIND_DOMAIN}`);
      console.log(`  2. Add to "website": Workers & Pages → website → Custom domains → Set up a custom domain → ${FIND_DOMAIN}`);
    } else {
      console.log(`"${FIND_DOMAIN}" not found in any project's custom domains (or API returned different shape).`);
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
