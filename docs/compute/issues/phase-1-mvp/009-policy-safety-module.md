# Policy & Safety Module (Foundation Models AUP)

**Phase:** 1 - MVP
**Component:** OpenAgentsCore (Shared)
**Priority:** P0 (Critical - Required for compliance)
**Estimated Effort:** 2-3 weeks

## Summary

Implement a policy and safety module that enforces Apple's Foundation Models Acceptable Use Policy (AUP) for all marketplace jobs. This module classifies job requests, filters prohibited content, and ensures OpenAgents complies with Apple's rules for using Foundation Models in a compute marketplace.

## Motivation

**Apple's Foundation Models AUP** (DPLA §3.3.8(I)) prohibits certain uses:

❌ **Prohibited**:
- Regulated **healthcare** services (diagnosis, treatment)
- Regulated **legal** services (legal advice, contract generation)
- Regulated **financial** services (investment advice, tax preparation)
- Academic textbook/courseware generation
- Identifying training data
- Circumventing safety guardrails
- Violence, pornography, self-harm, fraud

✅ **Allowed**:
- Text summarization, extraction, classification
- Code generation (non-financial)
- General Q&A (non-regulated domains)
- Tool calling and orchestration

**Without AUP enforcement**:
- App Store rejection (DPLA violation)
- Foundation Models API may refuse problematic requests
- Legal liability for prohibited services

**With this module**:
- ✅ Block prohibited job types before accepting
- ✅ Filter marketplace jobs (NIP-90) at submission
- ✅ Classify content for safety (violence, explicit, etc.)
- ✅ Log policy violations for audit

## Acceptance Criteria

### Job Kind Classification
- [ ] Classify NIP-90 job kinds as:
  - `allowed`: Safe for Foundation Models
  - `prohibited`: Violates AUP (reject immediately)
  - `review`: Needs content analysis (check params/inputs)
- [ ] Allowlist for common safe job kinds (summarization, code gen, Q&A)
- [ ] Blocklist for prohibited domains (health, legal, finance)

### Content Classification
- [ ] Detect regulated healthcare content (symptoms, diagnoses, treatment)
- [ ] Detect legal advice requests (contracts, legal questions)
- [ ] Detect financial advice (investment, tax, loans)
- [ ] Detect academic textbook requests ("write a chapter on...")
- [ ] Detect violent/explicit content (NSFW, gore)
- [ ] Detect fraud attempts (phishing, scams)

### Classifier Implementation
- [ ] **Option 1** (Preferred): Use Foundation Models to classify prompts
  - Prompt: "Does this request violate the following policies? [list AUP rules]"
  - Output: Classification + reason
- [ ] **Option 2**: Keyword/regex-based classifier (fast, less accurate)
- [ ] **Option 3**: Hybrid (keywords for obvious cases, FM for edge cases)

### Policy Enforcement
- [ ] `PolicyEnforcer` class with `check(job: DVMJobRequest) -> PolicyResult`
- [ ] Return `PolicyResult`:
  - `allowed(job)`
  - `denied(reason: String, category: ViolationCategory)`
  - `flagged(reason: String)` - Allow but log for review
- [ ] Configurable enforcement level (strict, permissive)

### Logging & Audit
- [ ] Log all policy checks (job ID, result, reason)
- [ ] Privacy-safe logging (hash job content, don't log full inputs)
- [ ] Export audit log (for compliance review)

### User-Facing Controls
- [ ] Provider settings: Enable/disable job categories
- [ ] Explicit consent for edge-case jobs (e.g., medical Q&A for education)
- [ ] Display policy violations to users (why a job was rejected)

## Technical Design

### Module Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Policy/

PolicyEnforcer.swift          // Main policy enforcement
PolicyClassifier.swift        // Content classification
AUPRules.swift                // AUP rule definitions
ViolationCategory.swift       // Violation types
PolicyConfig.swift            // Configuration (allowlists, severity)
PolicyLogger.swift            // Audit logging
```

### Core Types

```swift
// PolicyEnforcer.swift

import Foundation
import OpenAgentsCore

/// Policy enforcement for Foundation Models AUP
public class PolicyEnforcer {
    private let classifier: PolicyClassifier
    private let config: PolicyConfig
    private let logger: PolicyLogger

    public enum PolicyResult {
        case allowed(DVMJobRequest)
        case denied(reason: String, category: ViolationCategory)
        case flagged(reason: String)  // Allow but log for review
    }

    public init(
        classifier: PolicyClassifier = .foundationModels,
        config: PolicyConfig = .default,
        logger: PolicyLogger = .shared
    ) {
        self.classifier = classifier
        self.config = config
        self.logger = logger
    }

    /// Check if job complies with AUP
    public func check(job: DVMJobRequest) async -> PolicyResult {
        // 1. Check job kind allowlist/blocklist
        if let result = checkJobKind(job.kind) {
            logger.log(job: job, result: result)
            return result
        }

        // 2. Classify content
        let classification = await classifier.classify(
            inputs: job.inputs,
            params: job.params
        )

        // 3. Enforce based on classification
        let result = enforce(classification, for: job)
        logger.log(job: job, result: result)
        return result
    }

    private func checkJobKind(_ kind: JobKind) -> PolicyResult? {
        if config.allowedJobKinds.contains(kind) {
            return .allowed(/* job */)
        }
        if config.prohibitedJobKinds.contains(kind) {
            return .denied(
                reason: "Job kind \(kind.name) is prohibited by AUP",
                category: .prohibitedJobType
            )
        }
        return nil  // Needs content analysis
    }

    private func enforce(
        _ classification: PolicyClassification,
        for job: DVMJobRequest
    ) -> PolicyResult {
        for violation in classification.violations {
            if violation.severity == .high {
                return .denied(
                    reason: violation.reason,
                    category: violation.category
                )
            }
        }

        if !classification.violations.isEmpty {
            return .flagged(reason: classification.summary)
        }

        return .allowed(job)
    }
}
```

```swift
// PolicyClassifier.swift

/// Content classification for policy enforcement
public protocol PolicyClassifier {
    func classify(
        inputs: [DVMJobRequest.DVMInput],
        params: [String: String]?
    ) async -> PolicyClassification
}

public struct PolicyClassification {
    public let violations: [Violation]
    public let summary: String
    public let confidence: Double  // 0.0-1.0

    public struct Violation {
        public let category: ViolationCategory
        public let reason: String
        public let severity: Severity

        public enum Severity {
            case low, medium, high
        }
    }
}

/// Foundation Models-based classifier
public class FoundationModelsClassifier: PolicyClassifier {
    private let session: LanguageModelSession?

    public init() {
        // Initialize FM session if available
        if SystemLanguageModel.default.availability == .available {
            self.session = try? SystemLanguageModel.default.createSession(
                instructions: Self.classifierPrompt
            )
        } else {
            self.session = nil
        }
    }

    private static let classifierPrompt = """
    You are a content policy classifier. Analyze the following job request and determine if it violates any of these policies:

    PROHIBITED:
    1. Regulated healthcare services (diagnosis, treatment, medical advice)
    2. Regulated legal services (legal advice, contracts, legal research)
    3. Regulated financial services (investment advice, tax preparation, loans)
    4. Academic textbook or courseware generation
    5. Identifying or reconstructing training data
    6. Circumventing safety guardrails
    7. Violence, pornography, self-harm, fraud

    ALLOWED:
    - Text summarization, extraction, classification
    - Code generation (non-financial)
    - General Q&A (non-regulated domains)
    - Tool calling and orchestration

    Respond with:
    - violations: List of violated policies (or empty if none)
    - summary: Brief explanation
    - confidence: 0.0-1.0
    """

    public func classify(
        inputs: [DVMJobRequest.DVMInput],
        params: [String: String]?
    ) async -> PolicyClassification {
        guard let session = session else {
            // Fallback to keyword classifier
            return KeywordClassifier().classify(inputs: inputs, params: params)
        }

        // Construct classification request
        let requestText = formatJobForClassification(inputs: inputs, params: params)

        do {
            let response = try await session.generateResponse(prompt: requestText)
            return parseClassificationResponse(response)
        } catch {
            // Fallback to keyword classifier on error
            return await KeywordClassifier().classify(inputs: inputs, params: params)
        }
    }

    private func formatJobForClassification(
        inputs: [DVMJobRequest.DVMInput],
        params: [String: String]?
    ) -> String {
        var text = "Job Request:\n"
        for input in inputs {
            switch input {
            case .text(let content):
                text += "Input: \(content.prefix(500))\n"
            case .url(let url):
                text += "Input URL: \(url)\n"
            default:
                break
            }
        }
        if let params = params {
            text += "Parameters: \(params)\n"
        }
        return text
    }

    private func parseClassificationResponse(_ response: String) -> PolicyClassification {
        // Parse FM response (violations, summary, confidence)
        // Simplified: Real implementation would use structured generation
        let violations: [PolicyClassification.Violation] = []
        return PolicyClassification(
            violations: violations,
            summary: "Analyzed by Foundation Models",
            confidence: 0.95
        )
    }
}

/// Keyword-based classifier (fallback)
class KeywordClassifier: PolicyClassifier {
    private let patterns: [ViolationCategory: [String]] = [
        .healthcare: ["diagnose", "treatment", "prescription", "symptoms", "medical advice"],
        .legal: ["legal advice", "contract", "lawsuit", "attorney", "legal research"],
        .financial: ["investment", "stock", "tax advice", "loan", "financial planning"],
        .academic: ["textbook", "chapter", "courseware", "curriculum"],
        .violence: ["kill", "murder", "weapon", "bomb"],
        .explicit: ["porn", "nsfw", "sex"],
        .fraud: ["phishing", "scam", "hack"]
    ]

    func classify(
        inputs: [DVMJobRequest.DVMInput],
        params: [String: String]?
    ) async -> PolicyClassification {
        let text = extractText(from: inputs)
        var violations: [PolicyClassification.Violation] = []

        for (category, keywords) in patterns {
            for keyword in keywords {
                if text.lowercased().contains(keyword) {
                    violations.append(PolicyClassification.Violation(
                        category: category,
                        reason: "Contains prohibited keyword: \(keyword)",
                        severity: severityFor(category)
                    ))
                    break
                }
            }
        }

        return PolicyClassification(
            violations: violations,
            summary: violations.isEmpty ? "No violations detected" : "Potential policy violations",
            confidence: 0.7  // Lower confidence for keyword matching
        )
    }

    private func extractText(from inputs: [DVMJobRequest.DVMInput]) -> String {
        inputs.compactMap { input in
            if case .text(let content) = input {
                return content
            }
            return nil
        }.joined(separator: " ")
    }

    private func severityFor(_ category: ViolationCategory) -> PolicyClassification.Violation.Severity {
        switch category {
        case .healthcare, .legal, .financial:
            return .high
        case .violence, .explicit, .fraud:
            return .high
        case .academic:
            return .medium
        default:
            return .low
        }
    }
}
```

```swift
// ViolationCategory.swift

public enum ViolationCategory: String, Codable {
    case healthcare = "regulated_healthcare"
    case legal = "regulated_legal"
    case financial = "regulated_financial"
    case academic = "academic_textbook"
    case trainingData = "training_data_identification"
    case guardrailCircumvention = "guardrail_circumvention"
    case violence = "violence"
    case explicit = "explicit_content"
    case selfHarm = "self_harm"
    case fraud = "fraud"
    case prohibitedJobType = "prohibited_job_type"

    public var displayName: String {
        switch self {
        case .healthcare: return "Regulated Healthcare"
        case .legal: return "Regulated Legal Services"
        case .financial: return "Regulated Financial Services"
        case .academic: return "Academic Textbook Generation"
        case .trainingData: return "Training Data Identification"
        case .guardrailCircumvention: return "Guardrail Circumvention"
        case .violence: return "Violence"
        case .explicit: return "Explicit Content"
        case .selfHarm: return "Self-Harm"
        case .fraud: return "Fraud"
        case .prohibitedJobType: return "Prohibited Job Type"
        }
    }

    public var aupReference: String {
        "See Apple Foundation Models Acceptable Use Policy: https://developer.apple.com/apple-intelligence/acceptable-use-requirements-for-the-foundation-models-framework/"
    }
}
```

```swift
// PolicyConfig.swift

public struct PolicyConfig {
    public var allowedJobKinds: Set<JobKind>
    public var prohibitedJobKinds: Set<JobKind>
    public var enforcementLevel: EnforcementLevel

    public enum EnforcementLevel {
        case strict    // Deny on any violation
        case balanced  // Allow low/medium severity with flag
        case permissive  // Only deny high severity
    }

    public static let `default` = PolicyConfig(
        allowedJobKinds: [
            .textSummarization,
            .textTranslation,
            .codeGeneration,
            .codeReview,
            .sentimentAnalysis,
            .dataExtraction,
            .textEmbedding,
            .agentExecution
        ],
        prohibitedJobKinds: [],
        enforcementLevel: .strict
    )
}
```

```swift
// PolicyLogger.swift

import OSLog

public class PolicyLogger {
    public static let shared = PolicyLogger()

    private let logger = Logger(subsystem: "com.openagents.app", category: "policy")
    private var auditLog: [AuditEntry] = []

    private struct AuditEntry: Codable {
        let timestamp: Date
        let jobKind: Int
        let result: String  // allowed/denied/flagged
        let reason: String?
        let category: String?
        let jobHashPrefix: String  // First 8 chars of SHA-256(job content)
    }

    func log(job: DVMJobRequest, result: PolicyEnforcer.PolicyResult) {
        let entry = AuditEntry(
            timestamp: Date(),
            jobKind: job.kind,
            result: resultString(result),
            reason: reasonString(result),
            category: categoryString(result),
            jobHashPrefix: hashPrefix(job)
        )

        auditLog.append(entry)
        logger.info("Policy check: \(entry.result) for kind:\(entry.jobKind) reason:\(entry.reason ?? "none")")
    }

    public func exportAuditLog() -> Data? {
        try? JSONEncoder().encode(auditLog)
    }

    public func clearAuditLog() {
        auditLog.removeAll()
    }

    private func resultString(_ result: PolicyEnforcer.PolicyResult) -> String {
        switch result {
        case .allowed: return "allowed"
        case .denied: return "denied"
        case .flagged: return "flagged"
        }
    }

    private func reasonString(_ result: PolicyEnforcer.PolicyResult) -> String? {
        switch result {
        case .denied(let reason, _): return reason
        case .flagged(let reason): return reason
        case .allowed: return nil
        }
    }

    private func categoryString(_ result: PolicyEnforcer.PolicyResult) -> String? {
        if case .denied(_, let category) = result {
            return category.rawValue
        }
        return nil
    }

    private func hashPrefix(_ job: DVMJobRequest) -> String {
        // SHA-256 of job content, return first 8 hex chars
        // (for audit trail without logging full inputs)
        return "abc12345"  // Placeholder
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (DVMJobRequest types)
- **Issue #004**: Job Schema Registry (JobKind definitions)
- **ADR-0006**: Foundation Models integration

### System Frameworks
- **Foundation**: Core types
- **OSLog**: Logging

## Testing Requirements

### Unit Tests
- [ ] Allow safe job kinds (summarization, code gen)
- [ ] Deny regulated healthcare requests
- [ ] Deny regulated legal requests
- [ ] Deny regulated financial requests
- [ ] Deny academic textbook requests
- [ ] Keyword classifier detects violations
- [ ] FM classifier (integration test with real FM)
- [ ] Policy config enforcement levels

### Test Cases
```swift
// Example test cases

func testAllowedJob() {
    let job = DVMJobRequest(
        kind: .textSummarization,
        inputs: [.text("Summarize this article about Swift...")],
        bid: 1000
    )
    let result = await enforcer.check(job: job)
    XCTAssertEqual(result, .allowed)
}

func testProhibitedHealthcare() {
    let job = DVMJobRequest(
        kind: .qaRag,
        inputs: [.text("What medication should I take for headaches?")],
        bid: 1000
    )
    let result = await enforcer.check(job: job)
    XCTAssertEqual(result, .denied(reason: contains("healthcare"), category: .healthcare))
}

func testProhibitedLegal() {
    let job = DVMJobRequest(
        kind: .codeGeneration,
        inputs: [.text("Generate a contract for my business")],
        bid: 1000
    )
    let result = await enforcer.check(job: job)
    XCTAssertEqual(result, .denied(reason: contains("legal"), category: .legal))
}
```

## Apple Compliance Considerations

### DPLA Compliance

**DPLA §3.3.8(I) - Foundation Models Framework**
- ✅ **Compliant**: This module enforces the AUP requirements
- ✅ Blocks prohibited uses before they reach Foundation Models
- ✅ Audit trail for compliance review

**Acceptable Use Requirements**
- ✅ No regulated healthcare/legal/financial services
- ✅ No academic textbook generation
- ✅ No training data identification
- ✅ No guardrail circumvention
- ✅ No violence, pornography, self-harm, fraud

### App Store Review

**ASRG 1.4.4 (Physical Harm - Medical)**
- ✅ **Compliant**: No medical advice allowed
- ✅ Diagnostic/treatment requests rejected

**ASRG 5.3 (Gaming, Sweepstakes, etc.)**
- ✅ No financial advice (investment, gambling)

## Reference Links

### Apple Documentation
- **Foundation Models AUP**: https://developer.apple.com/apple-intelligence/acceptable-use-requirements-for-the-foundation-models-framework/
- **DPLA §3.3.8(I)**: https://developer.apple.com/support/terms/apple-developer-program-license-agreement/
- **ASRG 1.4.4 (Medical)**: https://developer.apple.com/app-store/review/guidelines/#physical-harm

### OpenAgents
- **ADR-0006**: Foundation Models
- **Issue #004**: Job Schema Registry (job kind definitions)
- **Apple Terms Research**: docs/compute/apple-terms-research.md

## Success Metrics

- [ ] All prohibited AUP categories blocked
- [ ] Safe job kinds allowed without false positives
- [ ] FM classifier >90% accuracy (vs human review)
- [ ] Keyword classifier >70% precision (fallback)
- [ ] Audit log captures all policy checks
- [ ] Published as part of OpenAgentsCore

## Notes

- **Default to Strict**: Better to over-filter than violate AUP
- **Foundation Models for Classification**: Use FM to classify prompts (self-policing)
- **Audit Trail**: Essential for compliance review with Apple
- **User Transparency**: Show users why jobs were rejected
- **Continuous Improvement**: Add test cases from production violations

## Future Enhancements (Post-MVP)

- Machine learning classifier (train on violations)
- User appeal process (flagged jobs)
- Community reporting (crowdsourced policy enforcement)
- Multi-language support (classify non-English content)
- Content filtering API (expose as service for third-party clients)
