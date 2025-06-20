# Nostr Relay Research Questions
## Session: 2025-06-19 23:20

### Objective
Research how to optimally connect autonomous agents to Nostr relays for real-time coordination without facing restrictions like proof-of-work requirements or authentication barriers.

### Context
We have complete Effect.js-based `RelayService` infrastructure but need to understand:
- Which relays will accept our NIP-28 (kinds 40-44) and NIP-90 (kinds 5000-7000) events
- How other libraries handle relay selection and failures
- Optimal strategies for agent deployment vs human clients

---

## Relay Selection & Management

**1. How does NDK handle relay discovery and selection?**
- What algorithms does NDK use to choose optimal relays?
- Does it have built-in relay scoring or reputation systems?
- How does it handle relay metadata and capabilities discovery?

**2. What relay pool strategies does nostr-tools use for reliability?**
- How many relays does it typically connect to simultaneously?
- What are the failover mechanisms when relays go offline?
- How does it distribute read vs write operations across relays?

**3. How does SNSTR handle relay failures and automatic fallbacks?**
- What retry logic is implemented for failed connections?
- How does it detect and respond to relay performance degradation?
- What connection pooling strategies are used?

**4. What are the most permissive public relays that accept all event kinds?**
- Which relays have minimal restrictions on event types?
- Are there relays specifically designed for development/testing?
- Which relays are known to accept automated/bot traffic?

**5. Which relays have the lowest barriers to entry (no auth, no PoW)?**
- Complete list of no-authentication-required relays
- Relays that don't require NIP-13 proof of work
- Any relays with explicit policies supporting automated clients?

---

## Policy & Compatibility

**6. What percentage of public relays require NIP-13 proof of work?**
- Statistical breakdown of PoW requirements across major relays
- What difficulty levels are typically required?
- Are there patterns by relay size/popularity?

**7. Which relays support NIP-28 (kinds 40-44) and NIP-90 (kinds 5000-7000) events?**
- Comprehensive compatibility matrix for our required event kinds
- Any relays that explicitly reject these experimental kinds?
- Special considerations for NIP-90 Data Vending Machine events?

**8. How do popular clients handle relays that reject their events?**
- What error handling patterns are used?
- Do they automatically try alternative relays?
- How do they inform users about rejections?

**9. What are common rate limiting policies across major relays?**
- Typical limits for reads, writes, subscriptions per timeframe
- How do limits differ for authenticated vs anonymous users?
- What happens when limits are exceeded?

**10. How do existing agent/bot implementations handle relay restrictions?**
- Examples of successful autonomous clients and their relay strategies
- Any known agent/AI projects using Nostr for coordination?
- Common patterns for programmatic vs human usage?

---

## Performance & Reliability

**11. What are typical connection patterns for automated clients vs human users?**
- How often do bots vs humans connect/disconnect?
- Different usage patterns that might trigger anti-bot measures?
- Recommended connection persistence strategies?

**12. How do libraries handle WebSocket reconnections and state recovery?**
- Automatic reconnection logic and backoff strategies
- How is subscription state restored after disconnection?
- Event deduplication and gap handling mechanisms?

**13. What monitoring/health checks do production Nostr clients implement?**
- What metrics are tracked for relay health?
- How do they detect and respond to degraded performance?
- Alerting and observability patterns?

**14. How do they handle relay latency and optimize for real-time communication?**
- Strategies for minimizing message delivery latency
- Geographic relay selection for performance
- Batching vs individual event publishing trade-offs?

---

## Economic Considerations

**15. Which relays offer free tiers suitable for agent development/testing?**
- Relays with generous free usage limits
- Any relays specifically supporting open source development?
- Trial periods or sandbox environments available?

**16. What are the costs for paid relay access vs running our own relay?**
- Typical pricing models for premium relay access
- Infrastructure costs for deploying a private relay
- Break-even analysis for usage volumes?

**17. How do existing AI/agent projects handle relay infrastructure costs?**
- Cost optimization strategies for high-volume automated clients
- Examples of projects that deployed their own relays vs using public ones
- Economic models that make agent coordination viable?

---

## Research Methodology

### Data Sources
- [ ] GitHub repositories: NDK, nostr-tools, SNSTR source code analysis
- [ ] Public relay lists and directories (nostr.watch, relay.exchange)
- [ ] Nostr protocol documentation and NIP specifications  
- [ ] Community forums, Discord/Telegram discussions
- [ ] Production client implementations and case studies

### Deliverables
- [ ] Relay compatibility matrix for our required event kinds
- [ ] Recommended relay pool configuration for agent deployment
- [ ] Implementation patterns from successful libraries
- [ ] Cost analysis and infrastructure recommendations
- [ ] Risk assessment for different relay strategies

### Success Criteria
- Identify 3-5 reliable relays that accept our events without restrictions
- Understand optimal connection patterns for autonomous agents
- Have clear fallback strategy if public relays prove inadequate
- Cost-effective path to production deployment

---

## Priority Order

**High Priority** (needed for immediate Phase 1 implementation):
- Questions 4, 5, 7: Which relays will accept our events
- Questions 2, 3: Proven reliability patterns from other libraries

**Medium Priority** (needed for robust production deployment):
- Questions 6, 8, 9: Understanding restrictions and handling failures  
- Questions 11, 12: Optimal connection patterns for agents

**Low Priority** (optimization and long-term strategy):
- Questions 15, 16, 17: Economic considerations and cost optimization
- Questions 13, 14: Advanced monitoring and performance tuning

---

*Research to be conducted by AI research agent with findings compiled for immediate Phase 1 implementation decisions.*