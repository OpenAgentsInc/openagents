# 2026-06-26 Khala GLM / OpenRouter Fallback After-Action

## Status

Fixed in the same change as this after-action.

The public Khala route now uses this main fallback thread:

1. GLM-5.2 REAP 504B private Hydralisk G4 fleet
2. OpenRouter hidden fallback, pinned in source to upstream model `openrouter/free`
3. Vertex Gemini
4. Fireworks

GPT-OSS is not in the main Khala fallback thread. Raw GPT-OSS model ids remain
separate explicit supply-lane requests only.

## What Failed

The CLI surfaced repeated `inference_unavailable` failures to users even though
the product promise is that Khala should fall back through armed public lanes
instead of exposing backend saturation or Spot/preemption churn as a dead chat.

The confusing part was that live metadata showed the request was reaching the
GLM-primary route, then escaping to OpenRouter:

```text
primaryAdapterId: hydralisk-vllm-glm-5p2-reap-504b
servedAdapterId: openrouter-khala-glm-fallback
fallbackReason: rate_limited
servedModel: z-ai/glm-5.2-20260616
```

At the same time, the GLM readiness endpoint could show the fleet as ready. That
is not a contradiction: the GLM fleet can be healthy while its bounded inflight
slots are saturated, which is a retryable lane condition and must overflow.

## Root Cause

The previous OpenRouter fallback had two dangerous properties:

- Registration depended on env-supplied OpenRouter model configuration, which
  made production behavior drift-prone.
- The documented and tested route still carried historical GPT-OSS overflow
  expectations in places, even after GLM became the primary Khala route.

That made it too easy for the deployed route, docs, and tests to disagree about
whether a saturated GLM request should fall to GLM-class OpenRouter, GPT-OSS,
Gemini, or Fireworks.

## Fix

- Pinned the OpenRouter Khala fallback upstream model in code to
  `openrouter/free`.
- Armed OpenRouter from `OPENROUTER_API_KEY` presence only; the legacy model env
  is ignored by registration.
- Set the main Khala adapter plan to
  `GLM -> OpenRouter -> Vertex Gemini -> Fireworks`.
- Rewrote router tests so GPT-OSS adapters registered in the test process are
  not called by the main Khala thread.
- Added a regression that forces GLM, OpenRouter, and Gemini to fail and proves
  Fireworks is the final fallback.
- Updated Worker config comments so the deployed environment documentation
  matches the code.
- Shipped CLI v0.1.6 with restored streaming output and cleaner token counter
  output.

## Verification

Required checks for this incident class:

- Router unit tests prove the exact fallback order.
- OpenRouter adapter tests prove the upstream payload model is `openrouter/free`.
- Model-serving policy tests prove OpenRouter is armed by API key presence, not
  by a mutable model env.
- Live smoke should verify `/api/khala/chat` streams and returns metadata with
  `primaryAdapterId` set to GLM and no GPT-OSS adapter id in the main route.

