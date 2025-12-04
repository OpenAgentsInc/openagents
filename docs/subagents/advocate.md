# Advocate Subagent Spec

> A fiduciary subagent that represents user and project preferences in negotiations—within the MechaCoder ecosystem today, and eventually with external AI agents and systems at scale.

Inspired by Seb Krier's vision of ["Coasean Bargaining at Scale"](../research/coasean-bargaining-at-scale.md): AGI agents as "personal advocates" that can negotiate, form coalitions, and price externalities—turning transaction costs that once required top-down regulation into cheap, bottom-up bargaining.

---

## 1. Vision: From Internal Preferences to Coasean Negotiation

The Advocate starts as a **preference representation layer** for MechaCoder and evolves into a **full negotiation agent** capable of:

1. **Representing** your preferences with nuance and context
2. **Negotiating** resource allocation, priorities, and tradeoffs
3. **Forming coalitions** with other agents for collective bargaining
4. **Pricing externalities** in multi-stakeholder environments
5. **Enforcing contracts** through automated verification

This is inherently a longer-term, more speculative subagent than Healer or Archivist. We define three horizons:

| Horizon | Focus | Scope |
|---------|-------|-------|
| **v1 (Near-term)** | Internal preference management | Within MechaCoder ecosystem |
| **v2 (Medium-term)** | Multi-agent negotiation | With external AI systems |
| **v3 (Long-term)** | Full Coasean advocacy | Ecosystem-wide coordination |

---

## 2. Goals & Non-Goals

### Goals

1. **Preference representation with nuance**

   * Maintain a structured "preference profile" that captures not just what the user wants, but *under what conditions*
   * Example: "Use Claude Code for complex multi-file changes, but fall back to minimal subagent if budget is tight or after two failures"
   * Preferences can be conditional, weighted, and context-dependent

2. **Internal resource negotiation**

   * When multiple tasks compete for resources (tokens, time, model access), Advocate negotiates allocation
   * When orchestrator faces tradeoffs (fast vs thorough, cheap vs accurate), Advocate represents user preference
   * Resolve conflicts between project-level and user-level preferences

3. **External representation (v2+)**

   * Interface with external AI services (API providers, other agents) on user's behalf
   * Negotiate rate limits, pricing tiers, resource allocation with external systems
   * Form coalitions with other Advocates for collective bargaining power

4. **Externality pricing (v3)**

   * When one agent's actions affect others (shared repos, competing for CI, noisy neighbors), price and negotiate the externality
   * Enable hyper-granular contracting: "I'll accept a 10-minute delay on my build if you cover 5% of my compute costs"

### Non-Goals (initially)

* **Not a general chatbot**: Advocate doesn't handle arbitrary user conversations—it represents preferences in negotiations
* **Not a planner**: Strategist handles project-level planning; Advocate handles preference representation
* **Not an enforcer**: Sentinel handles safety; Advocate handles negotiation
* **Not a price oracle**: Advocate negotiates prices, not sets them unilaterally

---

## 3. Preference Architecture

### 3.1 Preference Profile

The core data structure Advocate maintains:

```ts
interface PreferenceProfile {
  id: string;
  version: string;
  updatedAt: string;

  // Who this profile represents
  principal: {
    kind: "user" | "project" | "org";
    id: string;
    name?: string;
  };

  // Hierarchical preferences with conditions
  preferences: Preference[];

  // Learned from interaction history
  inferred: InferredPreference[];

  // Explicit constraints (hard limits)
  constraints: Constraint[];

  // Delegation rules (when to defer to others)
  delegations: Delegation[];
}

interface Preference {
  id: string;
  domain: PreferenceDomain;
  description: string;

  // The preference as a conditional rule
  rule: {
    condition?: PreferenceCondition;  // IF this...
    preference: string;                // THEN prefer this
    weight: number;                    // 0-1, importance
    flexibility: "hard" | "soft" | "negotiable";
  };

  // Evidence for why this preference exists
  evidence?: {
    source: "explicit" | "inferred" | "default";
    conversationIds?: string[];
    episodeIds?: string[];
  };
}

type PreferenceDomain =
  | "model_selection"      // Which AI models to use
  | "resource_allocation"  // Budget, tokens, compute
  | "timing"               // Speed vs thoroughness
  | "quality"              // Accuracy vs cost
  | "safety"               // Risk tolerance
  | "privacy"              // Data sharing preferences
  | "style"                // Communication preferences
  | "workflow"             // Process preferences
  | "external_services";   // Third-party integrations

interface PreferenceCondition {
  context?: string[];      // e.g., ["testing", "production"]
  taskType?: string[];     // e.g., ["bug", "feature"]
  timeOfDay?: string;      // e.g., "business_hours"
  budgetRemaining?: { min?: number; max?: number };
  priority?: number[];     // task priority levels
  custom?: string;         // freeform condition
}

interface Constraint {
  id: string;
  description: string;
  rule: string;           // "NEVER do X" or "ALWAYS do Y"
  source: "explicit" | "policy" | "legal";
  enforcementLevel: "block" | "warn" | "log";
}

interface Delegation {
  domain: PreferenceDomain;
  delegateTo: string;      // another agent or role
  conditions?: string;
  scope?: "full" | "advisory";
}
```

### 3.2 Preference Sources

Preferences come from multiple sources, with explicit sources overriding inferred:

1. **Explicit** (highest priority)
   * Direct user statements: "Always use the cheapest model for test runs"
   * Configuration files: `ProjectConfig.advocate.preferences`
   * Interactive calibration sessions

2. **Inferred** (medium priority)
   * Learned from user behavior patterns via Archivist
   * Extracted from conversation history
   * Derived from project conventions

3. **Defaults** (lowest priority)
   * Sensible defaults for each domain
   * Can be overridden by any explicit or inferred preference

### 3.3 Preference Learning

Advocate learns preferences through:

```ts
interface PreferenceLearningEvent {
  kind:
    | "explicit_statement"    // User says "I prefer X"
    | "choice_made"           // User chooses X over Y
    | "correction"            // User corrects agent behavior
    | "satisfaction_signal"   // Positive/negative feedback
    | "pattern_detected";     // Archivist detects pattern

  domain: PreferenceDomain;
  context: string;
  observation: string;
  inferencedPreference?: string;
  confidence: number;  // 0-1
}
```

---

## 4. Negotiation Framework

### 4.1 Negotiation Types

Advocate handles several types of negotiations:

**Internal negotiations (v1)**
- Resource allocation between tasks
- Model selection tradeoffs
- Timing and priority conflicts
- Budget distribution

**External negotiations (v2)**
- API rate limit management
- Service tier selection
- Multi-agent resource sharing
- Coalition formation

**Ecosystem negotiations (v3)**
- Externality pricing
- Democratic vs economic mode switching
- Constitutional preference enforcement

### 4.2 Negotiation Protocol

```ts
interface Negotiation {
  id: string;
  kind: "allocation" | "tradeoff" | "coalition" | "externality";
  status: "pending" | "active" | "resolved" | "failed" | "escalated";

  // Parties involved
  parties: NegotiationParty[];

  // What's being negotiated
  subject: {
    resource?: string;       // e.g., "compute_budget"
    tradeoff?: string;       // e.g., "speed_vs_cost"
    externality?: string;    // e.g., "ci_queue_time"
    coalition?: string;      // e.g., "bulk_api_discount"
  };

  // Positions and offers
  positions: Position[];
  offers: Offer[];

  // Resolution
  outcome?: NegotiationOutcome;
}

interface NegotiationParty {
  id: string;
  kind: "advocate" | "orchestrator" | "external_agent" | "service";
  preferences: Preference[];  // relevant subset
  constraints: Constraint[];
  bidCapacity?: number;       // for economic negotiations
  votingWeight?: number;      // for democratic negotiations
}

interface Offer {
  fromParty: string;
  timestamp: string;
  terms: Record<string, unknown>;
  justification: string;
}

interface NegotiationOutcome {
  status: "agreement" | "no_deal" | "escalated";
  terms?: Record<string, unknown>;
  transfers?: Transfer[];     // compensation/payments
  commitments?: Commitment[]; // promises for future behavior
}
```

### 4.3 Democratic vs Economic Modes

Following the Coasean paper's insight that not all negotiations should be wealth-weighted:

```ts
type NegotiationMode = "economic" | "democratic" | "hybrid";

interface ModeSelection {
  subject: string;
  defaultMode: NegotiationMode;
  criteria: {
    // Use democratic mode when:
    democratic: string[];  // e.g., ["affects_community_values", "fundamental_rights"]
    // Use economic mode when:
    economic: string[];    // e.g., ["commercial_activity", "private_resource"]
  };
}
```

**Economic mode**: Outcomes determined by willingness-to-pay. Used for commercial activities, resource allocation, efficiency optimization.

**Democratic mode**: Each party gets equal weight regardless of resources. Used for community values, constitutional preferences, fundamental rights.

---

## 5. Coalition Formation

### 5.1 Coalition Types

Advocates can form coalitions for collective bargaining:

```ts
interface Coalition {
  id: string;
  name: string;
  purpose: string;

  members: CoalitionMember[];

  // Aggregated preferences for the coalition
  sharedPreferences: Preference[];

  // Rules for how the coalition operates
  governance: {
    decisionRule: "unanimous" | "majority" | "weighted";
    exitConditions: string[];
    disputeResolution: string;
  };

  // What the coalition negotiates for
  mandate: {
    subjects: string[];
    constraints: string[];
    delegation: "full" | "advisory";
  };
}

interface CoalitionMember {
  advocateId: string;
  joinedAt: string;
  weight?: number;
  delegatedDomains: PreferenceDomain[];
}
```

### 5.2 Coalition Use Cases

**Near-term (v1)**:
- Multiple MechaCoder instances coordinating on shared repo
- Project teams aggregating preferences for model selection

**Medium-term (v2)**:
- User groups negotiating bulk API discounts
- Cross-project coalitions for shared infrastructure

**Long-term (v3)**:
- Neighborhood-style coalitions pricing local externalities
- Democratic coalitions for policy decisions

---

## 6. Implementation Phases

### Phase A1: Preference Infrastructure (v1)

**Goal**: Build the preference representation layer that other agents can query.

1. **oa-advocate-01 – Design preference schema**
   * Create `src/advocate/schema.ts` with PreferenceProfile, Preference, Constraint
   * Add `AdvocateConfig` to ProjectConfig

2. **oa-advocate-02 – Implement PreferenceService**
   * JSONL-based storage in `.openagents/advocate/preferences.jsonl`
   * API: `getPreferences(domain)`, `updatePreference()`, `resolvePreference(context)`

3. **oa-advocate-03 – Wire into orchestrator**
   * Orchestrator queries Advocate for model selection preferences
   * Budget allocation respects Advocate preferences
   * Simple rule-based preference resolution

4. **oa-advocate-04 – Preference learning from Archivist**
   * When Archivist records memories about user preferences, Advocate extracts them
   * Confidence-weighted inference

### Phase A2: Internal Negotiation (v1)

**Goal**: Enable negotiation within the MechaCoder ecosystem.

5. **oa-advocate-05 – Design negotiation protocol**
   * Create `src/advocate/negotiation.ts` with Negotiation, Offer, Outcome types
   * Simple offer/counter-offer protocol

6. **oa-advocate-06 – Resource allocation negotiation**
   * When multiple tasks compete for budget, Advocate negotiates allocation
   * Tradeoff resolution (e.g., "use cheaper model to fit more tasks")

7. **oa-advocate-07 – Multi-principal support**
   * Support both user-level and project-level preferences
   * Conflict resolution rules when they disagree

8. **oa-advocate-08 – HUD integration**
   * Show current preferences in HUD
   * Visualize negotiations and outcomes

### Phase A3: External Representation (v2)

**Goal**: Interface with external systems on user's behalf.

9. **oa-advocate-09 – External agent protocol**
   * Define message format for inter-agent negotiation
   * Privacy-preserving preference communication (reveal preferences without revealing underlying data)

10. **oa-advocate-10 – API provider negotiation**
    * Rate limit management: when hitting limits, negotiate across projects
    * Model tier selection based on preferences

11. **oa-advocate-11 – Coalition formation (basic)**
    * Allow multiple Advocates to form temporary coalitions
    * Aggregated preference expression

### Phase A4: Coasean Features (v3)

**Goal**: Full externality pricing and democratic governance.

12. **oa-advocate-12 – Externality detection**
    * Detect when one agent's actions create externalities for others
    * Example: one project hogging CI resources affects others

13. **oa-advocate-13 – Externality pricing**
    * Calculate "price" for externalities based on affected parties' preferences
    * Enable compensation transfers

14. **oa-advocate-14 – Democratic/economic mode switching**
    * Implement mode selection based on subject matter
    * Voting mechanisms for democratic mode

15. **oa-advocate-15 – Constitutional preferences**
    * Hard constraints that represent "non-negotiable" values
    * Integration with Sentinel for enforcement

---

## 7. Module Structure

```text
src/advocate/
├── schema.ts           # PreferenceProfile, Preference, Constraint, Negotiation
├── service.ts          # AdvocateService: main entry point
├── preference/
│   ├── store.ts        # JSONL storage for preferences
│   ├── resolver.ts     # Resolve preferences given context
│   ├── learner.ts      # Infer preferences from behavior
│   └── calibrator.ts   # Interactive preference calibration
├── negotiation/
│   ├── protocol.ts     # Negotiation types and flow
│   ├── internal.ts     # Within-ecosystem negotiations
│   ├── external.ts     # Cross-agent negotiations (v2)
│   └── coalition.ts    # Coalition formation and management
├── modes/
│   ├── economic.ts     # Willingness-to-pay based negotiation
│   └── democratic.ts   # Equal-weight voting
├── hud.ts              # HUD integration helpers
└── __tests__/
    ├── preference.test.ts
    ├── negotiation.test.ts
    └── coalition.test.ts
```

---

## 8. Config

Extend `ProjectConfig` with:

```ts
interface AdvocateConfig {
  enabled: boolean;                    // default true

  // Preference management
  preferences: {
    sources: ("explicit" | "inferred" | "default")[];
    inferenceEnabled: boolean;         // learn from behavior
    calibrationPromptOnStart: boolean; // ask user preferences on first run
  };

  // Negotiation settings
  negotiation: {
    enabled: boolean;
    autoResolve: boolean;              // auto-resolve simple conflicts
    escalateThreshold: number;         // confidence threshold for escalation
    defaultMode: "economic" | "democratic";
  };

  // External integration (v2+)
  external?: {
    allowExternalNegotiation: boolean;
    trustedAgents: string[];           // agent IDs we'll negotiate with
    privacyLevel: "full" | "partial" | "minimal";
  };

  // Coalition settings (v2+)
  coalition?: {
    autoJoin: boolean;                 // auto-join beneficial coalitions
    maxCoalitions: number;
  };
}
```

---

## 9. HUD Integration

Add HUD message types for Advocate:

```ts
| { type: "advocate_preference_updated";
    domain: PreferenceDomain;
    summary: string;
    ts: string;
  }
| { type: "advocate_negotiation_start";
    negotiationId: string;
    subject: string;
    parties: string[];
    ts: string;
  }
| { type: "advocate_negotiation_resolved";
    negotiationId: string;
    outcome: "agreement" | "no_deal" | "escalated";
    summary: string;
    ts: string;
  }
| { type: "advocate_coalition_formed";
    coalitionId: string;
    memberCount: number;
    purpose: string;
    ts: string;
  }
```

HUD panels:
- **Preferences panel**: Current active preferences by domain
- **Negotiations panel**: Active and recent negotiations with outcomes
- **Coalitions panel** (v2): Current coalition memberships

---

## 10. Integration Points

### With Orchestrator
- Query preferences before model selection
- Request budget allocation decisions
- Report tradeoffs for negotiation

### With Archivist
- Extract preferences from memory entries
- Store negotiation outcomes as memories
- Learn from historical preference patterns

### With Quartermaster
- Coordinate on budget preferences
- Joint optimization of cost vs quality

### With Strategist
- Align project-level planning with user preferences
- Priority recommendations respect preferences

### With Sentinel
- Constitutional preferences become safety constraints
- Hard constraints are enforced, not negotiated

---

## 11. The Long View: Toward Coasean Coordination

The ultimate vision—aligned with the research paper—is a world where:

1. **Every MechaCoder user has an Advocate** that knows their preferences with nuance
2. **Advocates negotiate continuously** to resolve conflicts without centralized control
3. **Externalities are priced** rather than ignored or banned
4. **Coalitions form spontaneously** when collective bargaining is beneficial
5. **Democratic and economic modes coexist** based on the subject matter
6. **Privacy is preserved** through zero-knowledge preference proofs

This transforms the agent ecosystem from a collection of independent tools into a **coordinated economy** where:
- Transaction costs approach zero
- Preferences are expressed with precision
- Conflicts are resolved through bargaining, not fiat
- Individual agency is amplified, not diminished

The Advocate is the key agent in this vision: your tireless, competent digital representative—starting with your MechaCoder preferences today, and scaling to represent you in the broader AI agent economy tomorrow.

---

## 12. Open Questions

1. **How do we bootstrap preferences?** Initial calibration UX needs design.
2. **What's the privacy model?** How much preference info do we reveal in negotiations?
3. **How do we handle bad-faith actors?** Reputation systems? Escrow?
4. **What's the relationship to existing "preference learning" research?** Constitutional AI, RLHF, etc.
5. **How do we avoid the "tyranny of the majority" in democratic mode?** Constitutional constraints?
6. **What happens when preferences conflict with safety?** Sentinel integration.

---

## References

- [Coasean Bargaining at Scale](../research/coasean-bargaining-at-scale.md) - The foundational vision
- [Generative Agents](https://arxiv.org/abs/2304.03442) - Memory architecture inspiration
- [Mechanism Design](https://en.wikipedia.org/wiki/Mechanism_design) - Game-theoretic foundations
- [Harberger Tax](https://en.wikipedia.org/wiki/Harberger_Tax) - Anti-holdout mechanisms
- [Liquid Democracy](https://en.wikipedia.org/wiki/Liquid_democracy) - Delegation models
