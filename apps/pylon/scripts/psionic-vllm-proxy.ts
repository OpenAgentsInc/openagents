#!/usr/bin/env bun
import { handlePsionicVllmProxyRequest } from "../src/psionic-vllm-proxy.js"

const envValue = (name: string): string => {
  const value = process.env[name]?.trim()
  if (!value) {
    throw new Error(`${name} is required`)
  }
  return value
}

const port = Number(process.env.PYLON_PSIONIC_PROXY_PORT ?? "8011")
if (!Number.isInteger(port) || port <= 0) {
  throw new Error("PYLON_PSIONIC_PROXY_PORT must be a positive integer")
}

const config = {
  bearerToken: envValue("PYLON_PSIONIC_PROXY_BEARER_TOKEN"),
  canaryRef:
    process.env.PYLON_PSIONIC_PROXY_CANARY_REF?.trim() ??
    "canary.pylon.serving.known_answer.ok.v1",
  nodeRef: envValue("PYLON_PSIONIC_PROXY_NODE_REF"),
  replayChallengeRef: envValue("PYLON_PSIONIC_PROXY_REPLAY_CHALLENGE_REF"),
  servedModel: envValue("PYLON_PSIONIC_PROXY_SERVED_MODEL"),
  upstreamModel: envValue("PYLON_PSIONIC_PROXY_UPSTREAM_MODEL"),
  upstreamUrl: envValue("PYLON_PSIONIC_PROXY_UPSTREAM_URL"),
}

Bun.serve({
  fetch: request => handlePsionicVllmProxyRequest(request, config),
  hostname: "127.0.0.1",
  port,
})

console.log(`Pylon Psionic vLLM proxy listening on 127.0.0.1:${port}`)
