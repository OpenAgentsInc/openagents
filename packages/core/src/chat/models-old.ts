export const models = [
  // Default model that matches the default in settings-repository.ts
  // {
  //   provider: "anthropic",
  //   id: "claude-3-5-sonnet-20240620",
  //   name: "Claude 3.5 Sonnet (20240620)",
  //   created: 1729555200,
  //   description: "Default Claude 3.5 Sonnet model from June 2024 release",
  //   shortDescription: "Default Claude 3.5 Sonnet model",
  //   context_length: 200000,
  //   plan: "free",
  //   supportsTools: true,
  // },

  {
    provider: "openrouter",
    id: "deepseek/deepseek-chat-v3-0324:free",
    name: "DeepSeek V3 0324 (free)",
    created: 1742824755,
    description: "DeepSeek V3, a 685B-parameter, mixture-of-experts model, is the latest iteration of the flagship chat model family from the DeepSeek team.",
    shortDescription: "Latest DeepSeek model featuring 685B parameters with mixture-of-experts architecture.",
    context_length: 64000,
    plan: "free",
    supportsTools: true,
    "architecture": {
      "modality": "text->text",
      "tokenizer": "DeepSeek",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0",
      "completion": "0",
      "image": "0",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 64000,
      "max_completion_tokens": 8192,
      "is_moderated": false
    },
    "per_request_limits": null
  },

  // {
  //   provider: "groq",
  //   id: "llama-3.3-70b-versatile",
  //   name: "Llama 3.3 70B",
  //   created: 1731196800,
  //   context_length: 128000,
  //   plan: "free",
  //   "pricing": {
  //     "prompt": "0",
  //     "completion": "0",
  //     "image": "0",
  //     "request": "0",
  //     "input_cache_read": "0",
  //     "input_cache_write": "0",
  //     "web_search": "0",
  //     "internal_reasoning": "0"
  //   },
  // },

  // {
  //   provider: "groq",
  //   id: "qwen-qwq-32b",
  //   name: "Qwen QWQ 32B",
  //   created: 1731196800,
  //   context_length: 128000,
  //   plan: "free",
  //   description: "QwQ is the reasoning model of the Qwen series. Compared with conventional instruction-tuned models, QwQ, which is capable of thinking and reasoning, can achieve significantly enhanced performance in downstream tasks, especially hard problems. QwQ-32B is the medium-sized reasoning model, which is capable of achieving competitive performance against state-of-the-art reasoning models, e.g., DeepSeek-R1, o1-mini.",
  //   shortDescription: "A medium-sized reasoning model from the Qwen series that excels at complex problems through enhanced thinking capabilities.",
  //   supportsTools: true, // Explicitly mark as supporting tools
  //   "pricing": {
  //     "prompt": "0",
  //     "completion": "0",
  //     "image": "0",
  //     "request": "0",
  //     "input_cache_read": "0",
  //     "input_cache_write": "0",
  //     "web_search": "0",
  //     "internal_reasoning": "0"
  //   },
  // },

  {
    provider: "openrouter",
    id: "deepseek/deepseek-chat-v3-0324",
    name: "DeepSeek V3 0324",
    created: 1742824755,
    description: "DeepSeek V3, a 685B-parameter, mixture-of-experts model, is the latest iteration of the flagship chat model family from the DeepSeek team.",
    shortDescription: "Latest DeepSeek model featuring 685B parameters with mixture-of-experts architecture.",
    context_length: 64000,
    plan: "pro",
    supportsTools: true,
    "architecture": {
      "modality": "text->text",
      "tokenizer": "DeepSeek",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.00000027",
      "completion": "0.0000011",
      "image": "0",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 64000,
      "max_completion_tokens": 8192,
      "is_moderated": false
    },
    "per_request_limits": null
  },

  {
    provider: "openrouter",
    plan: "pro",
    "id": "deepseek/deepseek-r1",
    "name": "DeepSeek R1 (671B)",
    "created": 1737381095,
    "description": "DeepSeek R1 is the largest model in the DeepSeek series. As a reasoning model it cannot use tools.",
    "context_length": 64000,
    "supportsTools": false,
    "architecture": {
      "modality": "text->text",
      "tokenizer": "DeepSeek",
      "instruct_type": "deepseek-r1"
    },
    "pricing": {
      "prompt": "0.00000055",
      "completion": "0.00000219",
      "image": "0",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 64000,
      "max_completion_tokens": 64000,
      "is_moderated": false
    },
    "per_request_limits": null
  },

  // {
  //   provider: "groq",
  //   id: "deepseek-r1-distill-qwen-32b",
  //   name: "DeepSeek R1 (Qwen Distill)",
  //   created: 1731196800,
  //   context_length: 128000,
  //   plan: "free",
  //   "pricing": {
  //     "prompt": "0.0000025",
  //     "completion": "0.00001",
  //     "image": "0.003613",
  //     "request": "0",
  //     "input_cache_read": "0",
  //     "input_cache_write": "0",
  //     "web_search": "0",
  //     "internal_reasoning": "0"
  //   },
  // },
  // {
  //   provider: "groq",
  //   id: "deepseek-r1-distill-llama-70b",
  //   name: "DeepSeek R1 (Llama Distill)",
  //   created: 1731196800,
  //   context_length: 128000,
  //   plan: "free",
  //   "pricing": {
  //     "prompt": "0.0000025",
  //     "completion": "0.00001",
  //     "image": "0.003613",
  //     "request": "0",
  //     "input_cache_read": "0",
  //     "input_cache_write": "0",
  //     "web_search": "0",
  //     "internal_reasoning": "0"
  //   },
  // },
  {
    provider: "openrouter",
    "id": "anthropic/claude-3.5-sonnet",
    "name": "Claude 3.5 Sonnet",
    "created": 1729555200,
    "description": "New Claude 3.5 Sonnet delivers better-than-Opus capabilities, faster-than-Sonnet speeds, at the same Sonnet prices. Sonnet is particularly good at:\n\n- Coding: Scores ~49% on SWE-Bench Verified, higher than the last best score, and without any fancy prompt scaffolding\n- Data science: Augments human data science expertise; navigates unstructured data while using multiple tools for insights\n- Visual processing: excelling at interpreting charts, graphs, and images, accurately transcribing text to derive insights beyond just the text alone\n- Agentic tasks: exceptional tool use, making it great at agentic tasks (i.e. complex, multi-step problem solving tasks that require engaging with other systems)\n\n#multimodal",
    "shortDescription": "Advanced model with better-than-Opus capabilities and faster performance, excelling at coding, data science, visual processing, and agentic tasks.",
    "context_length": 200000,
    plan: "pro",
    supportsTools: true,
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "Claude",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.000003",
      "completion": "0.000015",
      "image": "0.0048",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 200000,
      "max_completion_tokens": 8192,
      "is_moderated": true
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "anthropic/claude-3.7-sonnet",
    "name": "Claude 3.7 Sonnet",
    "created": 1740422110,
    "description": "Claude 3.7 Sonnet is an advanced large language model with improved reasoning, coding, and problem-solving capabilities. It introduces a hybrid reasoning approach, allowing users to choose between rapid responses and extended, step-by-step processing for complex tasks. The model demonstrates notable improvements in coding, particularly in front-end development and full-stack updates, and excels in agentic workflows, where it can autonomously navigate multi-step processes. \n\nClaude 3.7 Sonnet maintains performance parity with its predecessor in standard mode while offering an extended reasoning mode for enhanced accuracy in math, coding, and instruction-following tasks.\n\nRead more at the [blog post here](https://www.anthropic.com/news/claude-3-7-sonnet)",
    "shortDescription": "Advanced model with hybrid reasoning that offers both rapid responses and extended step-by-step processing for complex tasks.",
    "context_length": 200000,
    plan: "pro",
    supportsTools: true,
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "Claude",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.000003",
      "completion": "0.000015",
      "image": "0.0048",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 200000,
      "max_completion_tokens": 128000,
      "is_moderated": true
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "google/gemini-2.0-flash-001",
    "name": "Gemini Flash 2.0",
    "created": 1738769413,
    "description": "Gemini Flash 2.0 offers a significantly faster time to first token (TTFT) compared to [Gemini Flash 1.5](/google/gemini-flash-1.5), while maintaining quality on par with larger models like [Gemini Pro 1.5](/google/gemini-pro-1.5). It introduces notable enhancements in multimodal understanding, coding capabilities, complex instruction following, and function calling. These advancements come together to deliver more seamless and robust agentic experiences.",
    "shortDescription": "Fast-responding model with high-quality output and enhanced multimodal understanding, coding capabilities, and function calling.",
    "context_length": 1000000,
    plan: "pro",
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "Gemini",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.0000001",
      "completion": "0.0000004",
      "image": "0.0000258",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 1000000,
      "max_completion_tokens": 8192,
      "is_moderated": false
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "openai/gpt-4o-mini",
    "name": "GPT-4o-mini",
    "created": 1721260800,
    "description": "GPT-4o mini is OpenAI's newest model after [GPT-4 Omni](/models/openai/gpt-4o), supporting both text and image inputs with text outputs.\n\nAs their most advanced small model, it is many multiples more affordable than other recent frontier models, and more than 60% cheaper than [GPT-3.5 Turbo](/models/openai/gpt-3.5-turbo). It maintains SOTA intelligence, while being significantly more cost-effective.\n\nGPT-4o mini achieves an 82% score on MMLU and presently ranks higher than GPT-4 on chat preferences [common leaderboards](https://arena.lmsys.org/).\n\nCheck out the [launch announcement](https://openai.com/index/gpt-4o-mini-advancing-cost-efficient-intelligence/) to learn more.\n\n#multimodal",
    "shortDescription": "OpenAI's cost-effective multimodal model that maintains high intelligence while being significantly cheaper than larger models.",
    "context_length": 128000,
    plan: "pro",
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "GPT",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.00000015",
      "completion": "0.0000006",
      "image": "0.000217",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 128000,
      "max_completion_tokens": 16384,
      "is_moderated": true
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "openai/gpt-4o-2024-11-20",
    "name": "GPT-4o",
    "created": 1732127594,
    "description": "The 2024-11-20 version of GPT-4o offers a leveled-up creative writing ability with more natural, engaging, and tailored writing to improve relevance & readability. It's also better at working with uploaded files, providing deeper insights & more thorough responses.\n\nGPT-4o (\"o\" for \"omni\") is OpenAI's latest AI model, supporting both text and image inputs with text outputs. It maintains the intelligence level of [GPT-4 Turbo](/models/openai/gpt-4-turbo) while being twice as fast and 50% more cost-effective. GPT-4o also offers improved performance in processing non-English languages and enhanced visual capabilities.",
    "shortDescription": "Latest GPT-4o version with enhanced creative writing and improved file handling, combining high intelligence with faster processing.",
    "context_length": 128000,
    plan: "pro",
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "GPT",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.0000025",
      "completion": "0.00001",
      "image": "0.003613",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 128000,
      "max_completion_tokens": 16384,
      "is_moderated": true
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "openai/gpt-4.5-preview",
    "name": "GPT-4.5 (Preview)",
    "created": 1740687810,
    "description": "GPT-4.5 (Preview) is a research preview of OpenAI's latest language model, designed to advance capabilities in reasoning, creativity, and multi-turn conversation. It builds on previous iterations with improvements in world knowledge, contextual coherence, and the ability to follow user intent more effectively.\n\nThe model demonstrates enhanced performance in tasks that require open-ended thinking, problem-solving, and communication. Early testing suggests it is better at generating nuanced responses, maintaining long-context coherence, and reducing hallucinations compared to earlier versions.\n\nThis research preview is intended to help evaluate GPT-4.5's strengths and limitations in real-world use cases as OpenAI continues to refine and develop future models. Read more at the [blog post here.](https://openai.com/index/introducing-gpt-4-5/)",
    "shortDescription": "OpenAI's latest research preview model with advanced reasoning capabilities and improved context handling, designed for complex problem-solving.",
    "context_length": 128000,
    plan: "pro",
    "architecture": {
      "modality": "text+image->text",
      "tokenizer": "GPT",
      "instruct_type": null
    },
    "pricing": {
      "prompt": "0.000075",
      "completion": "0.00015",
      "image": "0.108375",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 128000,
      "max_completion_tokens": 16384,
      "is_moderated": true
    },
    "per_request_limits": null
  },
  {
    provider: "openrouter",
    "id": "mistralai/mistral-nemo",
    "name": "Mistral Nemo",
    "created": 1721347200,
    "description": "A 12B parameter model with a 128k token context length built by Mistral in collaboration with NVIDIA.\n\nThe model is multilingual, supporting English, French, German, Spanish, Italian, Portuguese, Chinese, Japanese, Korean, Arabic, and Hindi.\n\nIt supports function calling and is released under the Apache 2.0 license.",
    "shortDescription": "12B multilingual model with 128k context window, supporting 11 languages and function calling capabilities.",
    "context_length": 128000,
    plan: "pro",
    "architecture": {
      "modality": "text->text",
      "tokenizer": "Mistral",
      "instruct_type": "mistral"
    },
    "pricing": {
      "prompt": "0.000000035",
      "completion": "0.00000008",
      "image": "0",
      "request": "0",
      "input_cache_read": "0",
      "input_cache_write": "0",
      "web_search": "0",
      "internal_reasoning": "0"
    },
    "top_provider": {
      "context_length": 131072,
      "max_completion_tokens": 8192,
      "is_moderated": false
    },
    "per_request_limits": null
  }
]
