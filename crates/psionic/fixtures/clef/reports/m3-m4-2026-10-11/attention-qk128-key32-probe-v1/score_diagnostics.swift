// Diagnostic kernels copy each score tile after the same MPP operation. They
// never participate in timing, and must leave the normal output bitwise intact.
func scoreDiagnostic(_ runner: Runner, _ index: Int, _ s: Shape, pattern: String) throws {
    try autoreleasepool {
        let input = try Inputs(runner.device, s, pattern: pattern)
        let base = try output(runner.device, s), candidate = try output(runner.device, s)
        let normalBase = try output(runner.device, s), normalCandidate = try output(runner.device, s)
        let a = try Buffer(runner.device, s.n * s.heads * s.keys * 4)
        let b = try Buffer(runner.device, s.n * s.heads * s.keys * 4)
        _ = try runner.run(32, input, base, scores: a)
        _ = try runner.run(128, input, candidate, scores: b)
        _ = try runner.run(32, input, normalBase)
        _ = try runner.run(128, input, normalCandidate)
        let ap = a.pointer.assumingMemoryBound(to: Float.self), bp = b.pointer.assumingMemoryBound(to: Float.self)
        let qp = input.q.pointer.assumingMemoryBound(to: Float16.self), kp = input.k.pointer.assumingMemoryBound(to: Float16.self)
        var bits = 0, cells = 0, holes = 0, maximum = 0.0, finite = true
        var first = [[String: Any]](), samples = [[String: Any]](), oracleMaximum = 0.0
        for row in 0..<s.n { for head in 0..<s.heads {
            let limit = s.first + min(s.n, (row / 16 + 1) * 16)
            for key in 0..<s.keys {
                let i = (row * s.heads + head) * s.keys + key
                if key >= limit {
                    try require(ap[i].bitPattern == 0x7fc00555 && bp[i].bitPattern == 0x7fc00555, "score copy wrote an uncomputed key")
                    holes += 1; continue
                }
                cells += 1
                if !(ap[i].isFinite && bp[i].isFinite) { finite = false }
                let error = abs(Double(ap[i]) - Double(bp[i]))
                if error.isFinite { maximum = max(maximum, error) }
                if ap[i].bitPattern != bp[i].bitPattern {
                    bits += 1
                    if first.count < 8 { first.append(["row": row, "head": head, "key": key, "baseline_bits": ap[i].bitPattern, "candidate_bits": bp[i].bitPattern]) }
                }
            }
            if [0, s.n - 1].contains(row) && [0, s.heads - 1].contains(head) {
                for key in Set([0, min(31, limit - 1), min(32, limit - 1), min(127, limit - 1), min(128, limit - 1), limit - 1]).sorted() {
                    let kh = head / (s.heads / s.kv), i = (row * s.heads + head) * s.keys + key
                    var expected = 0.0
                    for d in 0..<dim { expected += Double(qp[(row * s.heads + head) * dim + d]) * Double(kp[(key * s.kv + kh) * dim + d]) }
                    let ea = abs(Double(ap[i]) - expected), eb = abs(Double(bp[i]) - expected)
                    let ok = expected.isFinite && ap[i].isFinite && bp[i].isFinite && ea <= 0.001 && eb <= 0.001
                    if ea.isFinite && eb.isFinite { oracleMaximum = max(oracleMaximum, max(ea, eb)) }
                    samples.append(["row": row, "head": head, "key": key, "baseline_bits": ap[i].bitPattern, "candidate_bits": bp[i].bitPattern, "expected_f64": expected, "baseline_error_text": String(ea), "candidate_error_text": String(eb), "passed": ok])
                }
            }
        }}
        try a.guards(); try b.guards(); try input.unchanged()
        let oraclePassed = samples.allSatisfy { $0["passed"] as? Bool == true }
        checks += 1; comparisons += cells
        try emit(["kind": "score_diagnostic", "index": index, "n": s.n, "heads": s.heads, "first": s.first, "pattern": pattern, "values": cells, "uncomputed_poison_cells": holes, "different_bits": bits, "first_differences": first, "max_abs": maximum, "oracle_max_abs": oracleMaximum, "samples": samples, "finite": finite, "failures": bits, "passed": finite && bits == 0 && oraclePassed])
        try require(finite && bits == 0 && oraclePassed, "QK score diagnostic is not bitwise equal to baseline")
        try compare(base, normalBase.pointer.assumingMemoryBound(to: Float.self), count: s.count, label: "score-\(index)-baseline-instrumentation", bitwise: true)
        try compare(candidate, normalCandidate.pointer.assumingMemoryBound(to: Float.self), count: s.count, label: "score-\(index)-candidate-instrumentation", bitwise: true)
        try compare(candidate, base.pointer.assumingMemoryBound(to: Float.self), count: s.count, label: "score-\(index)-candidate-baseline", bitwise: true)
    }
}
