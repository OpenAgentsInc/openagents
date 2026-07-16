import { runExecutedConformanceReport } from "../src/report.ts";

process.stdout.write(`${JSON.stringify(await runExecutedConformanceReport(), null, 2)}\n`);
