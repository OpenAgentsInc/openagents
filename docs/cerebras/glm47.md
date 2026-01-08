# Z.ai GLM 4.7

> This model delivers strong coding performance with advanced reasoning capabilities, superior tool use, and enhanced real-world performance in agentic coding applications.

export const ModelInfo = ({modelId, modelCardUrl, contextLength = {}, maxOutput = {}, speed, inputOutput = {}, pricing = {}, rateLimits = [], endpoints = [], features = [], knownLimitations = []}) => {
  return <div className="space-y-6 not-prose">
      {modelId && <div className="flex items-center justify-between">
          <div>
            <span className="text-sm font-semibold text-zinc-700 dark:text-zinc-300">Model ID: </span>
            <code className="text-sm font-mono font-semibold text-zinc-900 dark:text-white">{modelId}</code>
          </div>
          {modelCardUrl && <a href={modelCardUrl} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-sm text-zinc-700 dark:text-zinc-300 hover:text-zinc-900 dark:hover:text-white transition-colors">
              Model card
              <Icon icon="external-link" size={14} color="currentColor" />
            </a>}
        </div>}

      <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
        <div className="flex items-center gap-3 mb-6">
          <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
            <Icon icon="chart-bar" size={18} color="#fb923c" />
          </div>
          <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Model Stats</h3>
        </div>

        <div className="grid grid-cols-2 md:grid-cols-4 gap-6">
          <div className="text-center">
            <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-3 uppercase">
              SPEED
            </div>
            <div className="text-2xl font-bold text-orange-500 dark:text-orange-400">
              {speed?.value}
            </div>
            <div className="text-sm text-zinc-600 dark:text-zinc-400 mt-1">
              {speed?.unit}
            </div>
          </div>

          <div className="text-center">
            <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-3 uppercase">
              INPUT / OUTPUT
            </div>
            <div className="flex items-center justify-center gap-2 h-12">
              <div className="flex items-center gap-1">
                {inputOutput.inputFormats && inputOutput.inputFormats.map((format, index) => <Icon key={index} icon={format === 'image' ? 'image' : format === 'audio' ? 'headphones' : format === 'video' ? 'video' : 'text'} size={18} color="#fb923c" />)}
              </div>
              <span className="text-zinc-400 text-lg">/</span>
              <div className="flex items-center gap-1">
                {inputOutput.outputFormats && inputOutput.outputFormats.map((format, index) => <Icon key={index} icon={format === 'image' ? 'image' : format === 'audio' ? 'headphones' : format === 'video' ? 'video' : 'text'} size={18} color="#fb923c" />)}
              </div>
            </div>
          </div>

          <div className="text-center">
            <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-3 uppercase">
              CONTEXT
            </div>
            <div className="space-y-2">
              <div>
                <div className="text-xs text-zinc-600 dark:text-zinc-400">Free Tier</div>
                <div className="text-sm font-medium text-zinc-900 dark:text-white">
                  {contextLength.freeTier}
                </div>
              </div>
              <div>
                <div className="text-xs text-zinc-600 dark:text-zinc-400">Paid Tiers</div>
                <div className="text-sm font-medium text-zinc-900 dark:text-white">
                  {contextLength.paidTiers}
                </div>
              </div>
            </div>
          </div>

          <div className="text-center">
            <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-3 uppercase">
              MAX OUTPUT
            </div>
            <div className="space-y-2">
              <div>
                <div className="text-xs text-zinc-600 dark:text-zinc-400">Free Tier</div>
                <div className="text-sm font-medium text-zinc-900 dark:text-white">
                  {maxOutput.freeTier || 'N/A'}
                </div>
              </div>
              <div>
                <div className="text-xs text-zinc-600 dark:text-zinc-400">Paid Tiers</div>
                <div className="text-sm font-medium text-zinc-900 dark:text-white">
                  {maxOutput.paidTiers || 'N/A'}
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {pricing.inputPrice && pricing.outputPrice && <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
              <Icon icon="dollar-sign" size={18} color="#fb923c" />
            </div>
            <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Pricing</h3>
          </div>
          <div className="space-y-4">
            <div className="flex">
              <div className="flex-1 text-center">
                <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-2 uppercase">Input</div>
                <div className="text-zinc-900 dark:text-white text-2xl font-bold">{pricing.inputPrice}</div>
              </div>
              <div className="w-px bg-zinc-200 dark:bg-zinc-800 mx-6"></div>
              <div className="flex-1 text-center">
                <div className="text-xs font-mono font-semibold tracking-wider text-zinc-500 dark:text-zinc-400 mb-2 uppercase">Output</div>
                <div className="text-zinc-900 dark:text-white text-2xl font-bold">{pricing.outputPrice}</div>
              </div>
            </div>
            <div className="pt-4">
              <p className="text-sm text-zinc-600 dark:text-zinc-400">
                Exploration pricing shown above is per million tokens. For volume discounts and enterprise features, see our{' '}
                <a href="https://www.cerebras.ai/pricing" className="text-black font-semibold underline decoration-orange-500 underline-offset-4 decoration-1 hover:decoration-2">
                  pricing page
                </a>
                .
              </p>
            </div>
          </div>
        </div>}

      {knownLimitations && knownLimitations.length > 0 && <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
              <Icon icon="note" size={18} color="#fb923c" />
            </div>
            <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Model Notes</h3>
          </div>
          <div className="space-y-2">
            {knownLimitations.map((limitation, index) => <div key={index} className="flex items-start gap-2 py-2">
                <div className="w-1.5 h-1.5 bg-zinc-400 dark:bg-zinc-500 rounded-full flex-shrink-0 mt-2"></div>
                <div className="text-zinc-900 dark:text-white text-sm leading-relaxed prose-sm max-w-none">
                  {limitation}
                </div>
              </div>)}
          </div>
        </div>}

      {rateLimits && rateLimits.length > 0 && <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
              <Icon icon="clock" size={18} color="#fb923c" />
            </div>
            <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Rate Limits</h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-zinc-200 dark:border-zinc-800">
                  <th className="text-left py-3 px-2 text-zinc-500 dark:text-zinc-400 font-medium">Tier</th>
                  <th className="text-left py-3 px-2 text-zinc-500 dark:text-zinc-400 font-medium">Requests/min</th>
                  <th className="text-left py-3 px-2 text-zinc-500 dark:text-zinc-400 font-medium">Input Tokens/min</th>
                  <th className="text-left py-3 px-2 text-zinc-500 dark:text-zinc-400 font-medium">Daily Tokens</th>
                </tr>
              </thead>
              <tbody>
                {rateLimits.map((limit, index) => <tr key={index} className="border-b border-zinc-200/50 dark:border-zinc-800/50 last:border-b-0">
                    <td className="py-3 px-2 text-zinc-900 dark:text-white font-medium">{limit.tier}</td>
                    <td className="py-3 px-2 text-zinc-900 dark:text-white">{limit.requestsPerMin}</td>
                    <td className="py-3 px-2 text-zinc-900 dark:text-white">{limit.inputTokensPerMin}</td>
                    <td className="py-3 px-2 text-zinc-900 dark:text-white">{limit.dailyTokens}</td>
                  </tr>)}
              </tbody>
            </table>
          </div>
        </div>}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {endpoints && endpoints.length > 0 && <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
                <Icon icon="link" size={18} color="#fb923c" />
              </div>
              <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Endpoints</h3>
            </div>
            <div className="space-y-3">
              {endpoints.map((endpoint, index) => {
    const endpointUrls = {
      'Chat Completions': '/v1/chat/completions',
      'Completions': '/v1/completions',
      'Models': '/v1/models'
    };
    const endpointName = typeof endpoint === 'string' ? endpoint : endpoint.name;
    const endpointUrl = typeof endpoint === 'object' && endpoint.url ? endpoint.url : endpointUrls[endpointName];
    return <div key={index} className="flex items-start gap-2 py-2">
                    <span className="text-zinc-400 dark:text-zinc-500 text-sm">→</span>
                    <div>
                      <div className="text-sm text-zinc-900 dark:text-white">
                        {endpointName}
                      </div>
                      {endpointUrl && <code className="text-xs text-zinc-600 dark:text-zinc-400 font-mono">
                          {endpointUrl}
                        </code>}
                    </div>
                  </div>;
  })}
            </div>
          </div>}

        {features && features.length > 0 && <div className="bg-white dark:bg-zinc-950 border border-zinc-200 dark:border-zinc-800 rounded-xl p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
                <Icon icon="sparkles" size={18} color="#fb923c" />
              </div>
              <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Capabilities</h3>
            </div>
            <div className="space-y-2">
              {features.map((feature, index) => <div key={index} className="flex items-center gap-2 py-2">
                  <span className="text-green-500 dark:text-green-400 text-sm">✓</span>
                  <span className="text-zinc-900 dark:text-white text-sm">{feature}</span>
                </div>)}
            </div>
          </div>}
      </div>

      <div className="bg-gradient-to-r from-orange-600/10 to-red-500/10 border border-orange-600/20 rounded-xl p-6">
        <div className="flex items-center gap-3 mb-3">
          <div className="w-8 h-8 bg-orange-600/20 rounded-lg flex items-center justify-center">
            <Icon icon="rocket" size={18} color="#fb923c" />
          </div>
          <h3 className="text-lg font-semibold text-zinc-900 dark:text-white">Need Higher Limits?</h3>
        </div>
        <p className="text-zinc-700 dark:text-zinc-300">
          Reach out for custom pricing with our Enterprise tier for higher rate limits and dedicated support.
        </p>
        <div className="mt-4">
          <a href="https://cerebras.ai/contact-us" className="inline-flex items-center gap-2 bg-orange-600 hover:bg-orange-700 text-white px-4 py-2 rounded-lg transition-colors font-medium">
            Contact Sales
            <Icon icon="arrow-right" color="white" size={16} />
          </a>
        </div>
      </div>
    </div>;
};

<Tip>
  Migrating from another model? Check out our [GLM 4.7 Migration Guide](/resources/glm-47-migration) for prompt optimization tips and best practices.
</Tip>

<ModelInfo
  modelId="zai-glm-4.7"
  modelCardUrl="https://huggingface.co/zai-org/GLM-4.7"
  contextLength={{
  freeTier: "64k tokens",
  paidTiers: "131k tokens"
}}
  maxOutput={{
  freeTier: "40k tokens",
  paidTiers: "40k tokens"
}}
  speed={{
  value: "~1000",
  unit: "tokens/sec"
}}
  rateLimits={[
  {
    tier: "Free",
    requestsPerMin: "10",
    inputTokensPerMin: "150k",
    dailyTokens: "1M"
  },
  {
    tier: "Developer",
    requestsPerMin: "250",
    inputTokensPerMin: "250k",
    dailyTokens: "N/A"
  }
]}
  pricing={{
  inputPrice: "$2.25 / M tokens",
  outputPrice: "$2.75 / M tokens"
}}
  endpoints={[
  "Chat Completions"
]}
  features={[
  "Reasoning",
  "Streaming",
  "Structured Outputs",
  "Tool Calling",
  "Parallel Tool Calling"
]}
  inputOutput={{
  inputFormats: ["text"],
  outputFormats: ["text"]
}}
  knownLimitations={[
  <span>
  Reasoning is enabled by default for this model. To disable it, see the <a href="/capabilities/reasoning#reasoning-with-z-ai-glm" className="font-semibold text-black underline underline-offset-4 decoration-2 hover:text-orange-500 hover:decoration-4" style={{ textDecorationColor: '#f97316' }}>reasoning guide</a>.
  </span>,
  <span>
  Structured outputs and tool calling with <code>strict: true</code> (constrained decoding) is supported for this model.
  </span>,
  <span>
  Use the <code>clear_thinking</code> parameter to control whether thinking content from previous turns is included in the prompt context. Defaults to <code>true</code> (exclude previous thinking). Set to <code>false</code> for agentic workflows where past reasoning may inform future tool calls. See the <a href="/api-reference/chat-completions#param-clear-thinking" className="font-semibold text-black underline underline-offset-4 decoration-2 hover:text-orange-500 hover:decoration-4" style={{ textDecorationColor: '#f97316' }}>API reference</a> for more details.
  </span>
]}
/>


---

> To find navigation and other pages in this documentation, fetch the llms.txt file at: https://inference-docs.cerebras.ai/llms.txt
