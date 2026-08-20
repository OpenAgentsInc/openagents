import * as NodeHttpClient from "@effect/platform-node/NodeHttpClient";
import * as NodeServices from "@effect/platform-node/NodeServices";
import { Layer } from "effect";

import { apiTransportNodeLayer, networkPolicyLiveLayer } from "./api-transport.js";
import { browserLauncherLayer } from "./browser-launcher.js";
import { credentialStoreOsLayer } from "./credential-store.js";
import { deviceClientLayer } from "./device-client.js";
import { environmentLayer } from "./environment.js";
import { gitRunnerLayer } from "./git-runner.js";
import { outputLayer } from "./output.js";
import { repositoryClientLayer } from "./repository-client.js";
import { secretInputLayer } from "./secret-input.js";

const transportLayer = apiTransportNodeLayer.pipe(
  Layer.provide(Layer.merge(NodeHttpClient.layerFetch, networkPolicyLiveLayer)),
);

const repositoryLayer = repositoryClientLayer.pipe(Layer.provide(transportLayer));
const deviceLayer = deviceClientLayer.pipe(Layer.provide(transportLayer));
const credentialsLayer = credentialStoreOsLayer.pipe(Layer.provide(NodeServices.layer));
const browserLayer = browserLauncherLayer.pipe(Layer.provide(NodeServices.layer));

const nodeDependentServices = Layer.mergeAll(outputLayer, secretInputLayer, gitRunnerLayer).pipe(
  Layer.provide(NodeServices.layer),
);

export const runtimeLayer = Layer.mergeAll(
  NodeServices.layer,
  environmentLayer,
  credentialsLayer,
  repositoryLayer,
  deviceLayer,
  browserLayer,
  nodeDependentServices,
);
