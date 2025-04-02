interface Model {
  author: "anthropic" | "cohere" | "deepseek" | "openai" | "google" | "meta" | "mistralai" | "qwen"
  created: number
  description: string
  id: string
  name: string
  provider: "openrouter" | "anthropic" | "ollama" | "lmstudio"
  shortDescription?: string
  supportsTools?: boolean
  tokenizer?: string
  pricing?: {
    prompt: string
    completion: string
    image: string
    request: string
    input_cache_read: string
    input_cache_write: string
    web_search: string
    internal_reasoning: string
  }
  top_provider?: {
    context_length: number
    max_completion_tokens: number
    is_moderated: boolean
  }
  architecture?: {
    modality: "text->text" | "text+image->text"
    tokenizer: string
    instruct_type: string | null
  }
  per_request_limits?: null
  context_length: number
}

export const MODELS: Model[] = [
  {
    author: 'qwen',
    provider: 'lmstudio',
    id: 'qwen2.5-14b-instruct',
    context_length: 131000,
    created: 1721088000,
    description: "Qwen2.5 14B Instruct is a powerful instruction-tuned language model with 14.7B parameters. It excels in coding, mathematics, instruction following, and long-text generation. The model supports 29+ languages including Chinese, English, French, Spanish, Portuguese, German, Italian, Russian, Japanese, Korean, and more. It features a 131K token context length with 8K token generation capability, and uses advanced architecture components like RoPE, SwiGLU, RMSNorm, and GQA attention.",
    name: "Qwen 2.5 14B Instruct",
    shortDescription: "14.7B parameter multilingual model with 131K context window, optimized for coding, math, and structured outputs across 29+ languages.",
    supportsTools: true,
  },
  {
    author: "qwen",
    provider: "lmstudio",
    id: "qwen2-7b-instruct",
    context_length: 8192,
    created: 1721088000,
    description: "Qwen2 7B is a transformer-based model that excels in language understanding, multilingual capabilities, coding, mathematics, and reasoning.",
    name: "Qwen 2 7B Instruct",
    shortDescription: "Qwen 2 7B Instruct is a large language model with 7B parameters.",
    supportsTools: true,
  },
  {
    author: "meta",
    provider: "ollama",
    id: "llama3.2",
    name: "Llama 3.2",
    created: 1742824755,
    description: "Llama 3.2 is a large language model with 12B parameters.",
    shortDescription: "Llama 3.2 is a large language model with 12B parameters.",
    context_length: 20000,
    supportsTools: true,
  },
  {
    author: "cohere",
    provider: "ollama",
    id: "command-r7b",
    name: "Command R7B",
    created: 1742824755,
    description: "The smallest model in Cohere's R series delivers top-tier speed, efficiency, and quality to build powerful AI applications on commodity GPUs and edge devices.",
    shortDescription: "Smallest model in Cohere's R series, delivering top-tier speed, efficiency, and quality.",
    context_length: 128000,
    supportsTools: true,
  },
  {
    author: "google",
    provider: "ollama",
    id: "gemma3",
    name: "Gemma 3 4B",
    created: 1742824755,
    description: "Gemma 3 4B is a large language model with 4B parameters.",
    shortDescription: "Gemma 3 4B is a large language model with 4B parameters.",
    context_length: 128000,
    supportsTools: false,
  },
  {
    author: "google",
    provider: "ollama",
    id: "gemma3:12b",
    name: "Gemma 3 12B",
    created: 1742824755,
    description: "Gemma 3 12B is a large language model with 12B parameters.",
    shortDescription: "Gemma 3 12B is a large language model with 12B parameters.",
    context_length: 128000,
    supportsTools: false,
  },
  {
    author: "deepseek",
    provider: "openrouter",
    id: "deepseek/deepseek-chat-v3-0324:free",
    name: "DeepSeek V3 0324 (free)",
    created: 1742824755,
    description: "DeepSeek V3, a 685B-parameter, mixture-of-experts model, is the latest iteration of the flagship chat model family from the DeepSeek team.",
    shortDescription: "Latest DeepSeek model featuring 685B parameters with mixture-of-experts architecture.",
    context_length: 64000,
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

  {
    author: "deepseek",
    provider: "openrouter",
    id: "deepseek/deepseek-chat-v3-0324",
    name: "DeepSeek V3 0324",
    created: 1742824755,
    description: "DeepSeek V3, a 685B-parameter, mixture-of-experts model, is the latest iteration of the flagship chat model family from the DeepSeek team.",
    shortDescription: "Latest DeepSeek model featuring 685B parameters with mixture-of-experts architecture.",
    context_length: 64000,
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
    author: "deepseek",
    provider: "openrouter",
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
    author: "anthropic",
    provider: "openrouter",
    "id": "anthropic/claude-3.5-sonnet",
    "name": "Claude 3.5 Sonnet",
    "created": 1729555200,
    "description": "New Claude 3.5 Sonnet delivers better-than-Opus capabilities, faster-than-Sonnet speeds, at the same Sonnet prices. Sonnet is particularly good at:\n\n- Coding: Scores ~49% on SWE-Bench Verified, higher than the last best score, and without any fancy prompt scaffolding\n- Data science: Augments human data science expertise; navigates unstructured data while using multiple tools for insights\n- Visual processing: excelling at interpreting charts, graphs, and images, accurately transcribing text to derive insights beyond just the text alone\n- Agentic tasks: exceptional tool use, making it great at agentic tasks (i.e. complex, multi-step problem solving tasks that require engaging with other systems)\n\n#multimodal",
    "shortDescription": "Advanced model with better-than-Opus capabilities and faster performance, excelling at coding, data science, visual processing, and agentic tasks.",
    "context_length": 200000,
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
    author: "anthropic",
    provider: "openrouter",
    "id": "anthropic/claude-3.7-sonnet",
    "name": "Claude 3.7 Sonnet",
    "created": 1740422110,
    "description": "Claude 3.7 Sonnet is an advanced large language model with improved reasoning, coding, and problem-solving capabilities. It introduces a hybrid reasoning approach, allowing users to choose between rapid responses and extended, step-by-step processing for complex tasks. The model demonstrates notable improvements in coding, particularly in front-end development and full-stack updates, and excels in agentic workflows, where it can autonomously navigate multi-step processes. \n\nClaude 3.7 Sonnet maintains performance parity with its predecessor in standard mode while offering an extended reasoning mode for enhanced accuracy in math, coding, and instruction-following tasks.\n\nRead more at the [blog post here](https://www.anthropic.com/news/claude-3-7-sonnet)",
    "shortDescription": "Advanced model with hybrid reasoning that offers both rapid responses and extended step-by-step processing for complex tasks.",
    "context_length": 200000,
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
    author: "google",
    provider: "openrouter",
    "id": "google/gemini-2.0-flash-001",
    "name": "Gemini Flash 2.0",
    "created": 1738769413,
    "description": "Gemini Flash 2.0 offers a significantly faster time to first token (TTFT) compared to [Gemini Flash 1.5](/google/gemini-flash-1.5), while maintaining quality on par with larger models like [Gemini Pro 1.5](/google/gemini-pro-1.5). It introduces notable enhancements in multimodal understanding, coding capabilities, complex instruction following, and function calling. These advancements come together to deliver more seamless and robust agentic experiences.",
    "shortDescription": "Fast-responding model with high-quality output and enhanced multimodal understanding, coding capabilities, and function calling.",
    "context_length": 1000000,
    supportsTools: true,
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
    author: "openai",
    provider: "openrouter",
    "id": "openai/gpt-4o-mini",
    "name": "GPT-4o-mini",
    "created": 1721260800,
    "description": "GPT-4o mini is OpenAI's newest model after [GPT-4 Omni](/models/openai/gpt-4o), supporting both text and image inputs with text outputs.\n\nAs their most advanced small model, it is many multiples more affordable than other recent frontier models, and more than 60% cheaper than [GPT-3.5 Turbo](/models/openai/gpt-3.5-turbo). It maintains SOTA intelligence, while being significantly more cost-effective.\n\nGPT-4o mini achieves an 82% score on MMLU and presently ranks higher than GPT-4 on chat preferences [common leaderboards](https://arena.lmsys.org/).\n\nCheck out the [launch announcement](https://openai.com/index/gpt-4o-mini-advancing-cost-efficient-intelligence/) to learn more.\n\n#multimodal",
    "shortDescription": "OpenAI's cost-effective multimodal model that maintains high intelligence while being significantly cheaper than larger models.",
    "context_length": 128000,
    supportsTools: true,
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
    author: "openai",
    provider: "openrouter",
    "id": "openai/gpt-4o-2024-11-20",
    "name": "GPT-4o",
    "created": 1732127594,
    "description": "The 2024-11-20 version of GPT-4o offers a leveled-up creative writing ability with more natural, engaging, and tailored writing to improve relevance & readability. It's also better at working with uploaded files, providing deeper insights & more thorough responses.\n\nGPT-4o (\"o\" for \"omni\") is OpenAI's latest AI model, supporting both text and image inputs with text outputs. It maintains the intelligence level of [GPT-4 Turbo](/models/openai/gpt-4-turbo) while being twice as fast and 50% more cost-effective. GPT-4o also offers improved performance in processing non-English languages and enhanced visual capabilities.",
    "shortDescription": "Latest GPT-4o version with enhanced creative writing and improved file handling, combining high intelligence with faster processing.",
    "context_length": 128000,
    supportsTools: true,
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
    author: "openai",
    provider: "openrouter",
    "id": "openai/gpt-4.5-preview",
    "name": "GPT-4.5 (Preview)",
    "created": 1740687810,
    "description": "GPT-4.5 (Preview) is a research preview of OpenAI's latest language model, designed to advance capabilities in reasoning, creativity, and multi-turn conversation. It builds on previous iterations with improvements in world knowledge, contextual coherence, and the ability to follow user intent more effectively.\n\nThe model demonstrates enhanced performance in tasks that require open-ended thinking, problem-solving, and communication. Early testing suggests it is better at generating nuanced responses, maintaining long-context coherence, and reducing hallucinations compared to earlier versions.\n\nThis research preview is intended to help evaluate GPT-4.5's strengths and limitations in real-world use cases as OpenAI continues to refine and develop future models. Read more at the [blog post here.](https://openai.com/index/introducing-gpt-4-5/)",
    "shortDescription": "OpenAI's latest research preview model with advanced reasoning capabilities and improved context handling, designed for complex problem-solving.",
    "context_length": 128000,
    supportsTools: true,
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
    author: "mistralai",
    provider: "openrouter",
    "id": "mistralai/mistral-nemo",
    "name": "Mistral Nemo",
    "created": 1721347200,
    "description": "A 12B parameter model with a 128k token context length built by Mistral in collaboration with NVIDIA.\n\nThe model is multilingual, supporting English, French, German, Spanish, Italian, Portuguese, Chinese, Japanese, Korean, Arabic, and Hindi.\n\nIt supports function calling and is released under the Apache 2.0 license.",
    "shortDescription": "12B multilingual model with 128k context window, supporting 11 languages and function calling capabilities.",
    "context_length": 128000,
    supportsTools: true,
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
