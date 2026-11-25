// Firm reason: 6 (AppKit/Metal dlopen host bridging local GTA VI Metal 4 ray-traced workbench into the Metaverse pane).
// Loads local scratch/gta6-mandy/metal/ (Mandy + Jason + ky34_hse Hardware RT + MetalFX 2x) without bundling external assets.
import AppKit
import Darwin
import Foundation
import Metal

#if OA_FULL

struct GtaWorkbenchState {
    var paused: Bool = false
    var denoiserOn: Bool = true
    var rockstarCam: Bool = false
    var clipIndex: Int32 = 1
    var exprIndex: Int32 = 0
    var accumSpp: UInt32 = 1
    var width: UInt32 = 0
    var height: UInt32 = 0
    var gpuMs: Double = 0
    var presetName: String = "kitchen_live"
    var offsetX: Float = 0
    var offsetY: Float = 0
    var offsetZ: Float = 0
    var offsetYaw: Float = 0
}

final class GtaWorkbenchSession {
    typealias ReadyC = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?, Double, UInt32) -> Void
    private typealias FnCreate = @convention(c) (UnsafePointer<CChar>, UnsafePointer<CChar>, UInt32, UInt32) -> UnsafeMutableRawPointer?
    private typealias FnDestroy = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias FnResize = @convention(c) (UnsafeMutableRawPointer?, UInt32, UInt32) -> Void
    private typealias FnStepAsync = @convention(c) (UnsafeMutableRawPointer?, Float, UInt32, Int32, ReadyC?, UnsafeMutableRawPointer?) -> Int32
    private typealias FnOrbit = @convention(c) (UnsafeMutableRawPointer?, Float, Float) -> Void
    private typealias FnPan = @convention(c) (UnsafeMutableRawPointer?, Float, Float) -> Void
    private typealias FnZoom = @convention(c) (UnsafeMutableRawPointer?, Float) -> Void
    private typealias FnMoveWasd = @convention(c) (UnsafeMutableRawPointer?, Float, Float, Float, Float) -> Void
    private typealias FnSetPreset = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>) -> Void
    private typealias FnSetInt = @convention(c) (UnsafeMutableRawPointer?, Int32) -> Void
    private typealias FnVoid = @convention(c) (UnsafeMutableRawPointer?) -> Void
    private typealias FnGetState = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<Int32>, UnsafeMutablePointer<Double>) -> Void
    private typealias FnCapturePng = @convention(c) (UnsafeMutableRawPointer?, UInt32, UnsafePointer<CChar>) -> Int32
    private typealias FnHitTest = @convention(c) (UnsafeMutableRawPointer?, Float, Float) -> Int32
    private typealias FnBeginDrag = @convention(c) (UnsafeMutableRawPointer?, Float, Float, Int32) -> Int32
    private typealias FnDragTo = @convention(c) (UnsafeMutableRawPointer?, Float, Float, Float, Float, Int32) -> Void
    private typealias FnRotate = @convention(c) (UnsafeMutableRawPointer?, Float) -> Void
    private typealias FnGetOffset = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutablePointer<Float>) -> Void

    private static var dlHandle: UnsafeMutableRawPointer?
    private static var fnCreate: FnCreate?
    private static var fnDestroy: FnDestroy?
    private static var fnResize: FnResize?
    private static var fnStepAsync: FnStepAsync?
    private static var fnOrbit: FnOrbit?
    private static var fnPan: FnPan?
    private static var fnZoom: FnZoom?
    private static var fnMoveWasd: FnMoveWasd?
    private static var fnSetPreset: FnSetPreset?
    private static var fnSetPaused: FnSetInt?
    private static var fnSetDenoiser: FnSetInt?
    private static var fnSetRockstarCam: FnSetInt?
    private static var fnCycleClip: FnVoid?
    private static var fnCycleExpr: FnVoid?
    private static var fnGetState: FnGetState?
    private static var fnCapturePng: FnCapturePng?
    private static var fnHitTest: FnHitTest?
    private static var fnBeginDrag: FnBeginDrag?
    private static var fnDragTo: FnDragTo?
    private static var fnEndDrag: FnVoid?
    private static var fnRotate: FnRotate?
    private static var fnResetPos: FnVoid?
    private static var fnGetOffset: FnGetOffset?

    private static let buildLock = NSLock()
    private static let abiVersionTag = "oa_mandy_abi_v5"

    let key: String
    let rootDir: String
    private var ctx: UnsafeMutableRawPointer?
    private(set) var loading = true
    private(set) var errorText = ""
    private(set) var state = GtaWorkbenchState()
    private(set) var latestTexture: MTLTexture?
    private(set) var smoothFps: Double = 60.0
    private var lastFrameWall: CFTimeInterval = 0
    private var pendingResizeSize: (UInt32, UInt32)?
    private var lastResizeAppliedAt: CFTimeInterval = 0
    private var destroyed = false

    var onReady: (() -> Void)?
    var onFrameTextureReady: ((MTLTexture) -> Void)?
    var onStateChanged: (() -> Void)?

    static let presets: [(id: String, label: String)] = [
        ("kitchen_live", "Kitchen Live (Full Body)"),
        ("portrait_live", "Portrait Live (Mandy)"),
        ("stg1", "1 · Kitchen Wide"),
        ("stg2", "2 · Counter Snapmatic"),
        ("stg3", "3 · Over-Counter Lean"),
        ("stg4", "4 · Hero Hair Bokeh"),
        ("stg5", "5 · Front Close-Up"),
        ("two_shot", "6 · Two-Shot (Mandy + Jason)"),
        ("jason_cu", "7 · Jason Hero Portrait"),
        ("showcase", "0 · Auto-Showcase (6 Clips)")
    ]

    static let exprNames = ["Auto", "Blink", "Smile", "Gaze", "Talk", "Intense"]
    static let clipNames = ["flirt_idle", "flirt_mandy", "flirt_enter", "finger_mandy", "counter_loop", "exit_mandy", "showcase"]

    static func locateMandyRoot(workspace: String = "") -> String? {
        let fm = FileManager.default
        var candidates: [String] = []
        if let env = ProcessInfo.processInfo.environment["OA_GTA6_MANDY_DIR"], !env.isEmpty {
            candidates.append(env)
        }
        if !workspace.isEmpty {
            candidates.append((workspace as NSString).appendingPathComponent("scratch/gta6-mandy"))
            // Also resolve parent repo root if workspace is inside a .claude/worktrees/* worktree
            if let range = workspace.range(of: "/.claude/worktrees/") {
                let mainRepo = String(workspace[..<range.lowerBound])
                candidates.append((mainRepo as NSString).appendingPathComponent("scratch/gta6-mandy"))
            }
        }
        let home = NSHomeDirectory()
        candidates.append("\(home)/work/openagents/scratch/gta6-mandy")
        candidates.append("\(home)/Desktop/GTA/gta6-mandy")
        for dir in candidates {
            let meta = (dir as NSString).appendingPathComponent("assets/mandy-meta.bin")
            let mm = (dir as NSString).appendingPathComponent("metal/mandy_metal.mm")
            if fm.fileExists(atPath: meta) && fm.fileExists(atPath: mm) {
                return dir
            }
        }
        return nil
    }

    init(key: String, rootDir: String, initialSize: CGSize) {
        self.key = key
        self.rootDir = rootDir
        let (rw, rh) = Self.clampedRenderDims(for: initialSize)
        self.state.width = rw
        self.state.height = rh
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            self?.loadAsync(width: rw, height: rh)
        }
    }

    deinit {
        destroyed = true
        if let c = ctx {
            Self.fnDestroy?(c)
            ctx = nil
        }
    }

    static func clampedRenderDims(for size: CGSize) -> (UInt32, UInt32) {
        // Preserve exact pane aspect ratio while capping total pixels to ~921K (1280x720 budget)
        // so SVGF 2x temporal denoising stays locked at 60+ FPS in both docked and expanded panes.
        let w = max(160.0, Double(size.width))
        let h = max(160.0, Double(size.height))
        let dpr = 1.5
        var rw = w * dpr
        var rh = h * dpr
        let maxPixels = 921_600.0
        if rw * rh > maxPixels {
            let s = sqrt(maxPixels / (rw * rh))
            rw *= s
            rh *= s
        }
        let evenW = UInt32(max(256, min(1440, Int(rw.rounded())) & ~1))
        let evenH = UInt32(max(256, min(1440, Int(rh.rounded())) & ~1))
        return (evenW, evenH)
    }

    private func loadAsync(width: UInt32, height: UInt32) {
        let res = Self.ensureLibraryLoaded(rootDir: rootDir)
        if let err = res {
            DispatchQueue.main.async { [weak self] in
                guard let self, !self.destroyed else { return }
                self.loading = false
                self.errorText = err
                self.onReady?()
            }
            return
        }
        let assetsDir = (rootDir as NSString).appendingPathComponent("assets")
        let metalDir = (rootDir as NSString).appendingPathComponent("metal")
        let libPath = Self.ensureMetalLib(metalDir: metalDir)
        guard !libPath.isEmpty else {
            DispatchQueue.main.async { [weak self] in
                guard let self, !self.destroyed else { return }
                self.loading = false
                self.errorText = "Missing mandy_rt.metallib in \(metalDir)"
                self.onReady?()
            }
            return
        }
        guard let fnCreate = Self.fnCreate else {
            DispatchQueue.main.async { [weak self] in
                guard let self, !self.destroyed else { return }
                self.loading = false
                self.errorText = "Failed to bind oa_mandy_create symbol"
                self.onReady?()
            }
            return
        }
        let created = assetsDir.withCString { aPtr in
            libPath.withCString { lPtr in
                fnCreate(aPtr, lPtr, width, height)
            }
        }
        DispatchQueue.main.async { [weak self] in
            guard let self else {
                if let c = created { Self.fnDestroy?(c) }
                return
            }
            if self.destroyed {
                if let c = created { Self.fnDestroy?(c) }
                return
            }
            if let c = created {
                self.ctx = c
                self.loading = false
                self.errorText = ""
                self.refreshState()
                self.onReady?()
            } else {
                self.loading = false
                self.errorText = "Failed to initialize Metal 4 Hardware Ray Tracer (requires Apple Silicon Metal RT)."
                self.onReady?()
            }
        }
    }

    private static func ensureMetalLib(metalDir: String) -> String {
        let fm = FileManager.default
        let metalSrc = (metalDir as NSString).appendingPathComponent("mandy_rt.metal")
        let libOut = (metalDir as NSString).appendingPathComponent("mandy_rt.metallib")
        let cachedOut = "/tmp/oa-mandy-workbench/mandy_rt.metallib"
        let srcDate = (try? fm.attributesOfItem(atPath: metalSrc)[.modificationDate] as? Date) ?? .distantPast
        if fm.fileExists(atPath: libOut) {
            let libDate = (try? fm.attributesOfItem(atPath: libOut)[.modificationDate] as? Date) ?? .distantPast
            if libDate >= srcDate { return libOut }
        }
        if fm.fileExists(atPath: cachedOut) {
            let libDate = (try? fm.attributesOfItem(atPath: cachedOut)[.modificationDate] as? Date) ?? .distantPast
            if libDate >= srcDate { return cachedOut }
        }
        // Try recompiling mandy_rt.metal into /tmp/oa-mandy-workbench/mandy_rt.metallib if Metal toolchain is mounted
        if fm.fileExists(atPath: metalSrc) {
            try? fm.createDirectory(atPath: "/tmp/oa-mandy-workbench", withIntermediateDirectories: true)
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/bin/bash")
            p.arguments = ["-lc", """
                MT=$(ls -d /var/run/com.apple.security.cryptexd/mnt/com.apple.MobileAsset.MetalToolchain*/Metal.xctoolchain/usr/bin 2>/dev/null | head -n 1)
                if [ -n "$MT" ]; then
                  "$MT/metal" -std=metal3.2 -O3 -I"\(metalDir)" -c "\(metalSrc)" -o /tmp/oa-mandy-workbench/mandy_rt.air && \
                  "$MT/metallib" /tmp/oa-mandy-workbench/mandy_rt.air -o "\(cachedOut)" && \
                  rm -f /tmp/oa-mandy-workbench/mandy_rt.air
                fi
                """]
            try? p.run()
            p.waitUntilExit()
            if p.terminationStatus == 0 && fm.fileExists(atPath: cachedOut) {
                return cachedOut
            }
        }
        return fm.fileExists(atPath: libOut) ? libOut : ""
    }

    private static func ensureLibraryLoaded(rootDir: String) -> String? {
        buildLock.lock()
        defer { buildLock.unlock() }
        if dlHandle != nil && fnCreate != nil { return nil }

        let fm = FileManager.default
        let metalDir = (rootDir as NSString).appendingPathComponent("metal")
        let mmPath = (metalDir as NSString).appendingPathComponent("mandy_metal.mm")
        let shPath = (metalDir as NSString).appendingPathComponent("mandy_shared.h")
        let workDir = "/tmp/oa-mandy-workbench"
        let dylibPath = "\(workDir)/libmandy_workbench_\(abiVersionTag).dylib"
        let genMmPath = "\(workDir)/mandy_workbench_abi.mm"

        try? fm.createDirectory(atPath: workDir, withIntermediateDirectories: true)

        let mmDate = (try? fm.attributesOfItem(atPath: mmPath)[.modificationDate] as? Date) ?? .distantPast
        let shDate = (try? fm.attributesOfItem(atPath: shPath)[.modificationDate] as? Date) ?? .distantPast
        let dyDate = (try? fm.attributesOfItem(atPath: dylibPath)[.modificationDate] as? Date) ?? .distantPast
        let needsBuild = !fm.fileExists(atPath: dylibPath) || dyDate < mmDate || dyDate < shDate

        if needsBuild {
            guard var src = try? String(contentsOfFile: mmPath, encoding: .utf8) else {
                return "Unable to read \(mmPath)"
            }
            // Transform standalone mandy_metal.mm into an async non-blocking Metal dylib without touching user source.
            let hooksDecl = """
            #include <atomic>
            static id<MTLTexture> g_oa_ext_target = nil;
            static void (*g_oa_ready_cb)(void *, void *, double, uint32_t) = nullptr;
            static void *g_oa_ready_user = nullptr;
            static std::atomic<int> *g_oa_in_flight = nullptr;
            static float g_oa_mandy_off[4] = { 0.0f, 0.0f, 0.0f, 0.0f };

            @implementation MandyMetalEngine
            """
            guard src.contains("@implementation MandyMetalEngine") else {
                return "Unexpected mandy_metal.mm layout (missing @implementation MandyMetalEngine)"
            }
            src = src.replacingOccurrences(of: "@implementation MandyMetalEngine", with: hooksDecl)

            // Apply live user drag offset (X, Y, Z, yaw) right after Mandy's mocap pose evaluation,
            // while keeping the camera track anchored to her base kitchen coordinate so the room stays still!
            let evalNeedle = "        rig.eval_pose(clock_s, anim_t);"
            let evalRepl = """
                    rig.eval_pose(clock_s, anim_t);
                    rig.char_x += g_oa_mandy_off[0];
                    rig.char_y += g_oa_mandy_off[1];
                    rig.char_z += g_oa_mandy_off[2];
                    rig.char_yaw += g_oa_mandy_off[3];
            """
            guard src.contains(evalNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing rig.eval_pose hook)"
            }
            src = src.replacingOccurrences(of: evalNeedle, with: evalRepl)

            let trkNeedle = """
                    float trk_x = cam.tx + rig.char_x * 0.62f;
                    float trk_y = cam.ty + rig.char_y * 0.50f;
                    float trk_z = cam.tz + rig.char_z * 0.62f;
            """
            let trkRepl = """
                    float trk_x = cam.tx + (rig.char_x - g_oa_mandy_off[0]) * 0.62f;
                    float trk_y = cam.ty + (rig.char_y - g_oa_mandy_off[1]) * 0.50f;
                    float trk_z = cam.tz + (rig.char_z - g_oa_mandy_off[2]) * 0.62f;
            """
            if src.contains(trkNeedle) {
                src = src.replacingOccurrences(of: trkNeedle, with: trkRepl)
            }

            let blitNeedle = "    if (drawable != nil) {"
            let blitRepl = """
                id<MTLTexture> oa_dst = g_oa_ext_target;
                void (*oa_cb)(void *, void *, double, uint32_t) = g_oa_ready_cb;
                void *oa_user = g_oa_ready_user;
                std::atomic<int> *oa_inflight = g_oa_in_flight;
                uint32_t oa_done_spp = live_fx ? 1u : (accum_count + 1u);
                g_oa_ext_target = nil; g_oa_ready_cb = nullptr; g_oa_ready_user = nullptr; g_oa_in_flight = nullptr;
                if (oa_dst != nil) {
                    id<MTLBlitCommandEncoder> blit = [cb blitCommandEncoder];
                    uint32_t cw = std::min((uint32_t)oa_dst.width, rt_w);
                    uint32_t ch = std::min((uint32_t)oa_dst.height, rt_h);
                    [blit copyFromTexture:tex_ldr sourceSlice:0 sourceLevel:0 sourceOrigin:MTLOriginMake(0,0,0)
                               sourceSize:MTLSizeMake(cw, ch, 1) toTexture:oa_dst destinationSlice:0 destinationLevel:0 destinationOrigin:MTLOriginMake(0,0,0)];
                    [blit endEncoding];
                } else if (drawable != nil) {
            """
            guard src.contains(blitNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing drawable blit hook)"
            }
            src = src.replacingOccurrences(of: blitNeedle, with: blitRepl)

            let semNeedle = "        dispatch_semaphore_signal(sem);"
            let semRepl = """
                    if (oa_inflight) oa_inflight->fetch_sub(1, std::memory_order_acq_rel);
                    if (oa_dst != nil && oa_cb) {
                        oa_cb(oa_user, (__bridge void *)oa_dst, weakSelf->gpu_ms_smooth, oa_done_spp);
                    }
                    dispatch_semaphore_signal(sem);
            """
            guard src.contains(semNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing completed handler semaphore)"
            }
            src = src.replacingOccurrences(of: semNeedle, with: semRepl)

            let waitNeedle = "    if (drawable == nil) [cb waitUntilCompleted];"
            let waitRepl = "    if (drawable == nil && oa_dst == nil) [cb waitUntilCompleted];"
            guard src.contains(waitNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing waitUntilCompleted hook)"
            }
            src = src.replacingOccurrences(of: waitNeedle, with: waitRepl)

            src = src.replacingOccurrences(of: "int main(int argc, const char **argv)", with: "int mandy_standalone_main(int argc, const char **argv)")

            let cAbiTail = """

            // ---- OpenAgents Desktop Embedded Metal 4 Workbench C ABI ----
            struct OaMandyCtx {
                MandyMetalEngine *engine;
                id<MTLTexture> ring[3];
                uint32_t ring_w;
                uint32_t ring_h;
                uint32_t ring_idx;
                std::atomic<int> in_flight{0};
                float off[4] = { 0.0f, 0.0f, 0.0f, 0.0f }; // X, Y, Z, yaw user placement offset
                bool drag_active = false;
                float drag_plane_y = 0.9f;
                simd_float3 drag_start_hit = { 0.0f, 0.0f, 0.0f };
                float drag_start_off[4] = { 0.0f, 0.0f, 0.0f, 0.0f };
            };

            static void oa_mandy_camera_ray(OaMandyCtx *c, float ndc_x, float ndc_y,
                                            simd_float3 &out_eye, simd_float3 &out_dir,
                                            simd_float3 &out_fwd, simd_float3 &out_right, simd_float3 &out_up) {
                MandyMetalEngine *e = c->engine;
                if (e->frame_idx > 0 && simd_length(e->prev_cam_fwd.xyz) > 0.1f) {
                    out_eye = e->prev_cam_pos.xyz;
                    out_fwd = simd_normalize(e->prev_cam_fwd.xyz);
                    out_right = simd_normalize(e->prev_cam_right.xyz);
                    out_up = simd_normalize(e->prev_cam_up.xyz);
                    float tan_h = e->prev_cam_pos.w;
                    float aspect = e->prev_cam_right.w;
                    out_dir = simd_normalize(out_fwd + out_right * (ndc_x * tan_h * aspect) + out_up * (ndc_y * tan_h));
                    return;
                }
                auto &cam = e->cam;
                float trk_x = cam.tx + (e->rig.char_x - c->off[0]) * 0.62f;
                float trk_y = cam.ty + (e->rig.char_y - c->off[1]) * 0.50f;
                float trk_z = cam.tz + (e->rig.char_z - c->off[2]) * 0.62f;
                float cp = cosf(cam.pitch), sp = sinf(cam.pitch), cy = cosf(cam.yaw), sy = sinf(cam.yaw);
                out_eye = f3(trk_x + cam.dist * cp * sy, trk_y + cam.dist * sp, trk_z + cam.dist * cp * cy);
                out_fwd = simd_normalize(f3(trk_x, trk_y, trk_z) - out_eye);
                out_right = simd_normalize(simd_cross(out_fwd, f3(0, 1, 0)));
                out_up = simd_normalize(simd_cross(out_right, out_fwd));
                float tan_h = tanf(cam.fov_deg * 0.5f * MN_PI / 180.0f);
                float aspect = float(e->rt_w) / float(std::max(1u, e->rt_h));
                out_dir = simd_normalize(out_fwd + out_right * (ndc_x * tan_h * aspect) + out_up * (ndc_y * tan_h));
            }

            static float oa_ray_seg_dist2(simd_float3 ro, simd_float3 rd, simd_float3 a, simd_float3 b, float &out_t, simd_float3 &out_p) {
                simd_float3 u = rd;
                simd_float3 v = b - a;
                simd_float3 w = ro - a;
                float a_dot = simd_dot(u, u);
                float b_dot = simd_dot(u, v);
                float c_dot = simd_dot(v, v);
                float d_dot = simd_dot(u, w);
                float e_dot = simd_dot(v, w);
                float denom = a_dot * c_dot - b_dot * b_dot;
                float sc, tc;
                if (denom < 1e-6f) {
                    sc = 0.0f;
                    tc = (c_dot > 1e-6f) ? mn_clamp(e_dot / c_dot, 0.0f, 1.0f) : 0.0f;
                } else {
                    sc = (b_dot * e_dot - c_dot * d_dot) / denom;
                    tc = (a_dot * e_dot - b_dot * d_dot) / denom;
                    if (sc < 0.0f) {
                        sc = 0.0f;
                        tc = (c_dot > 1e-6f) ? mn_clamp(e_dot / c_dot, 0.0f, 1.0f) : 0.0f;
                    }
                    if (tc < 0.0f) {
                        tc = 0.0f;
                        sc = std::max(0.0f, -d_dot / std::max(a_dot, 1e-6f));
                    } else if (tc > 1.0f) {
                        tc = 1.0f;
                        sc = std::max(0.0f, (b_dot - d_dot) / std::max(a_dot, 1e-6f));
                    }
                }
                simd_float3 pr = ro + u * sc;
                simd_float3 ps = a + v * tc;
                out_t = sc;
                out_p = pr;
                simd_float3 diff = pr - ps;
                return simd_dot(diff, diff);
            }

            static bool oa_mandy_hit_body(OaMandyCtx *c, float ndc_x, float ndc_y, float &out_hit_y) {
                MandyMetalEngine *e = c->engine;
                auto &rig = e->rig;
                simd_float3 ro, rd, fwd, right, up;
                oa_mandy_camera_ray(c, ndc_x, ndc_y, ro, rd, fwd, right, up);

                simd_float3 head_w = (rig.bi_head >= 0 && rig.bi_head < (int)rig.live_wp.size())
                    ? rig.rage_to_world(rig.live_wp[rig.bi_head])
                    : f3(rig.char_x, rig.char_y + 1.56f, rig.char_z);
                simd_float3 pelv_w = (rig.bi_pelvis >= 0 && rig.bi_pelvis < (int)rig.live_wp.size())
                    ? rig.rage_to_world(rig.live_wp[rig.bi_pelvis])
                    : f3(rig.char_x, rig.char_y + 0.92f, rig.char_z);
                simd_float3 feet_w = f3(pelv_w.x, rig.char_y + 0.05f, pelv_w.z);

                struct Cap { simd_float3 a, b; float r; };
                std::vector<Cap> caps;
                caps.push_back({ head_w - f3(0, 0.06f, 0), head_w + f3(0, 0.14f, 0), 0.19f });
                caps.push_back({ pelv_w, head_w, 0.25f });
                caps.push_back({ feet_w, pelv_w, 0.25f });
                if (rig.bi_l_uarm >= 0 && rig.bi_l_hand >= 0)
                    caps.push_back({ rig.rage_to_world(rig.live_wp[rig.bi_l_uarm]), rig.rage_to_world(rig.live_wp[rig.bi_l_hand]), 0.14f });
                if (rig.bi_r_uarm >= 0 && rig.bi_r_hand >= 0)
                    caps.push_back({ rig.rage_to_world(rig.live_wp[rig.bi_r_uarm]), rig.rage_to_world(rig.live_wp[rig.bi_r_hand]), 0.14f });

                float best_t = 1e9f;
                bool hit = false;
                for (const auto &cap : caps) {
                    float t = 0.0f; simd_float3 hp{};
                    float d2 = oa_ray_seg_dist2(ro, rd, cap.a, cap.b, t, hp);
                    if (d2 <= cap.r * cap.r && t > 0.05f && t < best_t) {
                        best_t = t;
                        out_hit_y = mn_clamp(hp.y, rig.char_y + 0.10f, rig.char_y + 1.65f);
                        hit = true;
                    }
                }
                return hit;
            }

            static bool oa_ray_plane_xz(simd_float3 ro, simd_float3 rd, float plane_y, simd_float3 &out_hit) {
                if (fabsf(rd.y) < 0.09f) return false;
                float t = (plane_y - ro.y) / rd.y;
                if (t <= 0.05f || t > 18.0f) return false;
                out_hit = ro + rd * t;
                return true;
            }

            static void oa_mandy_apply_kitchen_live(MandyMetalEngine *eng) {
                eng->rig.mode = MODE_CLIP1;
                eng->cam = { 0.42f, 0.06f, 2.05f, -0.08f, 1.02f, -0.02f, 44.0f, 0.004f };
                eng->paused = false;
                eng->use_rockstar_cam = false;
                eng->need_hair_reset = true;
                eng->cam_dirty = true;
            }

            static void oa_mandy_ensure_ring(OaMandyCtx *c, uint32_t w, uint32_t h) {
                if (c->ring[0] && c->ring_w == w && c->ring_h == h) return;
                MTLTextureDescriptor *d = [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm width:w height:h mipmapped:NO];
                d.usage = MTLTextureUsageShaderRead | MTLTextureUsageRenderTarget;
                d.storageMode = MTLStorageModePrivate;
                for (int i = 0; i < 3; ++i) c->ring[i] = [c->engine->device newTextureWithDescriptor:d];
                c->ring_w = w; c->ring_h = h; c->ring_idx = 0;
            }

            static void oa_mandy_drain(OaMandyCtx *c) {
                if (!c || !c->engine) return;
                // Drain both in-flight slots cleanly before resizing or destroying Metal resources.
                dispatch_semaphore_wait(c->engine->frame_sem, DISPATCH_TIME_FOREVER);
                dispatch_semaphore_wait(c->engine->frame_sem, DISPATCH_TIME_FOREVER);
                dispatch_semaphore_signal(c->engine->frame_sem);
                dispatch_semaphore_signal(c->engine->frame_sem);
            }

            extern "C" __attribute__((visibility("default")))
            void *oa_mandy_create(const char *asset_dir, const char *metallib_path, uint32_t w, uint32_t h) {
                @autoreleasepool {
                    if (!asset_dir || !metallib_path) return nullptr;
                    for (int i = 0; i < 4; ++i) g_oa_mandy_off[i] = 0.0f;
                    MandyMetalEngine *eng = [[MandyMetalEngine alloc] initWithAssetDir:std::string(asset_dir)
                                                                          metalLibPath:std::string(metallib_path)
                                                                                 width:std::max(256u, w & ~1u)
                                                                                height:std::max(256u, h & ~1u)];
                    if (!eng) return nullptr;
                    oa_mandy_apply_kitchen_live(eng);
                    eng->use_denoiser = true;
                    OaMandyCtx *c = new OaMandyCtx();
                    c->engine = eng;
                    oa_mandy_ensure_ring(c, eng->rt_w, eng->rt_h);
                    return (void *)c;
                }
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_destroy(void *raw) {
                if (!raw) return;
                @autoreleasepool {
                    OaMandyCtx *c = (OaMandyCtx *)raw;
                    oa_mandy_drain(c);
                    for (int i = 0; i < 3; ++i) c->ring[i] = nil;
                    c->engine = nil;
                    delete c;
                }
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_resize(void *raw, uint32_t w, uint32_t h) {
                if (!raw) return;
                @autoreleasepool {
                    OaMandyCtx *c = (OaMandyCtx *)raw;
                    uint32_t ew = std::max(256u, w & ~1u);
                    uint32_t eh = std::max(256u, h & ~1u);
                    if (c->engine->rt_w == ew && c->engine->rt_h == eh) return;
                    oa_mandy_drain(c);
                    [c->engine allocateRenderTargetsWidth:ew height:eh];
                    oa_mandy_ensure_ring(c, ew, eh);
                    c->engine->cam_dirty = true;
                }
            }

            extern "C" __attribute__((visibility("default")))
            int oa_mandy_step_async(void *raw, float dt, uint32_t spp, int reset_accum,
                                    void (*ready_cb)(void *, void *, double, uint32_t), void *user) {
                if (!raw) return 0;
                @autoreleasepool {
                    OaMandyCtx *c = (OaMandyCtx *)raw;
                    if (c->in_flight.load(std::memory_order_acquire) >= 2) return 0;
                    for (int i = 0; i < 4; ++i) g_oa_mandy_off[i] = c->off[i];
                    oa_mandy_ensure_ring(c, c->engine->rt_w, c->engine->rt_h);
                    id<MTLTexture> dst = c->ring[c->ring_idx % 3u];
                    c->ring_idx = (c->ring_idx + 1u) % 3u;
                    c->in_flight.fetch_add(1, std::memory_order_acq_rel);
                    g_oa_ext_target = dst;
                    g_oa_ready_cb = ready_cb;
                    g_oa_ready_user = user;
                    g_oa_in_flight = &c->in_flight;
                    uint32_t use_spp = spp > 0 ? spp : (c->engine->paused ? 4u : 1u);
                    [c->engine renderStepDt:std::min(std::max(dt, 0.0f), 0.05f) spp:use_spp resetAccum:(reset_accum != 0) drawable:nil];
                    return 1;
                }
            }

            extern "C" __attribute__((visibility("default")))
            int oa_mandy_hit_test(void *raw, float ndc_x, float ndc_y) {
                if (!raw) return 0;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                float hit_y = 0.9f;
                return oa_mandy_hit_body(c, ndc_x, ndc_y, hit_y) ? 1 : 0;
            }

            extern "C" __attribute__((visibility("default")))
            int oa_mandy_begin_drag(void *raw, float ndc_x, float ndc_y, int force_grab) {
                if (!raw) return 0;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                float hit_y = c->engine->rig.char_y + 0.95f;
                bool hit = oa_mandy_hit_body(c, ndc_x, ndc_y, hit_y);
                if (!hit && !force_grab) return 0;
                simd_float3 ro, rd, fwd, right, up;
                oa_mandy_camera_ray(c, ndc_x, ndc_y, ro, rd, fwd, right, up);
                c->drag_plane_y = hit_y;
                simd_float3 plane_hit{};
                if (!oa_ray_plane_xz(ro, rd, c->drag_plane_y, plane_hit)) {
                    plane_hit = f3(c->engine->rig.char_x, c->drag_plane_y, c->engine->rig.char_z);
                }
                c->drag_start_hit = plane_hit;
                for (int i = 0; i < 4; ++i) c->drag_start_off[i] = c->off[i];
                c->drag_active = true;
                return 1;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_drag_to(void *raw, float ndc_x, float ndc_y, float dx, float dy, int mode) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                if (!c->drag_active) return;
                float prev_x = c->off[0], prev_y = c->off[1], prev_z = c->off[2];
                if (mode == 2) {
                    // Option + drag: rotate Mandy in place
                    c->off[3] += dx * 0.014f;
                } else if (mode == 1) {
                    // Shift + drag: lift / lower Mandy along Y
                    float sc = std::max(0.6f, c->engine->cam.dist) * 0.0024f;
                    c->off[1] = mn_clamp(c->off[1] + dy * sc, -0.5f, 2.2f);
                } else {
                    // Floor-plane slide (X/Z): 1:1 cursor ray-plane tracking with shallow-angle fallback
                    simd_float3 ro, rd, fwd, right, up;
                    oa_mandy_camera_ray(c, ndc_x, ndc_y, ro, rd, fwd, right, up);
                    simd_float3 cur_hit{};
                    if (oa_ray_plane_xz(ro, rd, c->drag_plane_y, cur_hit)) {
                        c->off[0] = mn_clamp(c->drag_start_off[0] + (cur_hit.x - c->drag_start_hit.x), -6.0f, 6.0f);
                        c->off[2] = mn_clamp(c->drag_start_off[2] + (cur_hit.z - c->drag_start_hit.z), -6.0f, 6.0f);
                    } else {
                        simd_float3 r_xz = simd_normalize(f3(right.x, 0.0f, right.z));
                        simd_float3 f_xz = simd_normalize(f3(fwd.x, 0.0f, fwd.z));
                        float sc = std::max(0.6f, c->engine->cam.dist) * 0.0026f;
                        c->off[0] = mn_clamp(c->off[0] + (r_xz.x * dx + f_xz.x * dy) * sc, -6.0f, 6.0f);
                        c->off[2] = mn_clamp(c->off[2] + (r_xz.z * dx + f_xz.z * dy) * sc, -6.0f, 6.0f);
                    }
                }
                float step_d = sqrtf((c->off[0] - prev_x)*(c->off[0] - prev_x) +
                                     (c->off[1] - prev_y)*(c->off[1] - prev_y) +
                                     (c->off[2] - prev_z)*(c->off[2] - prev_z));
                if (step_d > 0.45f) c->engine->need_hair_reset = true;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_end_drag(void *raw) {
                if (!raw) return;
                ((OaMandyCtx *)raw)->drag_active = false;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_rotate(void *raw, float delta_yaw) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->off[3] += delta_yaw;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_reset_pos(void *raw) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                for (int i = 0; i < 4; ++i) { c->off[i] = 0.0f; g_oa_mandy_off[i] = 0.0f; }
                c->drag_active = false;
                c->engine->need_hair_reset = true;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_get_offset(void *raw, float out_f4[4]) {
                if (!raw || !out_f4) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                for (int i = 0; i < 4; ++i) out_f4[i] = c->off[i];
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_orbit(void *raw, float dx, float dy) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_rockstar_cam = false;
                c->engine->cam.yaw -= dx * 0.007f;
                c->engine->cam.pitch = mn_clamp(c->engine->cam.pitch - dy * 0.005f, -0.55f, 0.95f);
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_pan(void *raw, float dx, float dy) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                float cp = cosf(cam.pitch), sp = sinf(cam.pitch), cy = cosf(cam.yaw), sy = sinf(cam.yaw);
                simd_float3 fwd = simd_normalize(f3(-cp * sy, -sp, -cp * cy));
                simd_float3 right = simd_normalize(simd_cross(fwd, f3(0, 1, 0)));
                simd_float3 up = simd_cross(right, fwd);
                float sc = cam.dist * 0.0018f;
                cam.tx -= right.x * dx * sc + up.x * dy * sc;
                cam.ty -= right.y * dx * sc + up.y * dy * sc;
                cam.tz -= right.z * dx * sc + up.z * dy * sc;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_zoom(void *raw, float delta) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                cam.dist = mn_clamp(cam.dist * (1.0f - delta * 0.004f), 0.50f, 6.0f);
                float zt = mn_clamp((1.8f - cam.dist) / 0.9f, 0.0f, 1.0f);
                cam.fov_deg = 42.0f - 12.0f * zt;
                cam.aperture = 0.003f + 0.009f * zt;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_move_wasd(void *raw, float fwd_in, float right_in, float up_in, float dt) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                float cp = cosf(cam.pitch), sp = sinf(cam.pitch), cy = cosf(cam.yaw), sy = sinf(cam.yaw);
                simd_float3 fwd = simd_normalize(f3(-cp * sy, -sp, -cp * cy));
                simd_float3 right = simd_normalize(simd_cross(fwd, f3(0, 1, 0)));
                float spd = std::max(0.45f, cam.dist * 0.85f) * dt;
                simd_float3 mv = fwd * (fwd_in * spd) + right * (right_in * spd) + f3(0, up_in * spd, 0);
                cam.tx += mv.x; cam.ty += mv.y; cam.tz += mv.z;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_preset(void *raw, const char *name) {
                if (!raw || !name) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                std::string s(name);
                if (s == "kitchen_live") {
                    oa_mandy_apply_kitchen_live(c->engine);
                    return;
                }
                c->engine->use_rockstar_cam = false;
                apply_preset(c->engine->rig, c->engine->cam, s);
                if (s == "portrait_live" || s == "showcase") {
                    c->engine->paused = false;
                }
                c->engine->need_hair_reset = true;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_paused(void *raw, int paused) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->paused = (paused != 0);
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_denoiser(void *raw, int on) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_denoiser = (on != 0);
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_rockstar_cam(void *raw, int on) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->use_rockstar_cam = (on != 0);
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_cycle_clip(void *raw) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->rig.mode = (c->engine->rig.mode + 1) % 7;
                c->engine->paused = false;
                c->engine->need_hair_reset = true;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_cycle_expr(void *raw) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->engine->rig.expr = (c->engine->rig.expr + 1) % 6;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_get_state(void *raw, int32_t out_i[8], double out_d[4]) {
                if (!raw || !out_i || !out_d) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                out_i[0] = c->engine->paused ? 1 : 0;
                out_i[1] = c->engine->use_denoiser ? 1 : 0;
                out_i[2] = c->engine->use_rockstar_cam ? 1 : 0;
                out_i[3] = c->engine->rig.mode;
                out_i[4] = c->engine->rig.expr;
                out_i[5] = (int32_t)(c->engine->paused ? (c->engine->accum_count * 4u) : 1u);
                out_i[6] = (int32_t)c->engine->rt_w;
                out_i[7] = (int32_t)c->engine->rt_h;
                out_d[0] = c->engine->gpu_ms_smooth;
                out_d[1] = c->engine->clock_s;
                out_d[2] = c->engine->anim_t;
                out_d[3] = (double)c->engine->frame_idx;
            }

            extern "C" __attribute__((visibility("default")))
            int oa_mandy_capture_png(void *raw, uint32_t spp, const char *out_path) {
                if (!raw || !out_path) return 0;
                @autoreleasepool {
                    OaMandyCtx *c = (OaMandyCtx *)raw;
                    oa_mandy_drain(c);
                    [c->engine renderHighSPPSnapshot:(spp > 0 ? spp : 64u) toPNG:std::string(out_path)];
                    return 1;
                }
            }
            """
            src += cAbiTail
            do {
                try src.write(toFile: genMmPath, atomically: true, encoding: .utf8)
            } catch {
                return "Failed writing \(genMmPath): \(error.localizedDescription)"
            }
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/usr/bin/clang++")
            p.arguments = [
                "-std=c++20", "-O3", "-fobjc-arc", "-arch", "arm64", "-dynamiclib",
                "-I", metalDir,
                genMmPath, "-o", dylibPath,
                "-framework", "Metal", "-framework", "MetalFX",
                "-framework", "Cocoa", "-framework", "QuartzCore",
                "-framework", "CoreGraphics", "-framework", "ImageIO",
                "-framework", "UniformTypeIdentifiers"
            ]
            let errPipe = Pipe()
            p.standardError = errPipe
            do {
                try p.run()
                p.waitUntilExit()
            } catch {
                return "Failed invoking clang++: \(error.localizedDescription)"
            }
            if p.terminationStatus != 0 {
                let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
                let errMsg = String(data: errData, encoding: .utf8) ?? "clang++ failed"
                return "Failed compiling GTA 6 Metal 4 workbench dylib: \(errMsg.prefix(400))"
            }
        }

        guard let h = dlopen(dylibPath, RTLD_NOW | RTLD_LOCAL) else {
            let msg = dlerror().map { String(cString: $0) } ?? "dlopen failed"
            return "Failed loading \(dylibPath): \(msg)"
        }
        func sym<T>(_ name: String, as type: T.Type) -> T? {
            guard let p = dlsym(h, name) else { return nil }
            return unsafeBitCast(p, to: T.self)
        }
        dlHandle = h
        fnCreate = sym("oa_mandy_create", as: FnCreate.self)
        fnDestroy = sym("oa_mandy_destroy", as: FnDestroy.self)
        fnResize = sym("oa_mandy_resize", as: FnResize.self)
        fnStepAsync = sym("oa_mandy_step_async", as: FnStepAsync.self)
        fnOrbit = sym("oa_mandy_orbit", as: FnOrbit.self)
        fnPan = sym("oa_mandy_pan", as: FnPan.self)
        fnZoom = sym("oa_mandy_zoom", as: FnZoom.self)
        fnMoveWasd = sym("oa_mandy_move_wasd", as: FnMoveWasd.self)
        fnSetPreset = sym("oa_mandy_set_preset", as: FnSetPreset.self)
        fnSetPaused = sym("oa_mandy_set_paused", as: FnSetInt.self)
        fnSetDenoiser = sym("oa_mandy_set_denoiser", as: FnSetInt.self)
        fnSetRockstarCam = sym("oa_mandy_set_rockstar_cam", as: FnSetInt.self)
        fnCycleClip = sym("oa_mandy_cycle_clip", as: FnVoid.self)
        fnCycleExpr = sym("oa_mandy_cycle_expr", as: FnVoid.self)
        fnGetState = sym("oa_mandy_get_state", as: FnGetState.self)
        fnCapturePng = sym("oa_mandy_capture_png", as: FnCapturePng.self)
        fnHitTest = sym("oa_mandy_hit_test", as: FnHitTest.self)
        fnBeginDrag = sym("oa_mandy_begin_drag", as: FnBeginDrag.self)
        fnDragTo = sym("oa_mandy_drag_to", as: FnDragTo.self)
        fnEndDrag = sym("oa_mandy_end_drag", as: FnVoid.self)
        fnRotate = sym("oa_mandy_rotate", as: FnRotate.self)
        fnResetPos = sym("oa_mandy_reset_pos", as: FnVoid.self)
        fnGetOffset = sym("oa_mandy_get_offset", as: FnGetOffset.self)
        return fnCreate != nil ? nil : "Missing oa_mandy_* symbols in \(dylibPath)"
    }

    func requestResize(size: CGSize) {
        let dims = Self.clampedRenderDims(for: size)
        if dims.0 == state.width && dims.1 == state.height {
            pendingResizeSize = nil
            return
        }
        pendingResizeSize = dims
    }

    func stepAsync(dt: Float) {
        guard let c = ctx, !destroyed, let fnStep = Self.fnStepAsync else { return }
        if let pending = pendingResizeSize {
            let now = CACurrentMediaTime()
            if now - lastResizeAppliedAt >= 0.14 {
                pendingResizeSize = nil
                lastResizeAppliedAt = now
                Self.fnResize?(c, pending.0, pending.1)
                state.width = pending.0
                state.height = pending.1
            }
        }
        let me = Unmanaged.passUnretained(self).toOpaque()
        let cb: ReadyC = { user, texPtr, gpuMs, spp in
            guard let user, let texPtr else { return }
            let tex = Unmanaged<AnyObject>.fromOpaque(texPtr).takeUnretainedValue() as! MTLTexture
            let session = Unmanaged<GtaWorkbenchSession>.fromOpaque(user).takeUnretainedValue()
            DispatchQueue.main.async { [weak session] in
                guard let session, !session.destroyed else { return }
                let now = CACurrentMediaTime()
                if session.lastFrameWall > 0 {
                    let fdt = now - session.lastFrameWall
                    if fdt > 0.001 && fdt < 1.0 {
                        session.smoothFps = session.smoothFps * 0.88 + (1.0 / fdt) * 0.12
                    }
                }
                session.lastFrameWall = now
                session.latestTexture = tex
                session.state.gpuMs = gpuMs
                session.state.accumSpp = spp
                session.onFrameTextureReady?(tex)
            }
        }
        _ = fnStep(c, dt, 0, 0, cb, me)
    }

    func refreshState() {
        guard let c = ctx, let fnGet = Self.fnGetState else { return }
        var iv = [Int32](repeating: 0, count: 8)
        var dv = [Double](repeating: 0, count: 4)
        fnGet(c, &iv, &dv)
        state.paused = iv[0] != 0
        state.denoiserOn = iv[1] != 0
        state.rockstarCam = iv[2] != 0
        state.clipIndex = iv[3]
        state.exprIndex = iv[4]
        state.accumSpp = UInt32(max(1, iv[5]))
        state.width = UInt32(max(0, iv[6]))
        state.height = UInt32(max(0, iv[7]))
        state.gpuMs = dv[0]
        if let fnOff = Self.fnGetOffset {
            var ov = [Float](repeating: 0, count: 4)
            fnOff(c, &ov)
            state.offsetX = ov[0]
            state.offsetY = ov[1]
            state.offsetZ = ov[2]
            state.offsetYaw = ov[3]
        }
    }

    func hitTestMandy(ndcX: Float, ndcY: Float) -> Bool {
        guard let c = ctx, let fnHit = Self.fnHitTest else { return false }
        return fnHit(c, ndcX, ndcY) != 0
    }

    func beginDragMandy(ndcX: Float, ndcY: Float, forceGrab: Bool = false) -> Bool {
        guard let c = ctx, let fnBegin = Self.fnBeginDrag else { return false }
        return fnBegin(c, ndcX, ndcY, forceGrab ? 1 : 0) != 0
    }

    func dragMandyTo(ndcX: Float, ndcY: Float, dx: Float, dy: Float, mode: Int32) {
        guard let c = ctx else { return }
        Self.fnDragTo?(c, ndcX, ndcY, dx, dy, mode)
        refreshOffset()
    }

    func endDragMandy() {
        guard let c = ctx else { return }
        Self.fnEndDrag?(c)
    }

    func rotateMandy(deltaYaw: Float) {
        guard let c = ctx else { return }
        Self.fnRotate?(c, deltaYaw)
        refreshOffset()
    }

    func resetMandyPosition() {
        guard let c = ctx else { return }
        Self.fnResetPos?(c)
        refreshOffset()
        onStateChanged?()
    }

    private func refreshOffset() {
        guard let c = ctx, let fnOff = Self.fnGetOffset else { return }
        var ov = [Float](repeating: 0, count: 4)
        fnOff(c, &ov)
        state.offsetX = ov[0]
        state.offsetY = ov[1]
        state.offsetZ = ov[2]
        state.offsetYaw = ov[3]
    }

    func orbit(dx: Float, dy: Float) {
        guard let c = ctx else { return }
        Self.fnOrbit?(c, dx, dy)
        state.rockstarCam = false
    }

    func pan(dx: Float, dy: Float) {
        guard let c = ctx else { return }
        Self.fnPan?(c, dx, dy)
        state.rockstarCam = false
    }

    func zoom(delta: Float) {
        guard let c = ctx else { return }
        Self.fnZoom?(c, delta)
        state.rockstarCam = false
    }

    func moveWasd(fwd: Float, right: Float, up: Float, dt: Float) {
        guard let c = ctx else { return }
        Self.fnMoveWasd?(c, fwd, right, up, dt)
        state.rockstarCam = false
    }

    func applyPreset(_ name: String) {
        guard let c = ctx else { return }
        state.presetName = name
        name.withCString { Self.fnSetPreset?(c, $0) }
        refreshState()
        onStateChanged?()
    }

    func togglePaused() {
        guard let c = ctx else { return }
        Self.fnSetPaused?(c, state.paused ? 0 : 1)
        refreshState()
        onStateChanged?()
    }

    func toggleDenoiser() {
        guard let c = ctx else { return }
        Self.fnSetDenoiser?(c, state.denoiserOn ? 0 : 1)
        refreshState()
        onStateChanged?()
    }

    func toggleRockstarCam() {
        guard let c = ctx else { return }
        Self.fnSetRockstarCam?(c, state.rockstarCam ? 0 : 1)
        refreshState()
        onStateChanged?()
    }

    func cycleClip() {
        guard let c = ctx else { return }
        Self.fnCycleClip?(c)
        refreshState()
        onStateChanged?()
    }

    func cycleExpr() {
        guard let c = ctx else { return }
        Self.fnCycleExpr?(c)
        refreshState()
        onStateChanged?()
    }

    func capturePngData(spp: UInt32 = 64) -> Data? {
        guard let c = ctx, let fnCap = Self.fnCapturePng else { return nil }
        let tmp = "/tmp/oa-mandy-workbench/snap-\(Int(Date().timeIntervalSince1970)).png"
        let ok = tmp.withCString { fnCap(c, spp, $0) }
        guard ok != 0 else { return nil }
        let data = try? Data(contentsOf: URL(fileURLWithPath: tmp))
        try? FileManager.default.removeItem(atPath: tmp)
        return data
    }

    var hudSummary: String {
        if loading { return " GTA VI Key Lento · Initializing Metal 4 RT + 6 BLASes… " }
        if !errorText.isEmpty { return " GTA VI Workbench: \(errorText) " }
        let modeStr = state.paused ? "CONVERGE \(state.accumSpp) SPP" : (state.denoiserOn ? "SVGF 2x" : "RAW PT")
        let camStr = state.rockstarCam ? "R* CAM" : state.presetName
        let clipName = (state.clipIndex >= 0 && Int(state.clipIndex) < Self.clipNames.count) ? Self.clipNames[Int(state.clipIndex)] : "stage"
        let moved = abs(state.offsetX) > 0.005 || abs(state.offsetY) > 0.005 || abs(state.offsetZ) > 0.005 || abs(state.offsetYaw) > 0.01
        let posStr = moved
            ? String(format: " · Mandy (%+.2f, %+.2f, %+.2f) %.0f°", state.offsetX, state.offsetY, state.offsetZ, state.offsetYaw * 180.0 / .pi)
            : ""
        return String(format: " GTA VI Key Lento · %.1f FPS (GPU %.1fms) · %@ · %@ · %@%@ ", smoothFps, state.gpuMs, modeStr, camStr, clipName, posStr)
    }
}

#endif
