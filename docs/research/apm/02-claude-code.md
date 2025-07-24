# Claude Code: Technical Architecture and APM Implementation Guide

## Bottom Line Up Front

Claude Code is Anthropic's command-line agentic coding tool that **already includes built-in OpenTelemetry support (beta)** for comprehensive performance monitoring. The tool operates as a standalone Node.js CLI application with extensive instrumentation capabilities, tracking metrics like token usage, tool invocations, session costs, and code modifications. For implementing APM-like measurements, organizations can leverage the existing OpenTelemetry integration combined with tools like Prometheus and Grafana to track "actions per minute" equivalents including message throughput, tool execution rates, and code generation events.

## Claude Code Architecture: A Deep Technical Dive

### Core System Design

Claude Code operates as a **standalone CLI tool** built on Node.js, distributed via NPM as `@anthropic-ai/claude-code`. Unlike many AI coding assistants, it's not a VSCode extension but rather a terminal-native application that integrates directly with development workflows. The architecture follows a "low-level and unopinionated" design philosophy, providing near-raw access to Claude's AI capabilities without forcing specific workflows.

The system runs locally as a subprocess that communicates directly with Anthropic's API servers, eliminating intermediate servers and maintaining data privacy. Each `claude` command creates or resumes a session with a unique ID, enabling persistent context across interactions. The tool supports multiple models including Claude 3.7 Sonnet (default), Sonnet 4, and Opus 4, with automatic model switching based on usage patterns and rate limits.

### Communication Architecture and Model Context Protocol

Claude Code's extensibility centers around the **Model Context Protocol (MCP)**, an open standard for AI-tool integration. MCP uses JSON-RPC 2.0 over multiple transport mechanisms including STDIO for local integrations, Server-Sent Events (SSE) for remote connections, and streamable HTTP for bidirectional communication. This protocol enables Claude Code to function as both an MCP client and server, creating a flexible integration ecosystem.

The messaging system maintains stateful sessions with bidirectional communication between users, the Claude Code agent, and external services. Each MCP server exposes three types of capabilities: Resources (file-like data access), Tools (executable functions), and Prompts (reusable templates). Configuration occurs at multiple scopes - local project-specific settings, team-shared configurations via `.mcp.json` files, and user-wide preferences.

### Technical Specifications and Performance Characteristics

Claude Code demonstrates impressive performance benchmarks across its model offerings. **Opus 4 achieves 72.5% on SWE-bench Verified** (79.4% with parallel test-time compute), while **Sonnet 4 reaches 72.7%** (80.2% with parallel compute). Both models support 200,000 token context windows with sophisticated memory management through hierarchical CLAUDE.md files.

Rate limiting varies by subscription tier: Pro Plan users receive 10-40 prompts every 5 hours, while Max Plan subscribers get 200-800 prompts. API users benefit from per-minute limits starting at 50 requests for Tier 1. The system implements intelligent caching that can reduce costs by 90% and latency by 85% for repeated contexts, with cached tokens not counting against rate limits.

Response times target 200-300ms baseline latency with ~0.99 seconds median Time to First Token (TTFT) for Claude Sonnet. Output speed reaches 72 tokens/second median, though processing scales quadratically with context length. The architecture supports multiple concurrent instances for parallel processing, with load balancing possible across multiple API keys.

## Current Monitoring and Telemetry Capabilities

### Built-in OpenTelemetry Support

Claude Code includes **comprehensive OpenTelemetry integration** (currently in beta), enabling standard metrics and events export. Configuration requires setting `CLAUDE_CODE_ENABLE_TELEMETRY=1` with support for OTLP, Prometheus, and Console exporters. The system tracks extensive metrics including:

- **`claude_code.session.count`** - CLI sessions started
- **`claude_code.lines_of_code.count`** - Code modifications (added/removed)
- **`claude_code.pull_request.count`** - PRs created
- **`claude_code.commit.count`** - Git commits made
- **`claude_code.cost.usage`** - Session costs in USD
- **`claude_code.token.usage`** - Token consumption by type and model

Attributes include session IDs, app versions, organization IDs, user UUIDs, model names, and resource information like OS type and architecture. Export intervals default to 60 seconds for metrics and 5 seconds for logs, with enterprise support for managed configuration files.

### Comprehensive Logging Infrastructure

Session transcripts are stored as JSONL files at `~/.claude/projects/<project-name>/<session-id>.jsonl`, containing complete conversation history, tool usage with inputs/outputs, timestamps, and metadata. The 30-day local retention can be configured, with zero data retention available for appropriate API keys.

Additional logging includes MCP debugging via stderr for stdio transport, Sentry integration for operational errors, and Statsig telemetry for latency and reliability metrics. Users can opt out via `DISABLE_TELEMETRY` and `DISABLE_ERROR_REPORTING` environment variables.

### Enterprise Analytics Dashboard

Anthropic provides an **official analytics dashboard** at console.anthropic.com/claude_code for API users, tracking lines of code accepted, suggestion accept rates, user activity over time, total and average daily spend, and team productivity metrics. Role-based access controls ensure appropriate visibility across Primary Owner, Owner, Billing, Admin, and Developer roles.

## Implementing APM for Claude Code Instances

### Multi-Layer Instrumentation Strategy

Effective APM for Claude Code requires monitoring at three distinct layers. The **Decision Layer** tracks reasoning processes, planning stages, and decision trees using spans like `agent.reasoning.plan` with metrics for task complexity and confidence scores. The **Action Layer** monitors tool invocations, API calls, and external system interactions through spans like `agent.tool.execution` capturing tool names, operations, and success rates. The **Context Layer** tracks memory usage, conversation state, and context retention with appropriate privacy safeguards.

### Key Metrics for "Actions Per Minute" Equivalents

Traditional APM metrics require adaptation for AI agents. Instead of simple request counts, track:

**Message Throughput Metrics:**
- Messages sent/received per minute by session and user
- Average message processing time including thinking
- Queue depth for pending messages
- Message success/failure rates

**Tool Invocation Metrics:**
- Tool calls per minute by type (Read, Write, Bash, Git, MCP)
- Tool execution duration percentiles (P50, P95, P99)
- Tool success rates and error patterns
- Concurrent tool execution counts

**Code Generation Events:**
- Lines of code generated/modified per minute
- Code acceptance rates over time windows
- File operations per minute (creates, updates, deletes)
- Refactoring scope metrics (files touched, lines changed)

**System Interaction Metrics:**
- Terminal commands executed per minute
- API calls to external services (rate and latency)
- MCP server interactions and response times
- Git operations frequency and duration

### Implementation Architecture

A recommended APM stack combines OpenTelemetry for standardized data collection, Prometheus for metrics storage with multi-dimensional labels, Grafana for visualization with pre-built dashboards, and specialized AI monitoring tools like Langfuse or OpenLIT for advanced analytics.

For multi-instance monitoring, implement:

**Session-Level Aggregation:** Track individual conversation threads, user-specific patterns, and session completion rates. Use session IDs as primary correlation identifiers across all metrics.

**Instance-Level Aggregation:** Monitor agent performance across concurrent sessions, resource utilization per instance, and load distribution effectiveness. Implement instance health checks and automatic failover.

**Fleet-Level Aggregation:** Analyze organization-wide usage patterns, capacity planning metrics, and cost optimization opportunities. Create predictive models for usage forecasting.

### Real-Time Monitoring Implementation

Critical real-time metrics include response latency for user experience, error rates for system health, token rate limit warnings, and security anomaly detection. Implement streaming architecture using Apache Kafka or similar for event processing, with sub-second dashboard updates and immediate alerting on threshold breaches.

Configure alerts for:
- Response times exceeding 2-second thresholds
- Error rates above 5% in 5-minute windows
- Token usage approaching rate limits (80% threshold)
- Unusual tool invocation patterns indicating potential issues

### Dashboard Design for Claude Code APM

Create a three-tier dashboard architecture:

**Executive Dashboard** displays high-level KPIs including system availability, user satisfaction scores, total cost trends, and business impact metrics like developer productivity gains.

**Operational Dashboard** shows real-time system health with active session counts, resource utilization across instances, current error rates and response times, and alert status with escalation paths.

**Developer Dashboard** provides detailed trace analysis showing complete request flows, performance profiling identifying bottlenecks, tool usage pattern analysis, and debugging interfaces for troubleshooting.

## Community Tools and Extended Monitoring Ecosystem

The Claude Code community has developed extensive monitoring solutions. **Claude-Code-Usage-Monitor** provides real-time terminal dashboards with predictive analytics for usage limits. **ccusage** offers fast CLI-based usage analysis with JSON export capabilities. **claude-code-otel** delivers a full observability stack with Grafana and Prometheus integration. **Datadog** integration enables enterprise-grade monitoring with custom Claude Code dashboards.

These tools address gaps in built-in monitoring, particularly real-time usage tracking, predictive analytics for rate limit management, and cross-session analysis capabilities. Organizations can leverage these alongside official tools for comprehensive observability.

## APM Implementation Roadmap

**Phase 1 (Weeks 1-2):** Enable built-in OpenTelemetry support, configure Prometheus and Grafana, establish baseline metrics collection, and create initial operational dashboards.

**Phase 2 (Weeks 3-4):** Implement custom metrics for tool usage and code generation, add session-level tracking and correlation, create user experience dashboards, and establish automated alerting rules.

**Phase 3 (Weeks 5-6):** Deploy distributed tracing for multi-tool workflows, implement cost tracking and optimization metrics, add predictive analytics for usage patterns, and create performance optimization workflows.

**Phase 4 (Ongoing):** Scale monitoring across multiple instances, implement ML-based anomaly detection, establish automated remediation for common issues, and continuously refine metrics based on operational insights.

## Technical Recommendations and Best Practices

For production deployments, implement client-side rate limiting using token bucket algorithms to prevent API throttling. Extensively use prompt caching for repeated contexts to reduce both costs and latency. Deploy across multiple API keys for load distribution and redundancy. Establish request queuing systems for handling burst traffic gracefully.

Monitor critical performance indicators including token usage efficiency (cached vs. uncached), tool execution patterns for optimization opportunities, session duration and completion rates, and cost per successful task completion. These metrics enable both technical optimization and business value demonstration.

## Conclusion

Claude Code provides a robust foundation for APM implementation through its built-in OpenTelemetry support, comprehensive logging infrastructure, and extensible architecture. The combination of official monitoring capabilities and a thriving ecosystem of community tools enables organizations to implement sophisticated APM solutions tracking "actions per minute" equivalents across messages, tool invocations, code generation, and system interactions.

Success requires treating monitoring as a first-class architectural concern, leveraging existing instrumentation while adding AI-specific metrics, and creating dashboards that serve both technical and business stakeholders. With proper implementation, organizations can achieve comprehensive visibility into Claude Code performance, optimize resource usage, and demonstrate clear ROI from AI-assisted development workflows.
