import * as NodeHttpClient from "@effect/platform-node/NodeHttpClient";
import * as NodeServices from "@effect/platform-node/NodeServices";
import { Layer } from "effect";

import { apiTransportNodeLayer, networkPolicyLiveLayer } from "./api-transport.js";
import { boxClientLayer } from "./box-client.js";
import { browserLauncherLayer } from "./browser-launcher.js";
import { computerConfigurationLayer } from "./computer-config.js";
import { computerClientLayer } from "./computer-client.js";
import { computerChannelNodeLayer, computerSocketNodeLayer } from "./computer-channel.js";
import { computerJournalLayer } from "./computer-journal.js";
import { computerProbeLayer } from "./computer-probe.js";
import { computerUpLayer } from "./computer-up.js";
import { computerAgentProcessNodeLayer } from "./computer-agents.js";
import { credentialStoreOsLayer } from "./credential-store.js";
import { pendingDeviceAuthorizationStoreLayer } from "./device-authorization-store.js";
import { deviceClientLayer } from "./device-client.js";
import { environmentLayer } from "./environment.js";
import { fleetClientLayer } from "./fleet-client.js";
import { forumClientLayer } from "./forum-client.js";
import { gitRunnerLayer } from "./git-runner.js";
import { issueClientLayer } from "./issue-client.js";
import { memoryClientLayer } from "./memory-client.js";
import { outputLayer } from "./output.js";
import { persistedConfigurationLayer } from "./persisted-configuration.js";
import { projectClientLayer } from "./project-client.js";
import { repositoryClientLayer } from "./repository-client.js";
import { requestBodyInputLayer } from "./request-body-input.js";
import { secretInputLayer } from "./secret-input.js";
import { terminalSessionNodeLayer } from "./terminal-session.js";

const transportLayer = apiTransportNodeLayer.pipe(
  Layer.provide(Layer.merge(NodeHttpClient.layerFetch, networkPolicyLiveLayer)),
);

const repositoryLayer = repositoryClientLayer.pipe(Layer.provide(transportLayer));
const forumLayer = forumClientLayer.pipe(Layer.provide(transportLayer));
const fleetLayer = fleetClientLayer.pipe(Layer.provide(transportLayer));
const deviceLayer = deviceClientLayer.pipe(Layer.provide(transportLayer));
const issueLayer = issueClientLayer.pipe(Layer.provide(transportLayer));
const projectLayer = projectClientLayer.pipe(Layer.provide(transportLayer));
const memoryLayer = memoryClientLayer.pipe(Layer.provide(transportLayer));
const boxClient = boxClientLayer.pipe(Layer.provide(transportLayer));
const computerClient = computerClientLayer.pipe(Layer.provide(transportLayer));
const credentialsLayer = credentialStoreOsLayer.pipe(Layer.provide(NodeServices.layer));
const pendingAuthorizationLayer = pendingDeviceAuthorizationStoreLayer.pipe(
  Layer.provide(environmentLayer),
);
const browserLayer = browserLauncherLayer.pipe(Layer.provide(NodeServices.layer));
const persistedLayer = persistedConfigurationLayer.pipe(Layer.provide(environmentLayer));
const computerConfiguration = computerConfigurationLayer.pipe(Layer.provide(environmentLayer));
const computerJournal = computerJournalLayer.pipe(Layer.provide(computerConfiguration));
const computerProbe = computerProbeLayer.pipe(
  Layer.provide(Layer.merge(computerConfiguration, NodeServices.layer)),
);
const computerChannel = computerChannelNodeLayer.pipe(Layer.provide(computerSocketNodeLayer));
const computerUp = computerUpLayer.pipe(
  Layer.provide(
    Layer.mergeAll(
      computerChannel,
      computerAgentProcessNodeLayer,
      computerClient,
      computerConfiguration,
      computerJournal,
      computerProbe,
      credentialsLayer,
    ),
  ),
);

const nodeDependentServices = Layer.mergeAll(
  outputLayer,
  secretInputLayer,
  requestBodyInputLayer,
  gitRunnerLayer,
).pipe(Layer.provide(NodeServices.layer));

export const runtimeLayer = Layer.mergeAll(
  NodeServices.layer,
  environmentLayer,
  transportLayer,
  persistedLayer,
  terminalSessionNodeLayer,
  credentialsLayer,
  pendingAuthorizationLayer,
  repositoryLayer,
  forumLayer,
  fleetLayer,
  issueLayer,
  projectLayer,
  memoryLayer,
  deviceLayer,
  boxClient,
  computerClient,
  browserLayer,
  computerConfiguration,
  computerJournal,
  computerProbe,
  computerUp,
  nodeDependentServices,
);
