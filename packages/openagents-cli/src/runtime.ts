import * as NodeHttpClient from "@effect/platform-node/NodeHttpClient";
import * as NodeServices from "@effect/platform-node/NodeServices";
import { Layer } from "effect";

import { apiTransportNodeLayer, networkPolicyLiveLayer } from "./api-transport.js";
import { credentialStoreUnavailableLayer } from "./credential-store.js";
import { environmentLayer } from "./environment.js";
import { gitRunnerLayer } from "./git-runner.js";
import { outputLayer } from "./output.js";
import { repositoryClientLayer } from "./repository-client.js";
import { secretInputLayer } from "./secret-input.js";

const transportLayer = apiTransportNodeLayer.pipe(
  Layer.provide(Layer.merge(NodeHttpClient.layerFetch, networkPolicyLiveLayer)),
);

const repositoryLayer = repositoryClientLayer.pipe(Layer.provide(transportLayer));

const nodeDependentServices = Layer.mergeAll(outputLayer, secretInputLayer, gitRunnerLayer).pipe(
  Layer.provide(NodeServices.layer),
);

export const runtimeLayer = Layer.mergeAll(
  NodeServices.layer,
  environmentLayer,
  credentialStoreUnavailableLayer,
  repositoryLayer,
  nodeDependentServices,
);
