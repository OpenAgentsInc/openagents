# 0149 OpenRouter Models Schema Analysis

## Overview

Analyzed the first ~300 lines of `docs/local/openrouter_models.json` to understand the schema structure and infer a TypeScript type definition.

## Schema Observations

### Root Structure
- Root object contains a single `data` array
- Array contains model definition objects

### Model Entry Structure

Each model entry contains the following fields:

1. **`id`** (string): Unique identifier, e.g., `"openai/gpt-5.1-codex-max"`
2. **`canonical_slug`** (string): Canonical identifier, may include version/date suffix
3. **`hugging_face_id`** (string): Hugging Face model ID, can be empty string `""`
4. **`name`** (string): Human-readable display name, e.g., `"OpenAI: GPT-5.1-Codex-Max"`
5. **`created`** (number): Unix timestamp (seconds since epoch)
6. **`description`** (string): Multi-line description text
7. **`context_length`** (number): Maximum context window size in tokens
8. **`architecture`** (object): Model architecture details
   - `modality`: String describing input/output, e.g., `"text+image->text"`, `"text->text"`
   - `input_modalities`: Array of strings, e.g., `["text", "image", "video", "file"]`
   - `output_modalities`: Array of strings, typically `["text"]`
   - `tokenizer`: String, e.g., `"GPT"`, `"Mistral"`, `"Nova"`, `"Other"`
   - `instruct_type`: Always `null` in observed examples
9. **`pricing`** (object): Pricing information (all values are strings, representing decimal numbers)
   - `prompt`: Cost per prompt token
   - `completion`: Cost per completion token
   - `request`: Cost per request (often `"0"`)
   - `image`: Cost per image (often `"0"`)
   - `web_search`: Cost for web search (often `"0"`)
   - `internal_reasoning`: Cost for internal reasoning (often `"0"`)
   - `input_cache_read`: Cost for cache reads (optional, not all models have this)
10. **`top_provider`** (object): Top provider configuration
    - `context_length`: Number (may differ from root `context_length`)
    - `max_completion_tokens`: Number or `null`
    - `is_moderated`: Boolean
11. **`per_request_limits`**: Always `null` in observed examples (likely an object when present)
12. **`supported_parameters`**: Array of strings, e.g., `["temperature", "max_tokens", "tools", ...]`
    - Common values: `temperature`, `top_p`, `max_tokens`, `stop`, `seed`, `tools`, `tool_choice`, `frequency_penalty`, `presence_penalty`, `logit_bias`, `logprobs`, `top_logprobs`, `response_format`, `structured_outputs`, `reasoning`, `include_reasoning`, `top_k`
13. **`default_parameters`** (object): Default parameter values
    - `temperature`: Number or `null`
    - `top_p`: Number or `null`
    - `frequency_penalty`: Number or `null`

### Notable Patterns

- **Pricing values are strings**: All pricing fields use string representation of decimal numbers (e.g., `"0.00000125"`), likely to preserve precision
- **Optional fields**: `input_cache_read` in pricing is not present on all models
- **Nullable fields**: `instruct_type`, `per_request_limits`, `max_completion_tokens` can be `null`
- **Parameter arrays**: `supported_parameters` and modality arrays are always present (non-empty arrays)
- **Free models**: Some models have `:free` suffix in `id` and have all pricing values as `"0"`

## Inferred TypeScript Schema

```typescript
type Modality = "text" | "image" | "video" | "file";
type Tokenizer = "GPT" | "Mistral" | "Nova" | "Other" | string;
type SupportedParameter =
  | "temperature"
  | "top_p"
  | "max_tokens"
  | "stop"
  | "seed"
  | "tools"
  | "tool_choice"
  | "frequency_penalty"
  | "presence_penalty"
  | "logit_bias"
  | "logprobs"
  | "top_logprobs"
  | "response_format"
  | "structured_outputs"
  | "reasoning"
  | "include_reasoning"
  | "top_k";

interface ModelArchitecture {
  modality: string; // e.g., "text+image->text", "text->text"
  input_modalities: Modality[];
  output_modalities: Modality[];
  tokenizer: Tokenizer;
  instruct_type: null; // Always null in observed examples
}

interface ModelPricing {
  prompt: string; // Decimal as string
  completion: string; // Decimal as string
  request: string; // Decimal as string
  image: string; // Decimal as string
  web_search: string; // Decimal as string
  internal_reasoning: string; // Decimal as string
  input_cache_read?: string; // Optional, decimal as string
}

interface TopProvider {
  context_length: number;
  max_completion_tokens: number | null;
  is_moderated: boolean;
}

interface DefaultParameters {
  temperature: number | null;
  top_p: number | null;
  frequency_penalty: number | null;
}

interface OpenRouterModel {
  id: string;
  canonical_slug: string;
  hugging_face_id: string; // Can be empty string
  name: string;
  created: number; // Unix timestamp (seconds)
  description: string;
  context_length: number;
  architecture: ModelArchitecture;
  pricing: ModelPricing;
  top_provider: TopProvider;
  per_request_limits: null | unknown; // Always null in examples, type unknown
  supported_parameters: SupportedParameter[];
  default_parameters: DefaultParameters;
}

interface OpenRouterModelsResponse {
  data: OpenRouterModel[];
}
```

## Notes

- The schema appears consistent across all observed entries
- Pricing uses string representation to avoid floating-point precision issues
- The `per_request_limits` field is always `null` in the sample, but likely has a structure when populated
- `instruct_type` is always `null` but included in the schema for completeness
- `SupportedParameter` type could be extended with more values as they appear in the full dataset


