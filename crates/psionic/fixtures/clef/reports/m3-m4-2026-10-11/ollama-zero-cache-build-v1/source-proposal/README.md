# Proposed zero-prefix-reuse Ollama comparison

Use a separate scratch-only build of Ollama 0.40.2 at commit
`b061384d90ff455462bc32745dd0de479de717a3`, with the two-line
[candidate patch](candidate.patch). It calls the existing scoring cache's
`close()` before each scoring row begins. Do not modify the installed app,
service, model artifacts, request body, tokenizer, forward pass, or head.
This is a source-backed proposal, not an executed or qualified runtime.

## Why this path is concrete

The pinned SystemOne request type exposes model, state, media, questions, and
keep-alive fields, but no prefix-reuse switch. The inspected environment registry
has no equivalent switch. `OLLAMA_KV_CACHE_TYPE` controls representation.
The runner's `DELETE /v1/models` branch is a TODO, not a usable reset endpoint.
These are findings about the inspected pinned paths, not a claim about every
future or private option. Sources and exact file hashes are in
[sources.json](sources.json).

Clef calls `scoreHiddenRow`, which obtains a `beginScore` session. Its reported
cached count is `input length - remaining length`; only remaining tokens are
forwarded. The proposed change resets the cache before this calculation rather
than overriding the reported count.

`prefixCache.close()` closes every trie snapshot, frees each model cache, and
resets its root, active path, and paged-out byte count. The KV cache's `Free()`
clears keys/values and sets its offset to zero. The recurrent cache clears
convolution/Delta state and its offset. The hidden cache clears saved backbone
outputs. The next session therefore starts from an empty root and offset zero,
subject to validation below. The same within-request state still accumulates
across chunks. Existing snapshot scheduling and end-of-session behavior remain.

The scorer runs on the existing serialized MLX thread. The request handler
synchronizes the stream before returning. Resetting at the next `beginScore`
does not interrupt another score's head. The model's resident weights and the
MLX allocator are separate from the scoring prefix cache. Generation uses a
separate cache and is unchanged. The dedicated benchmark process should serve
only the pinned Clef artifact and serial requests.

Primary source:
[prefix lifecycle](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/mlxrunner/prefix_cache.go),
[scoring path](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/mlxrunner/score.go),
[KV reset](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/mlxrunner/cache/kvcache.go),
[recurrent reset](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/mlxrunner/cache/recurrent.go),
and [hidden-output reset](https://github.com/ollama/ollama/blob/b061384d90ff455462bc32745dd0de479de717a3/mlxrunner/cache/hidden.go).

## Qualification before timing

1. Preserve and hash an unmodified checkout and build, then the one-patch build,
   from the same commit, toolchain, MLX dependencies, compiler options, and model
   blobs. Review the exact source diff. Label results as a modified Ollama
   benchmark build, not the unmodified installed release.
2. Add narrow lifecycle tests before compiling: first request, exact repeat,
   divergent suffix sharing the template, long request with snapshots, and
   cancellation followed by a fresh request. Each next session must have offset
   zero, all input tokens remaining, no old hidden rows, and no live trie history.
   Exercise KV, recurrent, and hidden caches, including snapshot cleanup.
3. Compare the modified build's first and repeated requests with the unmodified
   build's fresh-process first request. Keep artifact identity, encoding, chunk
   settings, operators, and complete payload bytes identical. Use strict existing
   raw-logit/probability and top-answer checks; the reset must not merely turn a
   cached run into a different numerical path without disclosure.
4. For every accepted SystemOne reply, require the top-level
   `prompt_eval_cached_count` to be present, typed as an integer, and exactly zero.
   Also retain the scorer's cache log and input-token count. Missing telemetry is
   unknown, not zero. Any nonzero count fails the uncached gate; do not rewrite or
   discard it. Compare exact saved HTTP body bytes and hashes against native
   requests, including the same nonce, model field, question order, and state.
5. Only after those checks, prepare a separate guarded, paired timing attempt.
   Retain every sample, reset work, response, memory reading, and cleanup receipt.
   Cache reset is inside each measured HTTP request. Report its cost as part of
   uncached serving; do not subtract it. Native prefix caching remains disabled.

The earlier 37–39 cached template tokens remain disclosed in their original
results. A leading state nonce does not erase the shared template. Allocator
reuse is not token-prefix reuse, and it must not be used to relabel those results.

The patch has not been applied. No Go/C++ build, server, model, or GPU workload
ran for this investigation. It requires a separately reviewed build and execution
plan with fixed resource limits. The current proposal includes only the source
diff, source identities, and this qualification plan.
