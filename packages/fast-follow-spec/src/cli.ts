#!/usr/bin/env node
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";
import {
  computeDocumentDigest,
  computeIntentDigest,
  parseFastFollow,
  serializeFastFollow,
  starterFastFollow,
} from "./index.ts";

const fail = (message: string, code = 2): never => {
  console.error(message);
  process.exit(code);
};
const read = (path: string) => readFileSync(path, "utf8");

const main = () => {
  const [command, ...args] = process.argv.slice(2);
  if (command === "validate") {
    if (args.length === 0) fail("usage: fast-follow-spec validate <FASTFOLLOW.md...>");
    let failures = 0;
    for (const path of args) {
      const result = parseFastFollow(read(path));
      if (result.valid) console.log(`ok ${path}`);
      else {
        failures += 1;
        console.error(`FAIL ${path}`);
        for (const item of result.diagnostics)
          console.error(`  ${item.code}${item.path ? ` (${item.path})` : ""}: ${item.message}`);
      }
    }
    if (failures > 0) process.exit(1);
    return;
  }
  if (command === "digest" || command === "projection") {
    if (args.length === 0) fail(`usage: fast-follow-spec ${command} <FASTFOLLOW.md...>`);
    let failures = 0;
    for (const path of args) {
      const source = read(path);
      const result = parseFastFollow(source);
      if (!result.valid) {
        failures += 1;
        console.error(`FAIL ${path}: ${result.diagnostics.map((item) => item.code).join(", ")}`);
        continue;
      }
      if (command === "digest")
        console.log(
          `${path}\n  document ${computeDocumentDigest(source)}\n  intent   ${computeIntentDigest(result.document)}`,
        );
      else console.log(JSON.stringify(result.document.projection, null, 2));
    }
    if (failures > 0) process.exit(1);
    return;
  }
  if (command === "init") {
    const path = args[0];
    if (!path)
      fail("usage: fast-follow-spec init <FASTFOLLOW.md> [--title title] [--id stable.id]");
    if (existsSync(path)) fail(`refusing to overwrite existing file: ${path}`, 1);
    const titleIndex = args.indexOf("--title");
    const idIndex = args.indexOf("--id");
    const title =
      titleIndex >= 0 ? args[titleIndex + 1] : basename(path, ".md").replaceAll(/[-_]/g, " ");
    const id = idIndex >= 0 ? args[idIndex + 1] : "project.fast_follow";
    if (!title || !id) fail("--title and --id require values");
    writeFileSync(path, starterFastFollow(title, id), { encoding: "utf8", flag: "wx" });
    console.log(`created ${path}`);
    return;
  }
  if (command === "format") {
    const path = args[0];
    if (!path) fail("usage: fast-follow-spec format <FASTFOLLOW.md>");
    const result = parseFastFollow(read(path));
    if (!result.valid)
      return fail(result.diagnostics.map((item) => `${item.code}: ${item.message}`).join("\n"), 1);
    writeFileSync(path, serializeFastFollow(result.document));
    return;
  }
  fail("usage: fast-follow-spec <validate|digest|projection|init> ...");
};

main();
