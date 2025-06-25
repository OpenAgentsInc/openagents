# Convex vector search scales to 100k embeddings with built-in similarity features

Convex provides **native vector search capabilities** with cosine similarity, supporting up to 4,096 dimensions and optimized for datasets under 100,000 vectors. The platform integrates vector operations directly into its reactive database architecture, enabling immediate consistency and real-time updates across connected clients. For applications requiring vector search alongside traditional database operations, Convex eliminates the need for separate infrastructure while maintaining sub-50ms query performance for typical workloads.

## Native vector support delivers integrated search without external dependencies

Convex implements vector search through dedicated vector indexes defined in the schema using TypeScript. The `vectorIndex()` method creates specialized indexes that support approximate nearest neighbor (ANN) search with cosine similarity. Unlike traditional databases that bolt on vector capabilities, Convex designed these features specifically for AI applications, with **80% of Convex applications leveraging AI functionality**.

The vector implementation uses **HNSW-based indexing** similar to dedicated vector databases, storing embeddings as `v.array(v.float64())` fields directly within documents. This approach enables developers to store vectors alongside metadata, user data, and application state in a single system. Vector searches execute only within Convex Actions, returning results with similarity scores ranging from -1 to 1, where higher values indicate greater similarity.

Key specifications include support for 2-4,096 dimensional vectors, up to 256 results per query, and 16 filter fields per index. The system maintains **immediate consistency** - newly written vectors are instantly searchable without the eventual consistency delays common in distributed vector databases.

## Schema design patterns optimize for different scale and access patterns

For applications under 50,000 vectors, a single-table approach provides the simplest architecture. Documents, metadata, and embeddings coexist in one table with a vector index:

```typescript
export default defineSchema({
  documents: defineTable({
    title: v.string(),
    content: v.string(),
    category: v.string(),
    embedding: v.array(v.float64()),
  }).vectorIndex("by_embedding", {
    vectorField: "embedding",
    dimensions: 1536, // OpenAI ada-002 dimensions
    filterFields: ["category"],
  }),
});
```

Larger applications benefit from **separating embeddings into dedicated tables**, reducing memory overhead when loading non-vector data. This pattern supports efficient batch processing and enables independent scaling of vector and metadata storage:

```typescript
export default defineSchema({
  documents: defineTable({
    url: v.string(),
    text: v.string(),
  }).index("byUrl", ["url"]),

  embeddings: defineTable({
    embedding: v.array(v.number()),
    documentId: v.id("documents"),
  }).vectorIndex("byEmbedding", {
    vectorField: "embedding",
    dimensions: 1536,
  }),
});
```

Message and chat applications require additional structure to handle conversation context and user interactions. The recommended pattern uses three tables: messages for conversation history, chunks for text segments, and embeddings for vector storage. This design enables efficient retrieval of conversation context while maintaining clean separation between user-facing data and vector operations.

## JavaScript implementation leverages Actions for external API integration

Vector search in Convex requires Actions rather than Queries or Mutations, as Actions support external API calls necessary for embedding generation. A typical implementation combines OpenAI's embedding API with Convex's vector search:

```typescript
export const semanticSearch = action({
  args: { query: v.string() },
  handler: async (ctx, args) => {
    // Generate embedding
    const openai = new OpenAI();
    const response = await openai.embeddings.create({
      input: args.query,
      model: "text-embedding-ada-002",
    });
    const embedding = response.data[0].embedding;

    // Search vectors
    const results = await ctx.vectorSearch("documents", "by_embedding", {
      vector: embedding,
      limit: 10,
      filter: (q) => q.eq("category", "technical"),
    });

    // Load full documents
    return await ctx.runQuery(internal.documents.fetchResults, {
      ids: results.map(r => r._id)
    });
  },
});
```

For custom similarity calculations beyond Convex's built-in cosine similarity, JavaScript implementations of distance metrics work within Actions. However, these manual calculations lack the performance benefits of indexed vector search and should only supplement the native functionality for special cases.

**LangChain integration** provides higher-level abstractions through the `ConvexVectorStore` class, simplifying RAG (Retrieval-Augmented Generation) implementations. This integration handles embedding generation, storage, and retrieval patterns automatically while maintaining compatibility with LangChain's broader ecosystem.

## Embedding generation follows Actions-based patterns with built-in resilience

Integration with OpenAI and other embedding services follows a consistent pattern using Convex Actions. The platform's Action system provides natural boundaries for external API calls, error handling, and retry logic. **Batch processing** significantly improves throughput for large datasets:

```typescript
export const embedBatch = action({
  args: { texts: v.array(v.string()) },
  handler: async (ctx, args) => {
    const result = await fetch("https://api.openai.com/v1/embeddings", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${process.env.OPENAI_API_KEY}`,
      },
      body: JSON.stringify({
        model: "text-embedding-ada-002",
        input: texts.map(text => text.replace(/\n/g, " ")),
      }),
    });

    const data = await result.json();
    const embeddings = data.data.sort((a, b) => a.index - b.index);

    // Store embeddings with proper error handling
    await ctx.runMutation(internal.embeddings.storeBatch, {
      embeddings: embeddings.map((item, i) => ({
        text: texts[i],
        embedding: item.embedding,
      })),
    });
  },
});
```

**Caching strategies** reduce API costs and latency. The Convex Action Cache component provides TTL-based caching with automatic invalidation. For frequently accessed content, database-level caching using content hashes as keys prevents redundant embedding generation while maintaining flexibility for updates.

Local embedding models integrate through HTTP services, enabling privacy-preserving deployments. A Python service running sentence-transformers or similar models receives requests from Convex Actions, returning embeddings without external API dependencies. This hybrid approach allows dynamic selection between cloud and local models based on data sensitivity or cost considerations.

## Performance optimization requires architectural decisions at 50-100k vectors

Convex vector search maintains **sub-50ms response times** for typical workloads under 100,000 vectors. The built-in consistency guarantees and reactive updates provide significant advantages for applications requiring real-time synchronization across clients. However, performance characteristics change dramatically as datasets grow beyond the recommended threshold.

**Filtering optimization** plays a crucial role in maintaining performance. Pre-indexed filter fields enable efficient narrowing of search scope without post-processing penalties. Always define filter fields in the vector index rather than filtering results after retrieval:

```typescript
// Efficient: Filter at index level
const results = await ctx.vectorSearch("documents", "by_embedding", {
  vector: queryEmbedding,
  limit: 10,
  filter: (q) => q.and(
    q.eq("category", "technical"),
    q.gte("_creationTime", dateThreshold)
  ),
});

// Inefficient: Post-processing filter
const allResults = await ctx.vectorSearch("documents", "by_embedding", {
  vector: queryEmbedding,
  limit: 100, // Fetch more to filter later
});
const filtered = allResults.filter(r => r.category === "technical");
```

For scale beyond 100k vectors, dedicated vector databases like **Qdrant or Pinecone** provide superior performance. Qdrant achieves 4x better performance in recent benchmarks with consistent sub-2ms latencies at scale. Pinecone's managed service handles millions of vectors with automatic scaling and optimization.

## Code patterns demonstrate message embedding and semantic search

Real-world implementations for chat and message applications follow established patterns combining user context with semantic search. A complete implementation stores conversation history while enabling semantic retrieval:

```typescript
export const chatWithContext = action({
  args: {
    sessionId: v.string(),
    message: v.string(),
  },
  handler: async (ctx, args) => {
    // Store user message
    await ctx.runMutation(internal.messages.create, {
      sessionId: args.sessionId,
      text: args.message,
      isViewer: true,
    });

    // Generate embedding for semantic search
    const embedding = await generateEmbedding(args.message);

    // Find relevant context
    const context = await ctx.vectorSearch("chunks", "by_embedding", {
      vector: embedding,
      limit: 5,
    });

    // Retrieve full context documents
    const contextDocs = await ctx.runQuery(internal.chunks.getByIds, {
      ids: context.map(c => c._id),
    });

    // Generate AI response with context
    const response = await generateAIResponse({
      message: args.message,
      context: contextDocs.map(d => d.text).join("\n"),
      history: await ctx.runQuery(internal.messages.getHistory, {
        sessionId: args.sessionId,
      }),
    });

    // Store AI response
    await ctx.runMutation(internal.messages.create, {
      sessionId: args.sessionId,
      text: response,
      isViewer: false,
    });

    return response;
  },
});
```

This pattern maintains conversation continuity while leveraging semantic search for relevant context retrieval. The separation between messages, chunks, and embeddings enables flexible content management without compromising search performance.

## Convex ecosystem provides specialized components without dedicated libraries

While Convex lacks dedicated vector operation libraries, the ecosystem offers several components enhancing vector functionality. The **Action Cache component** provides efficient caching for expensive embedding operations. **LangChain integration** through `ConvexVectorStore` enables sophisticated RAG pipelines. The **AI agent framework** includes built-in vector search and memory management for autonomous applications.

Community contributions demonstrate advanced patterns including incremental index updates, hybrid search combining keywords and vectors, and multi-modal embeddings. The **get-convex/convex-demos** repository showcases production-ready implementations for common use cases.

Third-party integrations extend capabilities through **Convex Components**. These reusable modules encapsulate complex vector workflows, embedding generation pipelines, and search optimization strategies. The component system enables sharing battle-tested implementations across projects while maintaining type safety and reactive guarantees.

## Architectural decisions depend on scale, consistency needs, and complexity

Choosing between Convex's built-in vector search and dedicated vector databases involves evaluating multiple factors. **For applications under 50,000 vectors** requiring tight integration with user data and real-time updates, Convex provides the optimal solution. The unified development experience, immediate consistency, and reactive subscriptions outweigh raw performance considerations.

**Applications scaling to 50-100k vectors** should implement hybrid architectures. Critical, frequently-accessed vectors remain in Convex for reactive queries, while bulk historical data migrates to specialized services. This approach maintains user experience quality while managing costs and performance.

**Beyond 100k vectors**, dedicated vector databases become necessary. Qdrant offers the best open-source performance with self-hosting options. Pinecone provides managed simplicity at higher cost. Weaviate excels at hybrid search combining dense and sparse vectors. The choice depends on specific requirements for latency, filtering complexity, and operational preferences.

## Conclusion

Convex's vector embedding support provides a **production-ready foundation** for AI applications requiring integrated search capabilities. The native implementation excels for applications prioritizing developer experience, consistency, and reactive updates over raw scale. With clear migration paths to dedicated solutions as requirements evolve, Convex enables rapid prototyping and iteration while maintaining architectural flexibility for future growth. The 100k vector guideline represents a practical threshold where specialized infrastructure begins delivering meaningful performance advantages, making Convex an excellent choice for the majority of AI applications that operate below this scale.
