# Post-Mortem: WebSocket Updates Regression

**Date**: 2025-11-06
**Severity**: Critical
**Status**: Fixed

## Executive Summary

A critical regression prevented WebSocket updates from appearing in the UI after the ring buffer filled to capacity. The bug existed in the ring buffer logic and the change observation mechanism, but was not caught by existing tests because they didn't test the full integration path.

---

## 1. What Broke: Root Cause Analysis

### The Bug (3 Parts)

#### Part 1: Ring Buffer Logic Flaw
**Location**: `BridgeManager.swift:147-151`

**Original Code**:
```swift
DispatchQueue.main.async { [weak self] in
    guard let self = self else { return }
    self.updates.append(note)
    if self.updates.count > 200 {
        self.updates.removeFirst(self.updates.count - 200)
    }
}
```

**Problems**:
1. **Append-then-trim**: Updates were appended first, then trimmed
2. **Off-by-one condition**: Used `> 200` instead of `>= 200`
3. **Bulk removal**: Removed `count - 200` items at once instead of 1

**Behavior**:
- Updates 1-200: Buffer grows normally
- Update 201: Appends, then trims back to 200
- Updates 202-297: Buffer keeps growing (condition `> 200` is false at 200, true at 201+)
- Eventually: Bulk trim from 297 → 200
- After that: Buffer oscillates 200 → 201 → 200

**Evidence from logs**:
```
[AcpThreadView] onChange fired: count 297 -> 200, timeline.count=290
```

#### Part 2: No Explicit Change Notification
The ring buffer logic didn't explicitly call `objectWillChange.send()`, relying instead on the implicit notification from the `@Published` wrapper. However, when the count stayed constant at 200 (after the bulk trim), no notification was sent for subsequent updates.

#### Part 3: Observer Mismatch
**Location**: `AcpThreadView.swift:122`

**Original Code**:
```swift
.onChange(of: bridge.updates.count) { oldCount, newCount in
    // Only fires when count changes
}
```

**Problem**: Once the ring buffer stabilized at count=200, new updates would append and immediately remove the oldest, keeping count constant. `.onChange` would never fire again.

**User Impact**: New messages from WebSocket appeared in logs but never rendered in the UI.

---

## 2. Why Existing Tests Didn't Catch It

### Test Gap Analysis

#### What Was Tested ✅

**BridgeManagerTests.swift:135-152** - `testUpdateRingBufferLimit()`
```swift
func testUpdateRingBufferLimit() {
    let updates = (1...250).map { i in
        TestHelpers.makeSessionUpdateNotification(...)
    }

    for update in updates {
        sut.updates.append(update)
        if sut.updates.count > 200 {
            sut.updates.removeFirst(sut.updates.count - 200)
        }
    }

    XCTAssertEqual(sut.updates.count, 200)
}
```

**Why it passed**:
- ✅ Manually applies ring buffer logic in test code
- ✅ Verifies final count is 200
- ❌ **Doesn't test the actual delegate callback path**
- ❌ **Doesn't test objectWillChange notifications**
- ❌ **Doesn't test integration with AcpThreadView**

#### What Was NOT Tested ❌

1. **Real Delegate Path**
   - No test for `mobileWebSocketClient(_:didReceiveJSONRPCNotification:payload:)`
   - The actual code path that receives updates from WebSocket
   - The DispatchQueue.main.async wrapper behavior

2. **ObservableObject Notifications**
   - No test verifying `objectWillChange` fires when updates are added
   - No test verifying `@Published` updates trigger at capacity
   - No test for the implicit vs explicit notification behavior

3. **Integration Between Components**
   - No test for BridgeManager → AcpThreadView update flow
   - No test that UI actually receives and processes new updates
   - No test that `.onChange` handlers fire correctly

4. **Ring Buffer at Capacity**
   - Tests added 250 updates but didn't verify behavior AFTER capacity
   - No test adding updates 251-300 to verify steady-state behavior
   - No test that notifications still fire when count stays at 200

### Why Manual Logic Passed

The test manually replicated the (buggy) ring buffer logic but verified it in isolation. The test proved "if I implement this logic manually, count ends at 200" but didn't prove "the production code behaves correctly at capacity."

---

## 3. What Changed to Fix It

### Fix #1: Ring Buffer Logic (BridgeManager.swift)

**Before**:
```swift
self.updates.append(note)
if self.updates.count > 200 {
    self.updates.removeFirst(self.updates.count - 200)
}
```

**After**:
```swift
// Ring buffer: keep last 200 updates
if self.updates.count >= 200 { self.updates.removeFirst() }
self.updates.append(note)
// Force objectWillChange to notify observers even when count stays at 200
self.objectWillChange.send()
```

**Changes**:
1. ✅ Trim-before-append: Removes oldest BEFORE adding new
2. ✅ Correct condition: `>= 200` ensures max capacity of 200
3. ✅ Single removal: Only removes 1 item at a time
4. ✅ Explicit notification: `objectWillChange.send()` guarantees observers fire

### Fix #2: Observer Mechanism (AcpThreadView.swift)

**Before**:
```swift
.onChange(of: bridge.updates.count) { oldCount, newCount in
    // Only fires when count changes
}
```

**After**:
```swift
import Combine  // Added

.onReceive(bridge.objectWillChange) { _ in
    let newUpdates = bridge.updates
    // Fires every time objectWillChange is sent
}
```

**Changes**:
1. ✅ Observes `objectWillChange` publisher instead of count
2. ✅ Fires on every explicit `objectWillChange.send()`
3. ✅ No Equatable requirement (original attempt used `.onChange(of: bridge.updates)` which required Equatable)
4. ✅ Works correctly at constant count

**Why the intermediate attempt failed**:
- Tried `.onChange(of: bridge.updates)`
- Compiler error: requires `SessionNotificationWire` to conform to `Equatable`
- Can't add Equatable to OpenAgentsCore type (not our code)
- Solution: Use `onReceive` instead

---

## 4. How to Add Tests to Prevent Future Regressions

### Recommended Test Suite Additions

#### Test 1: Ring Buffer Behavior at Capacity

```swift
func testRingBuffer_MaintainsCapacityAt200() {
    // Simulate delegate receiving updates
    for i in 1...250 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "message \(i)")
        )

        // Simulate actual delegate path
        DispatchQueue.main.async {
            // Ring buffer logic (should match BridgeManager)
            if self.sut.updates.count >= 200 {
                self.sut.updates.removeFirst()
            }
            self.sut.updates.append(update)
            self.sut.objectWillChange.send()
        }
    }

    // Wait for async completion
    let expectation = expectation(description: "updates processed")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
        expectation.fulfill()
    }
    wait(for: [expectation], timeout: 1.0)

    // Verify capacity maintained
    XCTAssertEqual(sut.updates.count, 200)

    // Verify newest updates retained (not oldest)
    if let firstUpdate = sut.updates.first {
        // Should contain "message 51" (201-250 kept, 1-50 dropped)
        // Adjust based on actual behavior
    }
}
```

#### Test 2: objectWillChange Fires at Capacity

```swift
func testObjectWillChange_FiresWhenRingBufferFull() {
    let expectation = expectation(description: "objectWillChange fires")
    expectation.expectedFulfillmentCount = 10  // Expect 10 fires

    var fireCount = 0
    let cancellable = sut.objectWillChange.sink { _ in
        fireCount += 1
        if fireCount <= 10 {
            expectation.fulfill()
        }
    }

    // Fill to capacity
    for i in 1...200 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "fill \(i)")
        )
        sut.updates.append(update)
        sut.objectWillChange.send()
    }

    // Add 10 more at capacity
    for i in 1...10 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "at capacity \(i)")
        )

        if sut.updates.count >= 200 { sut.updates.removeFirst() }
        sut.updates.append(update)
        sut.objectWillChange.send()
    }

    wait(for: [expectation], timeout: 2.0)
    XCTAssertGreaterThanOrEqual(fireCount, 10)

    cancellable.cancel()
}
```

#### Test 3: Integration Test - BridgeManager to UI

```swift
func testBridgeManagerUpdates_TriggerUIObserver() {
    let bridge = BridgeManager()
    let expectation = expectation(description: "UI observer fires")
    expectation.expectedFulfillmentCount = 5

    var receivedUpdates: [ACP.Client.SessionNotificationWire] = []

    // Simulate AcpThreadView observer
    let cancellable = bridge.objectWillChange.sink { _ in
        let snapshot = bridge.updates
        if !snapshot.isEmpty {
            if let lastUpdate = snapshot.last {
                receivedUpdates.append(lastUpdate)
                expectation.fulfill()
            }
        }
    }

    // Simulate WebSocket receiving updates
    for i in 1...5 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "integration test \(i)")
        )

        DispatchQueue.main.async {
            if bridge.updates.count >= 200 { bridge.updates.removeFirst() }
            bridge.updates.append(update)
            bridge.objectWillChange.send()
        }
    }

    wait(for: [expectation], timeout: 2.0)
    XCTAssertEqual(receivedUpdates.count, 5)

    cancellable.cancel()
}
```

#### Test 4: Ring Buffer at Capacity Maintains Order

```swift
func testRingBuffer_MaintainsOrderAtCapacity() {
    // Fill to capacity with identifiable updates
    for i in 1...200 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "initial-\(i)")
        )
        sut.updates.append(update)
    }

    XCTAssertEqual(sut.updates.count, 200)

    // Add 50 more updates
    for i in 1...50 {
        let update = TestHelpers.makeSessionUpdateNotification(
            update: TestHelpers.makeTextUpdate(text: "new-\(i)")
        )

        if sut.updates.count >= 200 { sut.updates.removeFirst() }
        sut.updates.append(update)
    }

    // Should still have 200 updates
    XCTAssertEqual(sut.updates.count, 200)

    // First update should be "initial-51" (dropped 1-50)
    // Last update should be "new-50"
    // Verify FIFO order maintained
}
```

#### Test 5: Delegate Callback Path

```swift
func testWebSocketDelegate_AppendsUpdatesCorrectly() {
    let bridge = BridgeManager()
    let client = MockMobileWebSocketClient()

    // Simulate connection
    bridge.mobileWebSocketClientDidConnect(client)

    // Simulate receiving notification
    let update = TestHelpers.makeSessionUpdateNotification(
        update: TestHelpers.makeTextUpdate(text: "delegate test")
    )
    let payload = try! JSONEncoder().encode(update)

    bridge.mobileWebSocketClient(
        client,
        didReceiveJSONRPCNotification: ACPRPC.sessionUpdate,
        payload: payload
    )

    // Verify update was added
    XCTAssertEqual(bridge.updates.count, 1)
}
```

---

## 5. Test Coverage Gaps Summary

### Current Coverage

| Component | Test Coverage | Integration Tests |
|-----------|--------------|-------------------|
| BridgeManager initialization | ✅ Good | N/A |
| Logging ring buffer | ✅ Good | N/A |
| Connection state machine | ✅ Good | N/A |
| Updates ring buffer logic | ⚠️ **Partial** | ❌ **Missing** |
| WebSocket delegate callbacks | ❌ **None** | ❌ **Missing** |
| objectWillChange notifications | ❌ **None** | ❌ **Missing** |
| AcpThreadView update handling | ❌ **None** | ❌ **Missing** |
| BridgeManager ↔ AcpThreadView | ❌ **None** | ❌ **Missing** |

### Recommended Additions

1. **Unit Tests** (BridgeManagerTests):
   - ✅ Ring buffer at capacity (steady state)
   - ✅ objectWillChange fires at capacity
   - ✅ Delegate callback path
   - ✅ Order preservation in ring buffer

2. **Integration Tests** (New file: BridgeManagerIntegrationTests):
   - ✅ BridgeManager → AcpThreadView update flow
   - ✅ WebSocket → Delegate → Storage → UI pipeline
   - ✅ Performance test: 1000 updates at capacity

3. **Regression Tests** (Specific for this bug):
   - ✅ Updates appear after count reaches 200
   - ✅ Updates appear between counts 200-300
   - ✅ onChange/onReceive fires correctly at capacity

---

## 6. Lessons Learned

### Testing Principles Violated

1. **Test the Real Code Path**: Tests must exercise the actual production code path, not replicate logic in test code
2. **Test Integration Points**: Component boundaries (BridgeManager ↔ AcpThreadView) need explicit integration tests
3. **Test Steady-State Behavior**: Don't just test 0→capacity, test behavior AT capacity over time
4. **Test Observer Patterns**: Combine publishers and SwiftUI observers need explicit verification
5. **Test Async Boundaries**: DispatchQueue.main.async wrappers affect timing and need async tests

### What We'll Do Better

1. ✅ Add integration tests for cross-component data flow
2. ✅ Test delegate methods directly, not just final state
3. ✅ Verify observable properties fire correctly
4. ✅ Test beyond initial fill to capacity (test 200→300 updates)
5. ✅ Add explicit objectWillChange verification

---

## 7. Performance Implications

### Before Fix
- Memory: Ring buffer could grow to ~300 items before bulk trim
- Notifications: Sparse after bulk trim (only on count change)
- UI Updates: Stopped after ring buffer filled

### After Fix
- Memory: Ring buffer stays at exactly 200 items
- Notifications: Every update triggers objectWillChange.send()
- UI Updates: Continuous, even at capacity

### Potential Concern
`objectWillChange.send()` fires on every update. With high message volume, this could cause UI churn.

**Mitigation**: Consider debouncing or throttling if we see performance issues with rapid updates.

---

## 8. Related Issues

None identified yet, but watch for:
- Performance degradation with rapid WebSocket updates
- Memory leaks if observers aren't properly cancelled
- Similar patterns in other ring buffers (check logs ring buffer)

---

## 9. Verification Checklist

- [x] Ring buffer maintains capacity of 200
- [x] Updates appear in UI after buffer fills
- [x] objectWillChange fires at capacity
- [x] onReceive observer fires correctly
- [x] No Equatable conformance errors
- [x] Build succeeds on macOS
- [x] Existing tests still pass
- [ ] **TODO**: Manual testing on iOS device
- [ ] **TODO**: Add recommended test suite
- [ ] **TODO**: Monitor for performance issues

---

## 10. Timeline

- **2025-11-05 23:37**: User reports regression, provides logs
- **2025-11-05 23:45**: Identified count observation issue via debug logging
- **2025-11-05 23:50**: Fixed ring buffer logic in BridgeManager
- **2025-11-05 23:55**: Fixed observer from count to array (failed - Equatable)
- **2025-11-06 00:00**: Fixed observer to use onReceive publisher
- **2025-11-06 00:05**: All fixes committed and pushed
- **2025-11-06 00:10**: Post-mortem analysis completed

**Total Resolution Time**: ~30 minutes from report to fix
**Root Cause Time**: ~15 minutes
**Fix Time**: ~15 minutes (including failed attempt)

---

## Commits

- `91d6ce8d`: Fix critical regression: WebSocket updates not appearing after ring buffer fills
- `3bab7a96`: Fix Equatable conformance issue by using onReceive instead of onChange

---

*End of Post-Mortem*
