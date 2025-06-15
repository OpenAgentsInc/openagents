# Nostr Relay (Relay) Specification

## 1. Introduction

### 1.1 Purpose
This document outlines the specifications for Relay, a Nostr relay designed and developed by OpenAgents, Inc. Relay aims to be a secure, performant, and highly customizable Nostr relay, optimized for privacy-focused use cases and forming a core component of the OpenAgents AI agent ecosystem.

Relay embraces Nostr's "smart client/dumb server" architecture while innovating at the boundaries through embedded relay capabilities. This enables unprecedented user sovereignty—users maintain complete control over their data while participating in the broader decentralized network.

### 1.2 Project Goal
To create a robust Nostr relay implementation that:
*   Adheres to core Nostr Improvement Proposals (NIPs).
*   Leverages modern technologies: EffectTS for robust, type-safe business logic; PostgreSQL for persistent storage; and pglite for lightweight, embedded, or edge deployment scenarios.
*   Prioritizes security, privacy, and censorship resistance, catering to the needs of privacy-focused users and AI agents operating in potentially hostile environments.
*   Serves as a foundational communication layer for OpenAgents' suite of AI agents (e.g., Commander, Onyx) and the broader decentralized ecosystem.
*   Scales to support the vision of 100 million users utilizing AI-powered privacy tools.

### 1.3 Key Technologies
*   **EffectTS**: For the core relay logic, ensuring type safety, composability, and robust error handling.
*   **PostgreSQL**: As the primary backend for persistent event storage in scalable, server-based deployments.
*   **pglite**: To enable relay functionality in environments without a traditional PostgreSQL server, such as embedded in client applications (desktop/mobile), edge functions, or for local development/testing.
*   **Rivet + Actor Core**: Leveraging stateful actors for connection management and edge deployment.

### 1.4 Relationship to OpenAgents Ecosystem
Relay is the reference Nostr relay implementation for OpenAgents. It is designed to seamlessly integrate with:
- **AI Agents**: Commander, Onyx, and Digital Srđa agents use Relay for coordination
- **Actor Infrastructure**: Each WebSocket connection managed by dedicated Rivet actors
- **Decentralized Platform**: Supporting 100M users with decentralized communication
- **Bitcoin/Lightning**: NWC integration for micropayment-enabled features

## 2. Goals and Objectives

*   **NIP Compliance**: Implement and maintain compatibility with fundamental and relevant optional NIPs.
*   **Security & Privacy**: Design with a "security-first" mindset, incorporating features that protect user data and resist surveillance.
*   **Performance**: Ensure low-latency event propagation (<100ms) and efficient handling of concurrent connections and subscriptions.
*   **Scalability**: Architect to support 100M+ users, billions of events, and millions of concurrent subscriptions.
*   **Resilience & Censorship Resistance**: Offer deployment options and features that enhance uptime and resist attempts at censorship or shutdown.
*   **Maintainability**: Utilize EffectTS to create a well-structured, testable, and maintainable codebase.
*   **Flexibility**: Support various deployment models, from large-scale cloud relays to lightweight embedded instances using pglite.
*   **Privacy Focused**: Incorporate considerations specific to the needs of privacy-conscious users, such as enhanced metadata protection and anonymous operation where feasible.
*   **AI Agent Optimized**: Special handling for agent-to-agent communication patterns and NIP-90 job routing.

## 3. Target Users and Systems

*   **OpenAgents AI Agents**: Commander, Onyx, Digital Srđa, and other AI agents developed by OpenAgents will use Relay as a primary communication and coordination backbone.
*   **Privacy-Focused Users & Applications**: Tools and platforms developed by OpenAgents will leverage Relay, targeting 100M users globally.
*   **Nostr Clients**: Standard Nostr clients should be able to connect and interact with Relay instances.
*   **Developers**: Developers building applications on the OpenAgents platform.
*   **Organizations**: NGOs and organizations requiring secure, decentralized communication.

## 4. Functional Requirements

### 4.1 NIP-01: Basic Protocol Flow
Relay MUST fully implement NIP-01. This includes:
*   **Event Structure**: Correctly parsing, validating (including signature verification), and storing NIP-01 event objects.
    *   `id`: 32-bytes hex-encoded sha256 of serialized event.
    *   `pubkey`: 32-bytes hex-encoded public key.
    *   `created_at`: Unix timestamp.
    *   `kind`: Integer.
    *   `tags`: Array of arrays of strings.
    *   `content`: String.
    *   `sig`: 64-bytes hex-encoded signature.
    *   Strict adherence to NIP-01 event serialization rules for ID generation.
*   **Communication Protocol**:
    *   Establishing WebSocket connections.
    *   Handling client-to-relay messages:
        *   `["EVENT", <event JSON>]`: Publish events. Relay MUST validate event (ID, signature, structure).
        *   `["REQ", <subscription_id>, <filters1>, <filters2>, ...>]`: Request events and subscribe.
        *   `["CLOSE", <subscription_id>]`: Stop subscriptions.
    *   Handling relay-to-client messages:
        *   `["EVENT", <subscription_id>, <event JSON>]`: Send stored or real-time events matching a subscription.
        *   `["OK", <event_id>, <true|false>, <message>]`: Acknowledge event publication status with standardized prefixes (`duplicate`, `pow`, `blocked`, `rate-limited`, `invalid`, `restricted`, `error`).
        *   `["EOSE", <subscription_id>]`: Indicate end of stored events for a subscription.
        *   `["CLOSED", <subscription_id>, <message>]`: Indicate subscription closure by the relay with standardized prefixes.
        *   `["NOTICE", <message>]`: Send human-readable messages to clients.
*   **Filter Logic**: Implement NIP-01 filter attributes:
    *   `ids`: List of event IDs.
    *   `authors`: List of pubkeys.
    *   `kinds`: List of kind numbers.
    *   `#<single-letter>` tags (e.g., `#e`, `#p`): List of tag values. Indexed tags MUST be queryable.
    *   `since`: Unix timestamp (inclusive).
    *   `until`: Unix timestamp (inclusive).
    *   `limit`: Maximum number of events for initial query (ordered by `created_at` DESC, then `id` ASC for ties).
    *   Multiple conditions within a filter are `AND`. Multiple filters in a `REQ` are `OR`.
*   **Event Kinds Handling**:
    *   **Regular Events** (e.g., kind 1): Store all.
    *   **Replaceable Events** (e.g., kind 0, 3, 10000-19999): Store only the latest event per `pubkey` and `kind`. For ties in `created_at`, the event with the lowest `id` (lexicographically) is kept.
    *   **Ephemeral Events** (e.g., kind 20000-29999): Not stored by default. Forwarded to subscribers if matching.
    *   **Addressable (Parameterized Replaceable) Events** (e.g., kind 30000-39999): Store only the latest event per `pubkey`, `kind`, and `d` tag value.

### 4.2 NIP Support (Initial Scope & Future)
Relay will aim to support the following NIPs, prioritized based on OpenAgents' needs:

**Phase 1 (Core Relay Functionality - MVP):**
*   **NIP-01**: Basic protocol (Mandatory)
*   **NIP-02**: Contact List and Pet Graph (Events of kind 3)
*   **NIP-09**: Event Deletion (Kind 5 events)
*   **NIP-11**: Relay Information Document (Serve `application/nostr+json` on WebSocket upgrade)
*   **NIP-13**: Proof of Work (Anti-spam protection)
*   **NIP-17**: Private Direct Messages (Critical for secure communications)
*   **NIP-19**: bech32-encoded entities
*   **NIP-28**: Public Chat (For community coordination)
*   **NIP-42**: Authentication (Critical for private relays)
*   **NIP-45**: Event Counts (Support for `COUNT` message type)

**Phase 2 (Advanced Features & Agent Support):**
*   **NIP-44**: Versioned Encryption (Used by NIP-17)
*   **NIP-50**: Search Capability (Essential for agents finding relevant data)
*   **NIP-59**: Gift Wrap (Used by NIP-17)
*   **NIP-65**: Relay List Metadata (Kind 10002)
*   **NIP-89**: Recommended Application Handlers (Kind 31990, for DVM discovery)
*   **NIP-90**: Data Vending Machines (Critical for AI agent jobs - kinds 5000-7999)
*   **NIP-94**: File Metadata (Kind 1063)
*   **NIP-98**: HTTP Auth for relays

**Phase 3 (Future Capabilities):**
*   **NIP-EE (MLS on Nostr)**: E2EE messaging using MLS protocol for scalable group encryption
    - Support for kinds 443 (KeyPackage), 444 (Welcome), 445 (Group Events)
    - Integration with `nostr_group_data` extension for secure metadata
    - Ephemeral keypair generation for enhanced privacy
*   Custom NIPs for agent swarm coordination
*   Custom NIPs for privacy-specific features (emergency broadcasts, dead man's switches)

### 4.3 Administration and Moderation
*   Configurable policies for event acceptance (e.g., based on PoW, pubkey whitelists/blacklists, content filtering rules).
*   Logging capabilities (with privacy considerations, see Security).
*   Metrics and monitoring hooks (e.g., number of connections, events processed, subscription counts).
*   Emergency broadcast capabilities for critical user alerts.
*   Automatic relay reputation system for spam prevention.

### 4.4 Actor-Based Features (Rivet Integration)
*   **Connection Actors**: Each WebSocket connection managed by a dedicated actor
*   **Subscription Actors**: Persistent subscription matching with actor state
*   **Event Processing Actors**: Parallel event validation and storage
*   **DVM Router Actors**: Specialized actors for NIP-90 job routing
*   **Relay Coordination Actors**: Inter-relay communication for resilience

## 5. Technical Architecture

### 5.1 System Overview
Relay will be architected as a modular, actor-based system leveraging Rivet for stateful connection management:

```
┌─────────────────────┐     ┌──────────────────────┐     ┌─────────────────────┐
│   Nostr Clients     │────▶│  Rivet Edge Nodes    │────▶│  Actor Supervisor   │
│ (Agents, Apps, etc) │     │  (Global Distribution)│     │  (Orchestration)    │
└─────────────────────┘     └──────────────────────┘     └──────────┬───────────┘
                                                                     │
                          ┌────────────────┐  ┌────────────────┐    │
                          │ Connection     │  │ Subscription   │    │
                          │ Actor          │  │ Matcher Actor  │◀───┘
                          └────────┬───────┘  └────────┬───────┘
                                   │                    │
                          ┌────────▼───────────────────▼────────┐
                          │      EffectTS Core Logic            │
                          │         (Relay Engine)              │
                          └────────┬───────────────────┬────────┘
                                   │                   │
                          ┌────────▼────────┐ ┌───────▼────────┐
                          │ PostgreSQL DB   │ │ pglite (Edge)  │
                          │ (Global State)  │ │ (Local State)  │
                          └─────────────────┘ └────────────────┘
```

### 5.2 EffectTS Core Logic ("Relay Engine")
*   **Services**: The application will be structured using EffectTS services:
    *   `WebSocketService`: Manages client connections and message framing
    *   `EventService`: Handles event validation, processing, and storage
    *   `SubscriptionService`: Manages client subscriptions and event dispatching
    *   `FilterService`: Implements NIP-01 filtering logic with optimization
    *   `AuthService`: Handles NIP-42 client authentication
    *   `ConfigService`: Manages relay configuration
    *   `ActorService`: Coordinates with Rivet actors
    *   `DVMService`: Routes NIP-90 job requests to appropriate handlers
    *   `SecurityService`: Rate limiting, PoW validation, spam detection
    *   `ReputationService`: Tracks pubkey reputation for quality control
*   **Layers**: Services composed for different environments:
    *   `PostgresLayer`: Full-scale cloud deployment
    *   `PgliteLayer`: Edge and embedded deployments
    *   `ActorLayer`: Rivet actor integration
    *   `TestLayer`: In-memory for testing
*   **Error Handling**: Robust error handling using EffectTS's typed errors
*   **Concurrency**: Utilize EffectTS fibers and Rivet actors for massive concurrency

### 5.3 Actor-Based Architecture (Rivet Integration)
```typescript
// Connection Actor - Manages individual client connections
export class ClientConnectionActor extends Actor<ConnectionState> {
  static initialState(): ConnectionState {
    return {
      clientId: "",
      authenticated: false,
      subscriptions: new Map(),
      rateLimitBucket: { tokens: 100, lastRefill: Date.now() },
      metadata: {}
    }
  }

  async handleMessage(message: NostrMessage) {
    // Rate limiting
    if (!this.consumeRateLimit()) {
      return this.sendError("rate-limited")
    }

    // Process based on message type
    switch (message[0]) {
      case "EVENT":
        await this.handleEvent(message[1])
        break
      case "REQ":
        await this.handleSubscription(message[1], message.slice(2))
        break
      case "CLOSE":
        await this.handleClose(message[1])
        break
      case "AUTH":
        await this.handleAuth(message[1])
        break
    }
  }

  async handleSubscription(subId: string, filters: Filter[]) {
    // Store subscription in actor state
    this.state.subscriptions.set(subId, { filters, active: true })

    // Forward to subscription matcher
    await this.send("subscription-matcher", "newSubscription", {
      clientId: this.state.clientId,
      subId,
      filters
    })
  }
}

// Subscription Matcher Actor - Efficiently matches events to subscriptions
export class SubscriptionMatcherActor extends Actor<MatcherState> {
  static initialState(): MatcherState {
    return {
      subscriptionTree: new SubscriptionTree(), // Optimized data structure
      activeSubscriptions: 0
    }
  }

  async matchEvent(event: NostrEvent) {
    const matches = this.state.subscriptionTree.findMatches(event)

    // Fan out to connection actors
    await Promise.all(
      matches.map(match =>
        this.send(`connection-${match.clientId}`, "eventMatched", {
          subId: match.subId,
          event
        })
      )
    )
  }
}
```

### 5.4 Database Interaction
*   **Abstract Interface**: `DatabaseService` with implementations for:
    *   PostgreSQL (via Prisma/Drizzle/Effect-SQL)
    *   pglite (WASM PostgreSQL)
    *   Actor State (for ephemeral data)
*   **Query Optimization**:
    *   Prepared statements for common queries
    *   Connection pooling for PostgreSQL
    *   Read replicas for scaling
*   **Schema Design**: Optimized for Nostr query patterns (see Data Model)

### 5.5 pglite Strategy
Three deployment modes leveraging pglite (PostgreSQL compiled to WebAssembly, ~3MB gzipped):

1. **Embedded Relay Mode**:
   - Bundled within Commander/Onyx applications
   - Local, private relay for individual users
   - Persists to file system or IndexedDB
   - Syncs with main relays when online
   - Enables true offline-first operation
   - Zero network latency for local queries
   - Complete data sovereignty

2. **Edge Relay Mode**:
   - Deployed on Rivet edge nodes globally
   - Uses pglite as WASM module
   - Regional data sharding for compliance
   - Sub-50ms latency for local users
   - Automatic failover between edge nodes
   - Bandwidth optimization for mobile users

3. **Lightweight Server Mode**:
   - Standalone server with file-based pglite
   - Perfect for community/organizational relays
   - No external database dependencies
   - Easy deployment in hostile environments
   - Runs on minimal hardware (Raspberry Pi capable)
   - One-command setup for non-technical users

### 5.6 Performance Optimizations
*   **Connection Pooling**: Reuse database connections
*   **Query Batching**: Batch similar queries together
*   **Subscription Trees**: Efficient data structures for matching
*   **Event Deduplication**: Bloom filters for quick duplicate detection
*   **Lazy Loading**: Stream large result sets
*   **Caching Layer**: Redis/Upstash for hot data
*   **CDN Integration**: Static content via edge CDN
*   **Compression**: zstd dictionary compression (30-70% reduction)
*   **Intelligent Prefetching**: Predict and cache likely-accessed content
*   **Adaptive Sync**: Adjust frequency based on battery/network quality

## 6. Data Model (PostgreSQL / pglite)

### 6.1 Core Schema
```sql
-- Events table - Core Nostr events
CREATE TABLE events (
    id TEXT PRIMARY KEY, -- 64-char hex event ID
    pubkey TEXT NOT NULL, -- 64-char hex public key
    created_at BIGINT NOT NULL, -- Unix timestamp
    kind INTEGER NOT NULL,
    content TEXT NOT NULL,
    sig TEXT NOT NULL, -- 128-char hex signature
    raw_event JSONB NOT NULL, -- Full event for quick retrieval
    expires_at BIGINT, -- NIP-40 expiration
    d_tag_value TEXT, -- NIP-33 addressable events
    deleted_at BIGINT, -- Soft delete timestamp
    pow_difficulty INTEGER, -- NIP-13 PoW difficulty

    -- Indexes for common queries
    INDEX idx_pubkey_kind_created (pubkey, kind, created_at DESC),
    INDEX idx_kind_created (kind, created_at DESC),
    INDEX idx_created_at (created_at DESC),
    INDEX idx_expires_at (expires_at) WHERE expires_at IS NOT NULL,
    INDEX idx_d_tag (pubkey, kind, d_tag_value) WHERE d_tag_value IS NOT NULL,
    INDEX idx_deleted (deleted_at) WHERE deleted_at IS NOT NULL
);

-- Event tags for efficient filtering
CREATE TABLE event_tags (
    event_id TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    tag_name TEXT NOT NULL, -- e.g., "e", "p", "t", "d"
    tag_value TEXT NOT NULL,
    tag_order INTEGER NOT NULL, -- Position in tag array

    PRIMARY KEY (event_id, tag_name, tag_value, tag_order),
    INDEX idx_tag_lookup (tag_name, tag_value, event_id)
);

-- Client authentication (NIP-42)
CREATE TABLE auth_challenges (
    challenge TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    pubkey TEXT, -- Once authenticated

    INDEX idx_client (client_id),
    INDEX idx_expires (expires_at)
);

-- Relay metadata and configuration
CREATE TABLE relay_metadata (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    updated_at BIGINT NOT NULL
);

-- DVM job tracking (NIP-90)
CREATE TABLE dvm_jobs (
    job_id TEXT PRIMARY KEY, -- Event ID of job request
    job_kind INTEGER NOT NULL, -- 5000-5999
    status TEXT NOT NULL, -- pending, processing, completed, failed
    provider_pubkey TEXT,
    result_id TEXT, -- Event ID of result (6000-6999)
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,

    INDEX idx_status_kind (status, job_kind),
    INDEX idx_provider (provider_pubkey)
);

-- Reputation tracking
CREATE TABLE pubkey_reputation (
    pubkey TEXT PRIMARY KEY,
    total_events BIGINT DEFAULT 0,
    spam_events BIGINT DEFAULT 0,
    pow_events BIGINT DEFAULT 0,
    reputation_score FLOAT DEFAULT 0.5,
    last_updated BIGINT NOT NULL,

    INDEX idx_reputation (reputation_score DESC)
);
```

### 6.2 pglite Schema Subset
For edge/embedded deployments, use a simplified schema:
```sql
-- Minimal events table for pglite
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    kind INTEGER NOT NULL,
    raw_event TEXT NOT NULL, -- JSON string instead of JSONB

    INDEX idx_recent (created_at DESC)
);

-- Simplified tags table
CREATE TABLE event_tags (
    event_id TEXT NOT NULL,
    tag_name TEXT NOT NULL,
    tag_value TEXT NOT NULL,

    INDEX idx_tags (tag_name, tag_value)
);
```

## 7. Security Requirements

### 7.1 Core Security Features
*   **Input Validation**:
    - Event size limits (256KB default, configurable)
    - Tag count limits (2000 tags max)
    - Content length validation
    - Strict JSON parsing
    - Schnorr signature verification

*   **Authentication (NIP-42)**:
    - Challenge-response authentication
    - Time-limited challenges (5 minutes)
    - Rate limiting per pubkey
    - IP-based fallback limits

*   **Authorization**:
    - Configurable pubkey whitelists/blacklists
    - Event kind restrictions per pubkey
    - Read/write permissions
    - Private relay support

*   **Rate Limiting**:
    - Token bucket algorithm per connection
    - Different limits for authenticated users
    - Event submission limits
    - Subscription count limits
    - Bandwidth throttling

### 7.2 Privacy-Focused Security
*   **Metadata Protection**:
    - Support for NIP-17 gift-wrapped messages
    - Minimal logging of connection metadata
    - Configurable log retention (default: 24 hours)
    - No IP address storage without explicit config

*   **Censorship Resistance**:
    - Multiple deployment modes for redundancy
    - Relay reputation sharing
    - Automatic failover between relays
    - Tor/I2P support

*   **Emergency Features**:
    - Dead man's switch events
    - Panic button (wipe local data)
    - Duress codes for fake data
    - Encrypted backup/restore

*   **Anti-Surveillance**:
    - Traffic padding options
    - Decoy traffic generation
    - Connection timing randomization
    - Fake subscription noise

### 7.3 Spam and Abuse Prevention
*   **Proof of Work (NIP-13)**:
    - Configurable difficulty requirements
    - Higher difficulty for anonymous users
    - Lower difficulty for verified users
    - GPU-resistant algorithms

*   **Reputation System**:
    - Track pubkey behavior over time
    - Automatic spam detection
    - Community-based reporting
    - Graduated response (warn → throttle → ban)

*   **Content Filtering**:
    - Optional ML-based spam detection
    - Configurable word filters
    - Image/media scanning
    - Link validation

## 8. Performance and Scalability

### 8.1 Performance Targets
*   **Latency**:
    - Event ingestion: <50ms (p95)
    - Event broadcast: <100ms (p95)
    - Subscription matching: <10ms (p95)
    - EOSE delivery: <500ms for 10K events

*   **Throughput**:
    - 100K events/second ingestion
    - 1M events/second broadcast
    - 10K new connections/second
    - 100K concurrent subscriptions

*   **Resource Usage**:
    - <100MB RAM per 1000 connections
    - <1ms CPU per event processed
    - <10GB storage per million events

### 8.2 Scaling Strategy
*   **Horizontal Scaling**:
    - Actor distribution across Rivet nodes
    - Database read replicas
    - Geographic sharding
    - Load balancer support

*   **Vertical Scaling**:
    - Connection pooling optimization
    - Query optimization
    - Caching layers
    - Hardware acceleration

*   **Edge Scaling**:
    - 1000+ edge nodes globally
    - Automatic node discovery
    - Regional data sovereignty
    - Latency-based routing

## 8.3 Synchronization Strategies

### Offline-First Architecture
*   **Event Queue Management**:
    - Events created offline stored with pending status
    - Automatic retry with exponential backoff
    - Priority queuing (user's own events first)
    - Conflict resolution via cryptographic ordering

*   **Selective Synchronization**:
    - Configurable filters for bandwidth management
    - Follow-graph based prioritization
    - Time-based windowing for initial sync
    - Progressive enhancement as bandwidth allows

*   **Negentropy Protocol Integration**:
    - Efficient set reconciliation between relays
    - Minimal bandwidth for discovering differences
    - Batch synchronization for efficiency
    - Automatic deduplication via event IDs

*   **Network Partition Handling**:
    - Continue operation during connectivity loss
    - Queue events for later transmission
    - Multi-relay fallback strategies
    - Seamless recovery on reconnection

### Embedded Relay Sync Patterns
*   **Hybrid Push/Pull**:
    - Real-time WebSocket for live events
    - Periodic pull for reliability
    - Intelligent backfill on reconnection
    - Adaptive frequency based on activity

*   **Storage Management**:
    - Compression (30-70% via zstd)
    - Configurable retention policies
    - Hot/cold storage separation
    - Automatic garbage collection

*   **Bandwidth Optimization**:
    - Delta synchronization only
    - Binary protocol options
    - Compression for mobile networks
    - Pause sync on metered connections

## 9. Deployment Scenarios

### 9.1 Global Public Relay (Primary)
- **Infrastructure**: Multi-region cloud deployment
- **Database**: Managed PostgreSQL with read replicas
- **Actors**: Distributed across 100+ Rivet nodes
- **Capacity**: 10M+ concurrent users
- **Use Case**: Main relay for OpenAgents ecosystem

### 9.2 Regional Edge Relays
- **Infrastructure**: Rivet edge nodes in 50+ locations
- **Database**: pglite with regional sharding
- **Actors**: Local actor clusters
- **Capacity**: 100K users per region
- **Use Case**: Low-latency access for users

### 9.3 Organizational Relays
- **Infrastructure**: Single server or container
- **Database**: PostgreSQL or pglite
- **Actors**: Basic actor setup
- **Capacity**: 10K users
- **Use Case**: NGOs, organizations, communities

### 9.4 Personal Embedded Relays
- **Infrastructure**: Within Commander/Onyx apps
- **Database**: pglite with local storage (IndexedDB for web, file system for native)
- **Actors**: In-process actors
- **Capacity**: Single user + contacts
- **Storage**: ~200MB typical usage with compression
- **Use Case**: Offline-first personal relay
- **Benefits**: Zero latency, complete privacy, works during internet shutdowns

### 9.5 Hostile Environment Deployment
- **Infrastructure**: Hidden services (Tor/I2P)
- **Database**: Encrypted pglite
- **Actors**: Minimal footprint
- **Capacity**: Variable
- **Use Case**: Operating under surveillance

## 10. Integration Points

### 10.1 AI Agent Integration
- **Digital Srđa**: Direct WebSocket connection for real-time advice
- **Agent Swarms**: Coordination via custom event kinds
- **DVM Jobs**: AI processing via NIP-90
- **Memory Storage**: Agent state in replaceable events

### 10.2 Bitcoin/Lightning Integration
- **NWC Protocol**: Nostr Wallet Connect support
- **Payment Events**: Lightning invoice/payment tracking
- **Micropayments**: Pay-per-event storage
- **Bounties**: User reward distribution

### 10.3 OpenAgents Platform Integration
- **Community Coordination**: Public channels (NIP-28)
- **Secure Comms**: Private groups (NIP-17)
- **Resource Sharing**: File metadata (NIP-94)
- **Training Distribution**: Educational content events

### 10.4 External Systems
- **MLS Bridge**: WhiteNoise protocol (NIP-EE implementation)
- **IPFS Integration**: Content addressing for media
- **Matrix Bridge**: Cross-protocol messaging
- **Blockchain Attestation**: Timestamp proofs
- **Negentropy Protocol**: Set reconciliation for efficient sync
- **Citrine Compatibility**: Connect to companion relay on Android
- **strfry Integration**: High-performance relay synchronization

## 11. Monitoring and Operations

### 11.1 Metrics Collection
- **Performance Metrics**:
  - Event processing rate
  - Subscription match time
  - Database query latency
  - Actor message throughput

- **Health Metrics**:
  - Active connections
  - Memory usage
  - Disk usage
  - Error rates

- **Security Metrics**:
  - Failed auth attempts
  - Spam detection rate
  - PoW validation time
  - Rate limit hits

### 11.2 Operational Tools
- **Admin Dashboard**: Real-time relay status
- **Log Aggregation**: Centralized logging (privacy-aware)
- **Alerting**: Automated incident detection
- **Backup/Restore**: Automated backup procedures

### 11.3 Maintenance Procedures
- **Rolling Updates**: Zero-downtime deployments
- **Database Migrations**: Safe schema updates
- **Actor Rebalancing**: Load distribution
- **Garbage Collection**: Expired event cleanup

## 12. Future Roadmap

### 12.1 Short Term (3-6 months)
- Complete Phase 1 NIP implementation
- Launch beta with 1000 users
- Rivet actor integration
- Basic DVM support

### 12.2 Medium Term (6-12 months)
- Phase 2 NIPs including NIP-90
- 100K user capacity
- Advanced spam prevention
- WhiteNoise/NIP-EE integration planning

### 12.3 Long Term (12-24 months)
- 1M+ user support
- Full MLS integration
- Custom privacy-focused NIPs
- Quantum-resistant signatures

### 12.4 Vision (2+ years)
- 100M users on platform
- Fully decentralized architecture
- AI-native communication protocols
- Interplanetary relay support

## 13. Success Criteria

### 13.1 Technical Success
- 99.9% uptime for main relay
- <100ms event propagation globally
- Zero security breaches
- 100% NIP compliance for supported features

### 13.2 User Success
- 1M+ monthly active users
- 90% user satisfaction rating
- <1% spam rate
- 24/7 availability in hostile environments

### 13.3 Ecosystem Success
- 100+ integrated applications
- 1000+ community relays
- Active developer ecosystem
- Industry standard for privacy-focused communications

## 14. Conclusion

Relay represents more than just a Nostr relay—it's the communication backbone for the next generation of decentralized applications. By combining cutting-edge technology (EffectTS, Rivet actors, pglite) with privacy-focused features, Relay will enable secure, scalable, and censorship-resistant communication for 100 million users worldwide.

The architecture prioritizes flexibility, security, and performance while maintaining simplicity for operators and developers. With its actor-based design and multi-deployment options, Relay can adapt to any environment—from high-performance cloud infrastructure to resource-constrained mobile devices operating under surveillance.

This specification will guide the development of Relay as a living document, evolving with the needs of the OpenAgents ecosystem and the global privacy-focused community it serves.
