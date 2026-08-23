import * as NodeHttpClient from "@effect/platform-node/NodeHttpClient";
import * as NodeServices from "@effect/platform-node/NodeServices";
import { Layer } from "effect";

import { apiTransportNodeLayer, networkPolicyLiveLayer } from "./api-transport.js";
import { browserLauncherLayer } from "./browser-launcher.js";
import { computerConfigurationLayer } from "./computer-config.js";
import { computerClientLayer } from "./computer-client.js";
import { computerJournalLayer } from "./computer-journal.js";
import { computerProbeLayer } from "./computer-probe.js";
import { credentialStoreOsLayer } from "./credential-store.js";
import { pendingDeviceAuthorizationStoreLayer } from "./device-authorization-store.js";
import { deviceClientLayer } from "./device-client.js";
import { environmentLayer } from "./environment.js";
import { forumClientLayer } from "./forum-client.js";
import { gitRunnerLayer } from "./git-runner.js";
import { outputLayer } from "./output.js";
import { persistedConfigurationLayer } from "./persisted-configuration.js";
import { repositoryClientLayer } from "./repository-client.js";
import { requestBodyInputLayer } from "./request-body-input.js";
import { secretInputLayer } from "./secret-input.js";
import { terminalSessionNodeLayer } from "./terminal-session.js";

const transportLayer = apiTransportNodeLayer.pipe(
  Layer.provide(Layer.merge(NodeHttpClient.layerFetch, networkPolicyLiveLayer)),
);

const repositoryLayer = repositoryClientLayer.pipe(Layer.provide(transportLayer));
const forumLayer = forumClientLayer.pipe(Layer.provide(transportLayer));
const deviceLayer = deviceClientLayer.pipe(Layer.provide(transportLayer));
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
  deviceLayer,
  computerClient,
  browserLayer,
  computerConfiguration,
  computerJournal,
  computerProbe,
  nodeDependentServices,
);
