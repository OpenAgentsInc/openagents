# Issue #006: Implement UpgradeExecutor

**Component**: Upgrade System
**Priority**: P1 (High)
**Estimated Effort**: 4-5 days
**Dependencies**: #002 (DecisionOrchestrator), #004 (AgentCoordinator)
**Assignee**: TBD

---

## Overview

Load, validate, and execute upgrade manifests. Implements pipeline executor with operations registry supporting declarative JSON workflows.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Upgrades/UpgradeExecutor.swift`

---

## Requirements

1. Load manifest from JSON file (with validation)
2. Execute pipeline operations sequentially
3. Variable substitution (e.g., `{session.analyze.insights}`)
4. Operations registry for known ops
5. Permission checking via PolicyEnforcer

---

## Implementation

```swift
actor UpgradeExecutor {
    private let operationsRegistry: OperationsRegistry
    private let policyEnforcer: PolicyEnforcer

    init() {
        self.operationsRegistry = OperationsRegistry()
        self.policyEnforcer = PolicyEnforcer()
    }

    func load(_ manifestPath: URL) async throws -> UpgradeManifest {
        let data = try Data(contentsOf: manifestPath)
        let manifest = try JSONDecoder().decode(UpgradeManifest.self, from: data)
        try await validate(manifest)
        return manifest
    }

    func validate(_ manifest: UpgradeManifest) async throws {
        // Check schema version
        // Validate cron expression
        // Validate permissions
        // Check all ops are known
        for op in manifest.pipeline {
            guard operationsRegistry.has(op.op) else {
                throw UpgradeError.unknownOperation(op.op)
            }
        }
    }

    func execute(_ pipeline: [UpgradeOperation], context: ExecutionContext) async throws -> ExecutionResult {
        var ctx = context
        let startTime = Date()

        for op in pipeline {
            // Check permissions
            guard await policyEnforcer.isAllowed(op, permissions: manifest.permissions) else {
                throw UpgradeError.permissionDenied(op.op)
            }

            // Execute operation
            let handler = operationsRegistry.handler(for: op.op)
            let result = try await handler(op, ctx)

            // Store output variable
            if let outputVar = op.output_var {
                ctx.variables[outputVar] = result
            }

            // Check time budget
            let elapsed = Date().timeIntervalSince(startTime)
            if elapsed > ctx.timeBudget {
                throw UpgradeError.timeBudgetExceeded
            }
        }

        return ExecutionResult(
            success: true,
            outputs: ctx.variables,
            duration: Date().timeIntervalSince(startTime),
            error: nil
        )
    }
}

struct UpgradeManifest: Codable {
    let id: String
    let version: String
    let schedule: UpgradeSchedule
    let permissions: UpgradePermissions
    let pipeline: [UpgradeOperation]
}

struct UpgradeOperation: Codable {
    let op: String
    let params: [String: JSONValue]?
    let backend: String?
    let output_var: String?
}

struct ExecutionContext {
    var variables: [String: JSONValue]
    let workingDir: URL
    let timeBudget: TimeInterval
}
```

### Operations Registry

```swift
actor OperationsRegistry {
    private var handlers: [String: OpHandler] = [:]

    init() {
        registerBuiltInOps()
    }

    private func registerBuiltInOps() {
        register("session.analyze") { op, ctx in
            let analyzer = SessionHistoryAnalyzer.shared
            let insights = try await analyzer.analyze(/* params from op */)
            return JSONValue.object(insights.toDictionary())
        }

        register("orchestrate.decide") { op, ctx in
            let orchestrator = DecisionOrchestrator.shared
            let decision = try await orchestrator.decideNextTask(/* context from ctx */)
            return JSONValue.object(decision.toDictionary())
        }

        register("agent.execute") { op, ctx in
            let coordinator = AgentCoordinator.shared
            let result = try await coordinator.delegate(/* task from op */)
            return JSONValue.object(result.toDictionary())
        }

        register("pr.create") { op, ctx in
            let prService = PRAutomationService.shared
            let prNumber = try await prService.createPR(/* params from op */)
            return JSONValue.number(Double(prNumber))
        }
    }

    func register(_ name: String, handler: @escaping OpHandler) {
        handlers[name] = handler
    }

    func handler(for name: String) -> OpHandler {
        handlers[name]!
    }

    func has(_ name: String) -> Bool {
        handlers[name] != nil
    }
}

typealias OpHandler = (UpgradeOperation, ExecutionContext) async throws -> JSONValue
```

---

## Testing

1. `testLoadManifest()` - Parse valid JSON
2. `testValidation()` - Reject invalid manifests
3. `testExecutePipeline()` - Sequential ops with variable substitution
4. `testPermissionChecking()` - Denied ops fail
5. `testTimeBudgetEnforcement()` - Stop after timeout

---

## Acceptance Criteria

- [ ] Loads and validates manifest JSON
- [ ] Executes all built-in operations
- [ ] Variable substitution works
- [ ] Permission checks enforced
- [ ] Time budget enforced
- [ ] Tests pass (≥90% coverage)

---

## References

- docs/compute/issues/upgrades.md
- private/20251108-upgrades-convo/01.md
