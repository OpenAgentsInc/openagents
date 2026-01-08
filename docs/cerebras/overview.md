# Build with the Speed of Cerebras

> Experience real-time AI responses for code generation, summarization, and autonomous tasks with the world’s fastest AI inference.

<Tip>Get a free API key by visiting our [API playground](https://cloud.cerebras.ai?utm_source=3pi_introduction\&utm_campaign=docs).</Tip>

<CodeGroup>
  ```python Python theme={null}
  import os
  from cerebras.cloud.sdk import Cerebras

  client = Cerebras(
      api_key=os.environ.get("CEREBRAS_API_KEY"),
  )

  chat_completion = client.chat.completions.create(
      messages=[
          {
              "role": "user",
              "content": "Why is fast inference important?",
          }
  ],
      model="llama-3.3-70b",
  )

  print(chat_completion)
  ```

  ```javascript Node.js theme={null}
  import Cerebras from '@cerebras/cerebras_cloud_sdk';

  const client = new Cerebras({
    apiKey: process.env['CEREBRAS_API_KEY'],
  });

  async function main() {
    const completionCreateResponse = await client.chat.completions.create({
      messages: [{ role: 'user', content: 'Why is fast inference important?' }],
      model: 'llama-3.3-70b',
    });

    console.log(completionCreateResponse);
  }

  main();
  ```

  ```bash cURL theme={null}
  curl --location 'https://api.cerebras.ai/v1/chat/completions' \
  --header 'Content-Type: application/json' \
  --header "Authorization: Bearer ${CEREBRAS_API_KEY}" \
  --data '{
    "model": "llama-3.3-70b",
    "stream": false,
    "messages": [{"content": "why is fast inference important?", "role": "user"}],
    "temperature": 0,
    "max_tokens": -1,
    "seed": 0,
    "top_p": 1
  }'
  ```
</CodeGroup>

<Card title="Start Building" icon="rocket" href="/quickstart" arrow="true" horizontal>
  Follow our QuickStart guide to build your first application
</Card>

<Columns cols={2}>
  <Card title="Explore Models" icon="magnifying-glass" href="/models/overview" arrow="true">
    View our available models, including performance specifications, rate limits, and pricing details.
  </Card>

  <Card title="Get Familiar" icon="sparkles">
    * Try our [live chatbot demo](https://inference.cerebras.ai).
    * Learn more about [pricing](/support/pricing).
    * Experiment with our inference solution in the [playground](https://cloud.cerebras.ai?utm_source=inferencedocs) before making an API call.
    * Explore our [API reference](https://inference-docs.cerebras.ai/api-reference/chat-completions) documentation.
  </Card>
</Columns>

<Tooltip tip="We designed the Cerebras API to be mostly compatible with OpenAI’s client libraries." cta="Read our Compatibility guide" href="/resources/openai">OpenAI base URL:</Tooltip> `https://api.cerebras.ai/v1`


---

> To find navigation and other pages in this documentation, fetch the llms.txt file at: https://inference-docs.cerebras.ai/llms.txt
