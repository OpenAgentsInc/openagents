# Markdown as a primitive for Psionic hypermedia framework

The integration of markdown as a core primitive for the Psionic hypermedia framework built on Bun presents a unique opportunity to combine high-performance server-side rendering with malleable, user-customizable content systems. After extensive research into various markdown solutions, security considerations, and architectural patterns, this report provides comprehensive recommendations aligned with Psionic's principles of minimal JavaScript, type safety, and hypermedia-driven design.

## Bun's markdown capabilities prove exceptional for performance

While Bun doesn't include native markdown parsing APIs, its performance characteristics make it an excellent runtime for markdown-heavy applications. **Bun delivers 2-4x better HTTP throughput than Node.js**, with 52,000+ requests per second capability. Its JavaScriptCore engine provides superior memory efficiency, and Bun.file() operations are twice as fast as Node.js equivalents. These advantages compound when processing markdown at scale.

For optimal performance, implement a tiered caching architecture leveraging Bun's static routes for frequently accessed content. The recommended approach combines in-memory Map/WeakMap caching with file-based pre-rendered HTML storage. Bun's built-in file watching capabilities enable efficient cache invalidation, while its streaming responses handle large documents without memory overhead. The startup time advantage – 4x faster than Node.js – proves particularly valuable for serverless deployments.

## markdown-it emerges as the ideal parser for hypermedia applications

Among the various markdown processing solutions evaluated, **markdown-it stands out as the optimal choice** for Psionic's hypermedia-first architecture. It achieves 743-1,587 operations per second in benchmarks while maintaining a minimal memory footprint. The library's 150+ plugin ecosystem provides extensive customization without sacrificing performance, and its synchronous processing model aligns perfectly with server-side rendering requirements.

The unified/remark/rehype ecosystem offers more powerful AST manipulation capabilities but incurs significant performance overhead – typically 500-800 ops/sec due to AST creation and transformation costs. MDX, while popular, proves fundamentally incompatible with hypermedia principles due to its React dependency and client-side JavaScript requirements. For Psionic's use case, markdown-it's combination of performance, extensibility, and framework-agnostic output makes it the clear winner.

## Hypermedia integration requires thoughtful architectural patterns

Integrating markdown with HTMX and hypermedia principles demands specific implementation strategies. The core pattern involves server-side markdown processing that generates pure HTML fragments for HTMX requests. By detecting the `hx-request` header, the server can return either full pages or HTML fragments as needed, enabling seamless partial updates without client-side JavaScript.

The recommended implementation leverages markdown-it's plugin system to embed HTMX attributes directly within markdown content. Custom directives can transform markdown syntax into hypermedia-enabled components, such as converting specific link patterns into HTMX-powered navigation or form elements. This approach maintains the simplicity of markdown authoring while enabling rich interactivity through server-driven state changes.

## Web Components integration through Declarative Shadow DOM

Server-side rendering of Web Components within markdown becomes achievable through Declarative Shadow DOM, a browser standard that enables shadow roots without JavaScript. This technology allows Psionic to maintain component encapsulation and style isolation while delivering fully rendered HTML from the server. The `<template shadowrootmode="open">` pattern provides the foundation for this integration.

Implementation requires a custom markdown directive system that transforms component references into properly structured Declarative Shadow DOM. While browsers without native support need a small polyfill, the approach enables progressive enhancement where components upgrade automatically when JavaScript loads. Critical considerations include handling cross-shadow-boundary references for accessibility and implementing proper hydration strategies for interactive components.

## Custom markdown extensions balance power with simplicity

Psionic should implement a carefully designed set of markdown extensions that enhance functionality without compromising the format's simplicity. The directive syntax (using colons for inline and triple colons for block directives) provides an intuitive way to embed components and invoke server-side functionality. These extensions should focus on hypermedia-specific features like partial content loading, server-side includes, and real-time update subscriptions.

The extension system must maintain strict boundaries between content and presentation. Rather than embedding complex logic in markdown, directives should reference server-side components that handle processing. This approach keeps markdown files portable and readable while enabling powerful runtime behavior through the server's interpretation of directives.

## Performance optimization through strategic architecture

Achieving optimal performance requires a multi-layered approach combining compile-time optimization with runtime efficiency. Pre-compile frequently accessed markdown during deployment, implement aggressive caching with proper invalidation strategies, and use Bun's streaming capabilities for large documents. The architecture should support both static generation for unchanging content and dynamic rendering for personalized or real-time content.

Connection pooling for database-backed metadata, bytecode compilation for production deployments, and strategic use of Bun's compiled executables all contribute to minimizing latency. The key insight is treating markdown processing as a pipeline where each stage can be independently optimized, cached, and scaled based on usage patterns.

## Effect-TS provides robust type safety and error handling

Integrating Effect-TS into the markdown processing pipeline ensures type safety and predictable error handling throughout the system. Define schemas for component props, markdown frontmatter, and processing options using Effect Schema. This approach catches configuration errors at compile time and provides meaningful runtime validation for user-generated content.

The Effect pattern proves particularly valuable for handling the various failure modes in markdown processing: file not found, parsing errors, component rendering failures, and network issues during real-time sync. By modeling these as typed effects, Psionic can provide consistent error handling and recovery strategies across the entire markdown pipeline.

## Real-time synchronization leverages event sourcing and CRDTs

For real-time collaborative editing, **implement CRDT-based synchronization using Yjs** rather than Operational Transform. CRDTs provide better offline support, eliminate the need for a central authority, and simplify conflict resolution. The event sourcing pattern complements this approach by maintaining an immutable audit trail of all changes, enabling features like version history and rollback.

WebSockets provide the transport layer for real-time updates, with careful attention to authentication and rate limiting. Each markdown document becomes an aggregate in the event sourcing model, with events capturing user edits, metadata changes, and access control modifications. This architecture supports both real-time collaboration and asynchronous workflows like review and approval processes.

## Security requires defense in depth

User-generated markdown poses significant security risks that demand a comprehensive mitigation strategy. **The fundamental principle: sanitize HTML output, never markdown input**. After markdown parsing, use DOMPurify or equivalent to remove dangerous elements and attributes. This approach preserves markdown's expressiveness while preventing XSS attacks.

Implement strict Content Security Policy headers, validate all URLs against a whitelist of allowed schemes, and monitor for suspicious patterns in user content. For real-time features, authenticate WebSocket connections using JWT tokens and implement per-user rate limiting. The architecture should assume all user input is potentially malicious and apply appropriate sanitization at every rendering point.

## Malleable markdown enables runtime customization

Drawing inspiration from systems like Obsidian and TiddlyWiki, Psionic should treat markdown as a malleable medium that users can customize at runtime. Implement a plugin system allowing users to define custom rendering rules, component mappings, and processing pipelines. These customizations should persist across sessions and synchronize across devices.

The malleability extends to the editing experience itself. Users should be able to modify keyboard shortcuts, create custom templates, and define their own markdown extensions. This approach transforms markdown from a static format into a programmable medium that adapts to individual workflows and preferences. The key is maintaining a balance between flexibility and system coherence.

## Architectural recommendations prioritize composability

The recommended architecture for Psionic's markdown system follows a composable, pipeline-based approach. At its core, use markdown-it for parsing with a carefully curated set of plugins. Layer on custom directives for hypermedia integration, Effect-TS for type safety, and Yjs for real-time collaboration. Each component should be independently testable and replaceable.

The implementation should proceed in phases: first establishing the core markdown processing pipeline, then adding hypermedia integration, followed by real-time features and finally malleability. This incremental approach allows for validation at each stage and ensures that foundational decisions properly support advanced features. Throughout development, maintain focus on server-side rendering performance and minimal client-side JavaScript.

## Conclusion

Treating markdown as a primitive for the Psionic hypermedia framework opens powerful possibilities for content-driven applications. By combining Bun's exceptional performance, markdown-it's extensibility, and thoughtful integration with hypermedia principles, Psionic can deliver a unique platform that respects both developer ergonomics and end-user agency. The key is maintaining markdown's simplicity while enabling sophisticated server-driven interactivity through careful architectural choices and robust engineering practices.
