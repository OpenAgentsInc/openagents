# macOS: Bidding Engine

**Phase:** 2 - Payments
**Component:** macOS App
**Priority:** P1 (High - Enables pricing)
**Estimated Effort:** 2 weeks

## Summary

Implement a bidding engine for the macOS worker that evaluates incoming job requests, calculates pricing, and decides whether to accept jobs based on bid amount, resource availability, and profitability.

## Motivation

Providers need intelligent bid management:
- **Accept profitable jobs**: Reject bids below cost
- **Dynamic pricing**: Adjust based on demand, resource usage
- **Prioritize high-value work**: Queue optimization
- **Resource awareness**: Don't accept jobs when overloaded

## Acceptance Criteria

### Bid Evaluation
- [ ] Calculate minimum acceptable bid for job (cost model)
- [ ] Compare job bid vs minimum
- [ ] Accept if bid >= minimum
- [ ] Reject with `payment-required` feedback (include BOLT11 in Phase 2)
- [ ] Priority scoring (higher bid = higher priority in queue)

### Cost Model
- [ ] Base cost per job kind (fixed overhead)
- [ ] Variable cost (per token, per second, per MB)
- [ ] Resource cost (CPU, memory, thermal state)
- [ ] Opportunity cost (queue backlog)

### Dynamic Pricing
- [ ] Price floor configuration (min acceptable price)
- [ ] Surge pricing (increase when busy)
- [ ] Discount pricing (decrease when idle)
- [ ] Time-of-day pricing (optional)

### Configuration
- [ ] Base price per job kind (msats)
- [ ] Variable pricing factors (enabled/disabled)
- [ ] Surge multiplier (e.g., 2x when queue > 5 jobs)
- [ ] Discount percentage (e.g., 20% off when idle)

## Technical Design

```swift
// BiddingEngine.swift

struct BiddingEngine {
    let config: BiddingConfig

    struct BiddingConfig {
        var basePricePerKind: [JobKind: Int64]  // msats
        var variableCostPerToken: Int64 = 1     // msats/token
        var surgePricingEnabled: Bool = true
        var surgeMultiplier: Double = 2.0       // 2x when busy
        var surgeThreshold: Int = 5             // queue depth
    }

    /// Evaluate job bid
    func evaluate(
        job: DVMJobRequest,
        queueDepth: Int,
        resourceState: ResourceState
    ) -> BidDecision {
        let minimumBid = calculateMinimumBid(
            job: job,
            queueDepth: queueDepth,
            resourceState: resourceState
        )

        guard let jobBid = job.bid else {
            return .reject(reason: "No bid provided", minimumBid: minimumBid)
        }

        if jobBid >= minimumBid {
            let priority = calculatePriority(jobBid: jobBid, minimumBid: minimumBid)
            return .accept(priority: priority)
        } else {
            return .reject(reason: "Bid too low", minimumBid: minimumBid)
        }
    }

    enum BidDecision {
        case accept(priority: Int)  // 0-100, higher = more urgent
        case reject(reason: String, minimumBid: Int64)
    }

    private func calculateMinimumBid(
        job: DVMJobRequest,
        queueDepth: Int,
        resourceState: ResourceState
    ) -> Int64 {
        // Base price
        var price = config.basePricePerKind[job.kind] ?? 1000

        // Surge pricing (when busy)
        if config.surgePricingEnabled && queueDepth >= config.surgeThreshold {
            price = Int64(Double(price) * config.surgeMultiplier)
        }

        // Resource cost (thermal throttling, high CPU)
        if resourceState.thermalState >= 2 {  // Thermal pressure
            price = Int64(Double(price) * 1.5)
        }

        return price
    }

    private func calculatePriority(jobBid: Int64, minimumBid: Int64) -> Int {
        // Priority 0-100 based on how much above minimum
        let ratio = Double(jobBid) / Double(minimumBid)
        return min(100, Int(ratio * 50))  // Cap at 100
    }

    struct ResourceState {
        var thermalState: Int          // 0-4 (nominal to critical)
        var cpuUsage: Double           // 0.0-1.0
        var memoryPressure: Double     // 0.0-1.0
    }
}
```

## Dependencies

- **Issue #007**: macOS Foundation Models Worker (job queue, resource state)
- **Issue #003**: BOLT11 Lightning Primitives (invoice generation for payment-required)

## Testing

- [ ] Accept jobs with bid >= minimum
- [ ] Reject jobs with bid < minimum
- [ ] Surge pricing activates when busy
- [ ] Priority queue ordering
- [ ] Resource-based pricing adjustments

## Success Metrics

- [ ] Reject unprofitable jobs
- [ ] Accept profitable jobs
- [ ] Queue prioritizes high-value work
- [ ] Dynamic pricing responds to load

## Future Enhancements

- ML-based demand forecasting
- Auction-style bidding (highest bidder wins)
- Reputation-based pricing (discounts for repeat customers)
