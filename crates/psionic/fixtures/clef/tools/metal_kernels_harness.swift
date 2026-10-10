// Standalone checks of crates/psionic-backend-metal/src/kernels/clef_prefill.metal
// on synthetic data: the tensor-op GEMM (error vs f64, chunk invariance), the
// delta scan (vs a CPU reference) and attention (vs f64, chunk invariance),
// with timings. Build and run on a Mac:
//   swiftc -O metal_kernels_harness.swift -o harness && ./harness <path to clef_prefill.metal>
// SCAN=512x2|256x2|128x2|1024x1|rows and FLASH=mpp|sg2|simt pick variants.

import Metal
import Foundation

let src = try! String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let device = MTLCreateSystemDefaultDevice()!
let lib = try! device.makeLibrary(source: src, options: MTLCompileOptions())
let queue = device.makeCommandQueue()!
var psos: [String: MTLComputePipelineState] = [:]
func pso(_ n: String) -> MTLComputePipelineState {
    if let p = psos[n] { return p }
    let p = try! device.makeComputePipelineState(function: lib.makeFunction(name: n)!)
    psos[n] = p; return p
}
func buf(_ bytes: Int) -> MTLBuffer { device.makeBuffer(length: max(bytes, 16), options: .storageModeShared)! }
func f32(_ b: MTLBuffer, _ n: Int) -> UnsafeMutablePointer<Float> { b.contents().bindMemory(to: Float.self, capacity: n) }
func f16(_ b: MTLBuffer, _ n: Int) -> UnsafeMutablePointer<Float16> { b.contents().bindMemory(to: Float16.self, capacity: n) }
@discardableResult
func run(_ body: (MTLComputeCommandEncoder) -> Void) -> Double {
    let cb = queue.makeCommandBuffer()!
    let e = cb.makeComputeCommandEncoder()!
    body(e); e.endEncoding(); cb.commit(); cb.waitUntilCompleted()
    if let err = cb.error { print("GPU error \(err)") }
    return cb.gpuEndTime - cb.gpuStartTime
}
var rng = SystemRandomNumberGenerator()
// warm the GPU clock: Apple GPUs ramp frequency under sustained load
do {
    let n = 2048, m = 8192, k = 4096
    let x = device.makeBuffer(length: n * k * 2, options: .storageModeShared)!, w = device.makeBuffer(length: m * k * 2, options: .storageModeShared)!, o = device.makeBuffer(length: n * m * 4, options: .storageModeShared)!
    let p = try! device.makeComputePipelineState(function: lib.makeFunction(name: "clef_gemm")!)
    var nmk = SIMD3<UInt32>(UInt32(n), UInt32(m), UInt32(k))
    let start = Date()
    while Date().timeIntervalSince(start) < 2.0 {
        let cb = queue.makeCommandBuffer()!; let e = cb.makeComputeCommandEncoder()!
        e.setComputePipelineState(p); e.setBuffer(x, offset: 0, index: 0); e.setBuffer(w, offset: 0, index: 1); e.setBuffer(o, offset: 0, index: 2); e.setBytes(&nmk, length: 16, index: 3)
        e.dispatchThreadgroups(MTLSize(width: m / 64, height: n / 64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
        e.endEncoding(); cb.commit(); cb.waitUntilCompleted()
    }
}
func rnd() -> Float { Float.random(in: -1...1, using: &rng) }

// ---------- GEMM ----------
func gemm(_ x: MTLBuffer, _ xoff: Int, _ w: MTLBuffer, _ out: MTLBuffer, _ ooff: Int, n: Int, m: Int, k: Int, acc: Bool, _ e: MTLComputeCommandEncoder) {
    let p = pso(acc ? "clef_gemm_accumulate" : "clef_gemm")
    e.setComputePipelineState(p)
    e.setBuffer(x, offset: xoff, index: 0); e.setBuffer(w, offset: 0, index: 1); e.setBuffer(out, offset: ooff, index: 2)
    var nmk = SIMD3<UInt32>(UInt32(n), UInt32(m), UInt32(k)); e.setBytes(&nmk, length: 16, index: 3)
    e.dispatchThreadgroups(MTLSize(width: (m + 63) / 64, height: (n + 63) / 64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
}
for (m, k) in [(12288, 4096), (4096, 12288), (1024, 4096), (32, 4096)] {
    let n = 1082
    let x = buf(n * k * 2), w = buf(m * k * 2), o = buf(n * m * 4), o2 = buf(n * m * 4)
    let xp = f16(x, n * k), wp = f16(w, m * k)
    for i in 0..<(n * k) { xp[i] = Float16(rnd()) }
    for i in 0..<(m * k) { wp[i] = Float16(rnd() * 0.05) }
    var best = 1e9
    for _ in 0..<5 { best = min(best, run { gemm(x, 0, w, o, 0, n: n, m: m, k: k, acc: false, $0) }) }
    // chunked: rows in chunks of 64 (and an accumulate check)
    run { e in var f = 0; while f < n { let c = min(64, n - f); gemm(x, f * k * 2, w, o2, f * m * 4, n: c, m: m, k: k, acc: false, e); f += c } }
    let op = f32(o, n * m), op2 = f32(o2, n * m)
    var same = true
    for i in 0..<(n * m) where op[i] != op2[i] { same = false; break }
    var maxErr: Float = 0
    for (r, c) in [(0, 0), (n - 1, m - 1), (n / 2, m / 3), (17, 5)] {
        var ref: Double = 0
        for kk in 0..<k { ref += Double(Float(xp[r * k + kk])) * Double(Float(wp[c * k + kk])) }
        maxErr = max(maxErr, abs(Float(ref) - op[r * m + c]))
    }
    print(String(format: "gemm n=%d m=%d k=%d: %.3f ms %.1f TF, err %.2e, chunk64 %@", n, m, k, best * 1e3, 2.0 * Double(n * m * k) / best / 1e12, maxErr, same ? "bitwise" : "DIFFERS"))
}

// ---------- delta scan ----------
do {
    let n = 1082, kh = 16, vh = 32, dim = 128, cw = 2 * kh * dim + vh * dim, voff = 2 * kh * dim
    let qn = buf(n * kh * dim * 4), kn = buf(n * kh * dim * 4), conv = buf(n * cw * 4), decay = buf(n * vh * 4), beta = buf(n * vh * 4), kq = buf(n * kh * 4)
    let state = buf(vh * dim * dim * 4), out = buf(n * vh * dim * 4)
    let qp = f32(qn, n * kh * dim), kp = f32(kn, n * kh * dim), cp = f32(conv, n * cw), dp = f32(decay, n * vh), bp = f32(beta, n * vh), kqp = f32(kq, n * kh)
    for t in 0..<n { for h in 0..<kh {
        var nk: Float = 0, nq: Float = 0
        for i in 0..<dim { kp[(t * kh + h) * dim + i] = rnd(); qp[(t * kh + h) * dim + i] = rnd(); nk += kp[(t*kh+h)*dim+i] * kp[(t*kh+h)*dim+i]; nq += qp[(t*kh+h)*dim+i]*qp[(t*kh+h)*dim+i] }
        var dot: Float = 0
        for i in 0..<dim { kp[(t*kh+h)*dim+i] /= nk.squareRoot(); qp[(t*kh+h)*dim+i] /= nq.squareRoot() * Float(dim).squareRoot(); dot += kp[(t*kh+h)*dim+i] * qp[(t*kh+h)*dim+i] }
        kqp[t * kh + h] = dot } }
    for i in 0..<(n * cw) { cp[i] = rnd() }
    for i in 0..<(n * vh) { dp[i] = 0.9 + 0.1 * abs(rnd()); bp[i] = abs(rnd()) }
    struct Args { var n: UInt32; var kh: UInt32; var vh: UInt32; var reorder: UInt32; var cw: UInt32; var voff: UInt32 }
    var args = Args(n: UInt32(n), kh: UInt32(kh), vh: UInt32(vh), reorder: 0, cw: UInt32(cw), voff: UInt32(voff))
    let variant = ProcessInfo.processInfo.environment["SCAN"] ?? "512x2"
    let (name, rowsPerGroup, threads) = [
        "512x2": ("clef_delta_scan128", 128, 512), "256x2": ("clef_delta_scan128_256x2", 64, 256),
        "128x2": ("clef_delta_scan128_128x2", 32, 128), "1024x1": ("clef_delta_scan128_1024x1", 128, 1024), "rows": ("clef_delta_scan128_rows", 8, 256)][variant]!
    let groups = vh * 128 / rowsPerGroup
    memset(state.contents(), 0, vh * dim * dim * 4)
    run { e in
        e.setComputePipelineState(pso(name))
        for (i, b) in [qn, kn, conv, decay, beta, kq, state, out].enumerated() { e.setBuffer(b, offset: 0, index: i) }
        e.setBytes(&args, length: MemoryLayout<Args>.size, index: 8)
        e.dispatchThreadgroups(MTLSize(width: groups, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: threads, height: 1, depth: 1))
    }
    var t = 1e9
    let saved = Data(bytes: out.contents(), count: n * vh * dim * 4)
    for _ in 0..<3 {
        t = min(t, run { e in
            e.setComputePipelineState(pso(name))
            for (i, b) in [qn, kn, conv, decay, beta, kq, state, out].enumerated() { e.setBuffer(b, offset: 0, index: i) }
            e.setBytes(&args, length: MemoryLayout<Args>.size, index: 8)
            e.dispatchThreadgroups(MTLSize(width: groups, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: threads, height: 1, depth: 1))
        })
    }
    saved.withUnsafeBytes { memcpy(out.contents(), $0.baseAddress!, saved.count) }
    // CPU reference for value head 3, rows 0..127
    let v = 3, khh = v / (vh / kh)
    var S = [Float](repeating: 0, count: dim * dim)
    var maxErr: Float = 0, scale: Float = 0
    let op = f32(out, n * vh * dim)
    for tt in 0..<n {
        let g = dp[tt * vh + v], b = bp[tt * vh + v]
        for r in 0..<dim {
            var sk: Float = 0, sq: Float = 0
            for c in 0..<dim { S[r * dim + c] *= g; sk += S[r*dim+c] * kp[(tt*kh+khh)*dim+c]; sq += S[r*dim+c] * qp[(tt*kh+khh)*dim+c] }
            let delta = (cp[tt * cw + voff + v * dim + r] - sk) * b
            for c in 0..<dim { S[r * dim + c] += kp[(tt*kh+khh)*dim+c] * delta }
            let o = sq + delta * kqp[tt * kh + khh]
            maxErr = max(maxErr, abs(o - op[(tt * vh + v) * dim + r])); scale = max(scale, abs(o))
        }
    }
    print(variant, String(format: "delta scan n=%d: %.3f ms per layer, max err %.2e of %.2e", n, t * 1e3, maxErr, scale))
}

// ---------- flash attention ----------
var scoreBuf: MTLBuffer? = nil
var probBuf: MTLBuffer? = nil
func flashMPP(_ q: MTLBuffer, _ qoff: Int, _ k: MTLBuffer, _ v: MTLBuffer, _ o: MTLBuffer, _ ooff: Int, n: Int, first: Int, _ e: MTLComputeCommandEncoder) {
    struct A { var n: UInt32; var keys: UInt32; var heads: UInt32; var kvh: UInt32; var head: UInt32 }
    let keys = first + n
    if scoreBuf == nil || scoreBuf!.length < n * keys * 4 { scoreBuf = buf(n * keys * 4); probBuf = buf(n * keys * 2) }
    for h in 0..<16 {
        var a = A(n: UInt32(n), keys: UInt32(keys), heads: 16, kvh: 4, head: UInt32(h))
        e.setComputePipelineState(pso("clef_attn_scores"))
        e.setBuffer(q, offset: qoff, index: 0); e.setBuffer(k, offset: 0, index: 1); e.setBuffer(scoreBuf, offset: 0, index: 2); e.setBytes(&a, length: 20, index: 3)
        e.dispatchThreadgroups(MTLSize(width: (keys + 63) / 64, height: (n + 63) / 64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
        var nkf = SIMD3<UInt32>(UInt32(n), UInt32(keys), UInt32(first))
        e.setComputePipelineState(pso("clef_causal_softmax_to_f16"))
        e.setBuffer(scoreBuf, offset: 0, index: 0); e.setBuffer(probBuf, offset: 0, index: 1); e.setBytes(&nkf, length: 16, index: 2)
        e.dispatchThreadgroups(MTLSize(width: n, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 256, height: 1, depth: 1))
        e.setComputePipelineState(pso("clef_attn_pv"))
        e.setBuffer(probBuf, offset: 0, index: 0); e.setBuffer(v, offset: 0, index: 1); e.setBuffer(o, offset: ooff, index: 2); e.setBytes(&a, length: 20, index: 3)
        e.dispatchThreadgroups(MTLSize(width: 256 / 64, height: (n + 63) / 64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
    }
}
func flash(_ q: MTLBuffer, _ qoff: Int, _ k: MTLBuffer, _ v: MTLBuffer, _ o: MTLBuffer, _ ooff: Int, n: Int, first: Int, _ e: MTLComputeCommandEncoder) {
    if ProcessInfo.processInfo.environment["FLASH"] == "mpp" { flashMPP(q, qoff, k, v, o, ooff, n: n, first: first, e); return }
    struct A { var n: UInt32; var heads: UInt32; var kvh: UInt32; var first: UInt32 }
    var a = A(n: UInt32(n), heads: 16, kvh: 4, first: UInt32(first))
    let fv = ProcessInfo.processInfo.environment["FLASH"] ?? "sg4"
    let sg = fv != "simt"
    let qb = fv == "sg2" ? 2 : 1
    e.setComputePipelineState(pso(fv == "simt" ? "clef_flash_attention256" : (qb == 4 ? "clef_flash_attention256_sg" : "clef_flash_attention256_sg2")))
    e.setBuffer(q, offset: qoff, index: 0); e.setBuffer(k, offset: 0, index: 1); e.setBuffer(v, offset: 0, index: 2); e.setBuffer(o, offset: ooff, index: 3)
    e.setBytes(&a, length: 16, index: 4)
    if sg {
        e.dispatchThreadgroups(MTLSize(width: (n + 8 * qb - 1) / (8 * qb), height: 4, depth: 1), threadsPerThreadgroup: MTLSize(width: 128 * qb, height: 1, depth: 1))
    } else {
        e.dispatchThreadgroups(MTLSize(width: (n + 7) / 8, height: 16, depth: 1), threadsPerThreadgroup: MTLSize(width: 256, height: 1, depth: 1))
    }
}
for L in [1082, 4096, 16384] {
    let heads = 16, kvh = 4, dim = 256
    let q = buf((L + 32) * heads * dim * 2), k = buf(L * kvh * dim * 2), v = buf(L * kvh * dim * 2), o = buf((L + 32) * heads * dim * 4), o2 = buf((L + 32) * heads * dim * 4)
    let qp = f16(q, L * heads * dim), kp = f16(k, L * kvh * dim), vp = f16(v, L * kvh * dim)
    for i in 0..<(L * heads * dim) { qp[i] = Float16(rnd() * 0.125) }
    for i in 0..<(L * kvh * dim) { kp[i] = Float16(rnd() * 2); vp[i] = Float16(rnd()) }
    var best = 1e9
    for _ in 0..<3 { best = min(best, run { flash(q, 0, k, v, o, 0, n: L, first: 0, $0) }) }
    run { e in var f = 0; while f < L { let c = min(512, L - f); flash(q, f * heads * dim * 2, k, v, o2, f * heads * dim * 4, n: c, first: f, e); f += c } }
    let op = f32(o, L * heads * dim), op2 = f32(o2, L * heads * dim)
    var same = true
    for i in 0..<(L * heads * dim) where op[i] != op2[i] { same = false; break }
    var maxErr = 0.0
    for t in [0, 31, 32, L / 2, L - 1] { for h in [0, 7, 15] {
        let kh = h / 4
        var s = [Double](repeating: 0, count: t + 1); var mx = -1e300
        for j in 0...t { var d = 0.0; for i in 0..<dim { d += Double(qp[(t*heads+h)*dim+i]) * Double(kp[(j*kvh+kh)*dim+i]) }; s[j] = d; mx = max(mx, d) }
        var sum = 0.0; for j in 0...t { s[j] = exp(s[j] - mx); sum += s[j] }
        for i in 0..<dim { var acc = 0.0; for j in 0...t { acc += s[j] / sum * Double(vp[(j*kvh+kh)*dim+i]) }; maxErr = max(maxErr, abs(acc - Double(op[(t*heads+h)*dim+i]))) }
    } }
    let flops = 4.0 * Double(heads * dim) * Double(L) * Double(L + 1) / 2
    print(String(format: "flash L=%d: %.3f ms per layer %.1f TF, err vs f64 %.2e, chunk512 %@", L, best * 1e3, flops / best / 1e12, maxErr, same ? "bitwise" : "DIFFERS"))
}
