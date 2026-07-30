import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const chartPath = process.argv[2];
const configPath = process.argv[3];
if (!chartPath || !configPath) {
  throw new Error("usage: strict-config-post-render.mjs CHART_YAML LIVEKIT_CONFIG");
}

let chart = await readFile(chartPath, "utf8");
const config = await readFile(configPath, "utf8");
const relativeKeyMountMatches = chart.match(/mountPath: keys\.yaml/g) ?? [];
if (relativeKeyMountMatches.length !== 1) {
  throw new Error(`expected one relative key-file mount, found ${relativeKeyMountMatches.length}`);
}
chart = chart.replace("mountPath: keys.yaml", "mountPath: /etc/livekit/keys.yaml");
const documents = chart.split(/\n---\n/);
const configMapIndexes = documents.flatMap((document, index) =>
  /^kind: ConfigMap$/m.test(document) && /^  name: livekit-server$/m.test(document) ? [index] : [],
);
const deploymentIndexes = documents.flatMap((document, index) =>
  /^kind: Deployment$/m.test(document) && /^  name: livekit-server$/m.test(document) ? [index] : [],
);

if (configMapIndexes.length !== 1) {
  throw new Error(`expected one livekit-server ConfigMap, found ${configMapIndexes.length}`);
}
if (deploymentIndexes.length !== 1) {
  throw new Error(`expected one livekit-server Deployment, found ${deploymentIndexes.length}`);
}

const indentedConfig = config
  .trimEnd()
  .split("\n")
  .map((line) => `    ${line}`)
  .join("\n");
const configMapIndex = configMapIndexes[0];
const configMap = documents[configMapIndex];
const replacedConfigMap = configMap.replace(
  /data:\n  config\.yaml: \|\n(?:    .*(?:\n|$))+/,
  `data:\n  config.yaml: |\n${indentedConfig}\n`,
);
if (replacedConfigMap === configMap) {
  throw new Error("failed to replace the chart-generated LiveKit configuration");
}
documents[configMapIndex] = replacedConfigMap.trimEnd();

const checksum = createHash("sha256").update(config).digest("hex");
const deploymentIndex = deploymentIndexes[0];
const deployment = documents[deploymentIndex];
const replacedDeployment = deployment.replace(
  /checksum\/config: [^\n]+/,
  `checksum/config: ${checksum}`,
);
if (replacedDeployment === deployment) {
  throw new Error("failed to bind Deployment checksum to the strict LiveKit configuration");
}
documents[deploymentIndex] = replacedDeployment;

process.stdout.write(`${documents.join("\n---\n").trimEnd()}\n`);
