import fs from 'node:fs';
import path from 'node:path';
import { chromium } from 'playwright';

const baseURL = process.env.E2E_BASE_URL ?? 'https://openagents.com';
const storageStatePath =
  process.env.E2E_STORAGE_STATE ??
  path.resolve(process.cwd(), '.auth', 'storageState.json');

const waitForEnter = () =>
  new Promise((resolve) => {
    process.stdin.resume();
    process.stdin.setEncoding('utf8');
    process.stdin.once('data', () => resolve());
  });

async function main() {
  const authDir = path.dirname(storageStatePath);
  fs.mkdirSync(authDir, { recursive: true });

  const browser = await chromium.launch({ headless: false });
  const context = await browser.newContext();
  const page = await context.newPage();

  console.log(`Opening ${baseURL} ...`);
  await page.goto(baseURL, { waitUntil: 'domcontentloaded' });
  console.log('Log in manually in the opened browser.');
  console.log('When you are fully logged in, press Enter here to save state.');
  await waitForEnter();

  await context.storageState({ path: storageStatePath });
  await browser.close();
  console.log(`Saved storage state to: ${storageStatePath}`);
}

main().catch((err) => {
  console.error('Failed to save auth state:', err);
  process.exit(1);
});
