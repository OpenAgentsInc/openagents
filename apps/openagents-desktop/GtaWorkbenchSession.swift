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
    var autoCamMode: Int32 = 1 // 0 = Off, 1 = Orbit, 2 = Kitchen Tour
    var customAnim: Int32 = 1  // 0 = Rockstar Mocap, 1..5 = Custom Procedural Animations
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
    private static var fnSetAutoCam: FnSetInt?
    private static var fnSetCustomAnim: FnSetInt?

    private static let buildLock = NSLock()
    private static let abiVersionTag = "oa_mandy_abi_v9"

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
    static let customAnims: [(id: Int32, label: String, shortName: String)] = [
        (1, "Custom: Hair Tousle & Hip Sway", "hair_tousle"),
        (2, "Custom: Hip Roll & Groove", "hip_groove"),
        (3, "Custom: Hair Flip & Pose", "hair_flip"),
        (4, "Custom: Late-Night Stretch", "kitchen_stretch"),
        (5, "Custom: Kitchen Twirl (360°)", "kitchen_twirl"),
        (0, "Original: Rockstar Mocap", "mocap")
    ]

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
            let rigDeclNeedle = "struct MandyRig {"
            let rigDeclRepl = """
            static int g_oa_custom_anim = 1;
            static float g_oa_custom_time = 0.0f;
            static inline float oa_smoothstep(float e0, float e1, float x) {
                float t = mn_clamp((x - e0) / std::max(1e-5f, e1 - e0), 0.0f, 1.0f);
                return t * t * (3.0f - 2.0f * t);
            }
            static inline simd_float3 oa_lerp3(simd_float3 a, simd_float3 b, float t) {
                return a + (b - a) * t;
            }
            // Exact orthonormal 3x3 basis (cols x, y, z) -> unit quaternion (x, y, z, w)
            static inline simd_float4 oa_quat_from_basis(simd_float3 bx, simd_float3 by, simd_float3 bz) {
                float m00 = bx.x, m01 = by.x, m02 = bz.x;
                float m10 = bx.y, m11 = by.y, m12 = bz.y;
                float m20 = bx.z, m21 = by.z, m22 = bz.z;
                float tr = m00 + m11 + m22;
                float qx, qy, qz, qw;
                if (tr > 0.0f) {
                    float s = sqrtf(tr + 1.0f) * 2.0f;
                    qw = 0.25f * s; qx = (m21 - m12) / s; qy = (m02 - m20) / s; qz = (m10 - m01) / s;
                } else if (m00 > m11 && m00 > m22) {
                    float s = sqrtf(1.0f + m00 - m11 - m22) * 2.0f;
                    qw = (m21 - m12) / s; qx = 0.25f * s; qy = (m01 + m10) / s; qz = (m02 + m20) / s;
                } else if (m11 > m22) {
                    float s = sqrtf(1.0f + m11 - m00 - m22) * 2.0f;
                    qw = (m02 - m20) / s; qx = (m01 + m10) / s; qy = 0.25f * s; qz = (m12 + m21) / s;
                } else {
                    float s = sqrtf(1.0f + m22 - m00 - m11) * 2.0f;
                    qw = (m10 - m01) / s; qx = (m02 + m20) / s; qy = (m12 + m21) / s; qz = 0.25f * s;
                }
                return q_unit(simd_make_float4(qx, qy, qz, qw));
            }
            struct MandyRig {
            """
            guard src.contains(rigDeclNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing struct MandyRig)"
            }
            src = src.replacingOccurrences(of: rigDeclNeedle, with: rigDeclRepl)

            // Inject anatomically accurate AAA 2-Bone Hinge Arm IK + RB_* roll-bone twist distribution +
            // true local-Z finger curl + contrapposto hip sway directly into MandyRig::eval_pose!
            let paletteNeedle = "        // Write 3x4 skinning matrix palette (3 float4 rows per bone!)"
            let customAnimBody = """
                    // ---- OpenAgents Anatomical Custom Procedural Animation Layer (1-DOF Hinge Arm IK + Roll Bones + Contrapposto) ----
                    if (!is_jason && !is_druid && mode != MODE_BIND && g_oa_custom_anim > 0 && bi_pelvis >= 0 && bi_head >= 0) {
                        phone_active = false;
                        cigar_active = false;
                        // Preserve live facial acting (blinks, warm smile, gaze); reset body bones cleanly to bind pose.
                        for (size_t i = 0; i < nb; ++i) {
                            int r = joints[i].role;
                            bool is_face = (r == ROLE_JAW || (r >= ROLE_L_EYE && r <= ROLE_R_CHEEK));
                            if (!is_face) {
                                live_lq[i] = joints[i].lq;
                                live_lt[i] = joints[i].lt;
                            }
                        }
                        run_fk();

                        // Stand clearly in front of the Key Lento counter with wide room for hair-combing & hip sway.
                        char_x = 0.22f;
                        char_y = 0.0f;
                        char_z = 0.16f;
                        char_yaw = 0.08f;

                        float bind_foot_z = (bi_l_foot >= 0 && bi_r_foot >= 0)
                            ? std::min(live_wp[bi_l_foot].z, live_wp[bi_r_foot].z) : -0.92f;

                        // AAA 1-DOF Hinge 2-Bone Arm IK with RB_Wrist / RB_Shoulder roll distribution & explicit palm orientation!
                        // `fingertip_target`: where her FINGERTIPS go (wrist is placed 10.5cm back along the approach!)
                        // `palm_dir_des`: desired world/char direction her PALM faces (e.g. inward toward skull/hair or hip!)
                        auto solve_arm_aaa = [&](int side, simd_float3 wrist_target, simd_float3 elbow_pole,
                                                 simd_float3 palm_dir_des, float wrist_flex, float finger_curl) {
                            int clav_bi = (side > 0) ? bi_l_clav : bi_r_clav;
                            int u_bi    = (side > 0) ? bi_l_uarm : bi_r_uarm;
                            int f_bi    = (side > 0) ? bi_l_farm : bi_r_farm;
                            int h_bi    = (side > 0) ? bi_l_hand : bi_r_hand;
                            if (u_bi < 0 || f_bi < 0 || h_bi < 0) return;

                            // 1. Natural clavicle elevation & protraction so shoulders rise organically when hands go up!
                            if (clav_bi >= 0) {
                                float dz = wrist_target.z - live_wp[u_bi].z;
                                float lift = mn_clamp((dz + 0.06f) * 0.42f, -0.05f, 0.26f);
                                rot_char(clav_bi, f3(0, -1, 0), float(side) * lift);
                                float pro = mn_clamp((live_wp[u_bi].y - wrist_target.y) * 0.25f, -0.08f, 0.14f);
                                rot_char(clav_bi, f3(0, 0, 1), -float(side) * pro);
                                run_fk();
                            }

                            simd_float3 sh  = live_wp[u_bi];
                            simd_float3 el0 = live_wp[f_bi];
                            simd_float3 ha0 = live_wp[h_bi];
                            float L1 = std::max(0.10f, simd_length(el0 - sh));
                            float L2 = std::max(0.10f, simd_length(ha0 - el0));

                            // 2. Law of Cosines in the exact shoulder->wrist->pole plane
                            simd_float3 to_t = wrist_target - sh;
                            float raw_d = simd_length(to_t);
                            float d = mn_clamp(raw_d, fabsf(L1 - L2) + 0.045f, (L1 + L2) * 0.965f);
                            simd_float3 dir_t = (raw_d > 1e-5f) ? (to_t / raw_d) : f3(float(side) * 0.5f, -0.5f, -0.7f);

                            float cos_a = mn_clamp((L1 * L1 + d * d - L2 * L2) / (2.0f * L1 * d), -1.0f, 1.0f);
                            float sin_a = sqrtf(std::max(0.0f, 1.0f - cos_a * cos_a));
                            simd_float3 pdir = elbow_pole - sh;
                            simd_float3 bend = pdir - dir_t * simd_dot(pdir, dir_t);
                            float bl = simd_length(bend);
                            bend = (bl > 1e-5f) ? (bend / bl) : f3(float(side), 0.1f, -0.2f);
                            simd_float3 el_des = sh + dir_t * (L1 * cos_a) + bend * (L1 * sin_a);

                            // 3. Exact zero-twist orthonormal basis for UpperArm (X = sh->el, Z = exact elbow hinge cross(u_x, f_x)!)
                            simd_float3 u_x = simd_normalize(el_des - sh);
                            simd_float3 f_x = simd_normalize((sh + dir_t * d) - el_des);
                            simd_float3 hz = simd_cross(u_x, f_x);
                            float hl = simd_length(hz);
                            simd_float3 u_z = (hl > 1e-4f) ? (hz / hl) : simd_normalize(simd_cross(u_x, bend));
                            simd_float3 u_y = simd_normalize(simd_cross(u_z, u_x));
                            simd_float4 q_uarm_w = oa_quat_from_basis(u_x, u_y, u_z);

                            int u_par = joints[u_bi].par;
                            simd_float4 pq = (u_par >= 0) ? live_wq[u_par] : simd_make_float4(0, 0, 0, 1);
                            live_lq[u_bi] = q_unit(q_mul(q_conj(pq), q_uarm_w));

                            // 3b. Smooth Shoulder/Deltoid Roll Distribution across RB_Shoulder01/02/03 (exact swing-twist around humerus +X!)
                            // Eliminates bicep/deltoid creasing when arms lift behind head or overhead.
                            {
                                simd_float4 dq = q_unit(q_mul(q_conj(joints[u_bi].lq), live_lq[u_bi]));
                                float arm_roll = 2.0f * atan2f(dq.x, dq.w);
                                if (arm_roll >  MN_PI) arm_roll -= 2.0f * MN_PI;
                                if (arm_roll < -MN_PI) arm_roll += 2.0f * MN_PI;
                                arm_roll = mn_clamp(arm_roll, -1.65f, 1.65f);
                                int rs1 = (side > 0) ? 101 : 242; // RB_*_Shoulder01 (x=0.000, deltoid cap)
                                int rs2 = (side > 0) ? 105 : 246; // RB_*_Shoulder02 (x=0.091, upper bicep)
                                int rs3 = (side > 0) ? 102 : 243; // RB_*_Shoulder03 (x=0.181, lower bicep)
                                if (rs1 < (int)nb && joints[rs1].par == u_bi)
                                    live_lq[rs1] = q_unit(q_mul(joints[rs1].lq, q_axis(f3(1, 0, 0), -arm_roll * 0.72f)));
                                if (rs2 < (int)nb && joints[rs2].par == u_bi)
                                    live_lq[rs2] = q_unit(q_mul(joints[rs2].lq, q_axis(f3(1, 0, 0), -arm_roll * 0.42f)));
                                if (rs3 < (int)nb && joints[rs3].par == u_bi)
                                    live_lq[rs3] = q_unit(q_mul(joints[rs3].lq, q_axis(f3(1, 0, 0), -arm_roll * 0.16f)));
                            }

                            // 4. Pure 1-DOF Hinge Bend on Forearm around local +Z (ZERO sideways/backward elbow distortion!)
                            float elbow_bend = acosf(mn_clamp(simd_dot(u_x, f_x), -1.0f, 1.0f));
                            elbow_bend = mn_clamp(elbow_bend, 0.08f, 2.38f);
                            live_lq[f_bi] = q_axis(f3(0, 0, 1), elbow_bend);
                            run_fk();

                            // 5. Distribute Forearm Pronation/Supination smoothly across RB_Wrist03/02/01 -> Hand!
                            // Verified from mandy-meta.bin: Left palm normal = local +Z, Right palm normal = local -Z!
                            simd_float3 cur_fx   = q_rot(live_wq[f_bi], f3(1, 0, 0));
                            simd_float3 cur_palm = q_rot(live_wq[f_bi], f3(0, 0, float(side)));
                            simd_float3 des_p = palm_dir_des - cur_fx * simd_dot(palm_dir_des, cur_fx);
                            float dpl = simd_length(des_p);
                            float pronation = 0.0f;
                            if (dpl > 1e-4f) {
                                des_p /= dpl;
                                float c_ang = mn_clamp(simd_dot(cur_palm, des_p), -1.0f, 1.0f);
                                float s_ang = simd_dot(cur_fx, simd_cross(cur_palm, des_p));
                                pronation = mn_clamp(atan2f(s_ang, c_ang), -1.55f, 1.55f);
                            }

                            // Roll-bone indices verified from mandy-joints.txt:
                            // Left:  RB_L_Wrist03=95 (25%), RB_L_Wrist02=96 (50%), RB_L_Wrist01=97 (75%)
                            // Right: RB_R_Wrist03=241 (25%), RB_R_Wrist02=240 (50%), RB_R_Wrist01=236 (75%)
                            int rw3 = (side > 0) ? 95 : 241;
                            int rw2 = (side > 0) ? 96 : 240;
                            int rw1 = (side > 0) ? 97 : 236;
                            if (rw3 < (int)nb && joints[rw3].par == f_bi)
                                live_lq[rw3] = q_unit(q_mul(joints[rw3].lq, q_axis(f3(1, 0, 0), pronation * 0.25f)));
                            if (rw2 < (int)nb && joints[rw2].par == f_bi)
                                live_lq[rw2] = q_unit(q_mul(joints[rw2].lq, q_axis(f3(1, 0, 0), pronation * 0.50f)));
                            if (rw1 < (int)nb && joints[rw1].par == f_bi)
                                live_lq[rw1] = q_unit(q_mul(joints[rw1].lq, q_axis(f3(1, 0, 0), pronation * 0.75f)));

                            // Hand gets full pronation + soft anatomical wrist flexion (strictly clamped <= 26 deg!)
                            float wf = mn_clamp(wrist_flex, -0.42f, 0.45f);
                            live_lq[h_bi] = q_unit(q_mul(q_axis(f3(1, 0, 0), pronation), q_axis(f3(0, 1, 0), float(side) * wf)));

                            // 6. Natural Finger Curl strictly around BONE-LOCAL -Z (verified from mandy-meta.bin!)
                            int f_role  = (side > 0) ? ROLE_L_FINGER : ROLE_R_FINGER;
                            int th_role = (side > 0) ? ROLE_L_THUMB  : ROLE_R_THUMB;
                            for (size_t i = 0; i < nb; ++i) {
                                if (joints[i].role == f_role) {
                                    // Local Z curl curls fingertips cleanly into the palm on both L and R hands!
                                    live_lq[i] = q_unit(q_mul(joints[i].lq, q_axis(f3(0, 0, 1), -finger_curl)));
                                } else if (joints[i].role == th_role) {
                                    live_lq[i] = q_unit(q_mul(joints[i].lq, q_axis(f3(0, 0, 1), -finger_curl * 0.35f)));
                                }
                            }
                            run_fk();
                        };

                        float t = g_oa_custom_time;
                        float hip_x = 0, hip_y = 0, hip_z = 0;
                        float pel_roll = 0, pel_yaw = 0, pel_pitch = 0;
                        float sp_roll = 0, sp_yaw = 0, sp_pitch = 0;
                        float head_yaw = 0, head_pitch = 0, head_roll = 0;
                        float extra_yaw = 0;
                        float breath = sinf(t * 1.45f);

                        if (g_oa_custom_anim == 1) {
                            // 1. HAIR TOUSLE & HIP SWAY (12s fluid 3-beat choreography!)
                            float cyc = fmodf(t, 12.0f);
                            float sway = sinf(t * 1.15f);
                            float fig8 = sinf(t * 2.30f);
                            hip_x     =  0.072f * sway;
                            hip_y     = -0.020f * cosf(t * 2.30f);
                            hip_z     = -0.022f - 0.010f * fabsf(sway);
                            pel_roll  = -0.155f * sway;
                            pel_yaw   =  0.125f * sinf(t * 1.15f + 0.55f);
                            pel_pitch = -0.045f + 0.028f * fig8;
                            sp_roll   =  0.175f * sway;
                            sp_yaw    = -0.140f * sinf(t * 1.15f + 0.45f);
                            sp_pitch  = -0.060f - 0.028f * breath;

                            if (cyc < 4.5f) {
                                float b = sinf((cyc / 4.5f) * MN_PI);
                                head_roll  = -0.14f * b;
                                head_yaw   =  0.12f * b;
                                head_pitch = -0.08f * b;
                            } else if (cyc < 9.0f) {
                                float b = sinf(((cyc - 4.5f) / 4.5f) * MN_PI);
                                head_roll  =  0.14f * b;
                                head_yaw   = -0.12f * b;
                                head_pitch = -0.08f * b;
                            } else {
                                float b = sinf(((cyc - 9.0f) / 3.0f) * MN_PI);
                                head_roll  =  0.06f * sinf(t * 2.2f) * b;
                                head_pitch = -0.12f * b;
                                sp_pitch  -=  0.065f * b;
                            }
                        } else if (g_oa_custom_anim == 2) {
                            // 2. HIP ROLL & GROOVE (sensual slow-tempo figure-8 hip circles + shoulder groove!)
                            float w = t * 1.95f;
                            float sx = sinf(w), cx = cosf(w);
                            hip_x     =  0.092f * sx;
                            hip_y     = -0.036f * sinf(2.0f * w);
                            hip_z     = -0.028f - 0.014f * fabsf(sx);
                            pel_roll  = -0.210f * sx;
                            pel_yaw   =  0.185f * cx;
                            pel_pitch = -0.055f + 0.052f * cosf(2.0f * w);
                            sp_roll   =  0.230f * sinf(w - 0.35f);
                            sp_yaw    = -0.200f * cosf(w - 0.35f);
                            sp_pitch  = -0.048f - 0.042f * sinf(2.0f * w - 0.5f);
                            head_roll = -0.11f * sinf(w - 0.65f);
                            head_yaw  =  0.10f * cosf(w - 0.65f);
                            head_pitch = -0.03f + 0.04f * sinf(2.0f * w);
                        } else if (g_oa_custom_anim == 3) {
                            // 3. HAIR FLIP & EDITORIAL POSE (6.5s dramatic slow-mo hair whip + tuck behind ear!)
                            float cyc = fmodf(t, 6.5f);
                            float dip  = oa_smoothstep(0.0f, 1.3f, cyc) * (1.0f - oa_smoothstep(1.3f, 2.3f, cyc));
                            float whip = oa_smoothstep(1.3f, 2.5f, cyc) * (1.0f - oa_smoothstep(4.8f, 6.4f, cyc));
                            hip_x     = -0.048f * dip + 0.082f * whip;
                            hip_z     = -0.032f * dip - 0.018f * whip;
                            pel_roll  =  0.10f * dip - 0.18f * whip;
                            pel_yaw   = -0.14f * dip + 0.22f * whip;
                            sp_roll   = -0.22f * dip + 0.19f * whip;
                            sp_pitch  =  0.24f * dip - 0.12f * whip;
                            sp_yaw    = -0.16f * dip - 0.18f * whip;
                            head_roll = -0.34f * dip + 0.20f * whip;
                            head_pitch = 0.32f * dip - 0.15f * whip;
                            head_yaw  = -0.20f * dip + 0.22f * whip;
                        } else if (g_oa_custom_anim == 4) {
                            // 4. LATE-NIGHT KITCHEN STRETCH (7.5s arms-high cozy stretch + back arch)
                            float cyc = fmodf(t, 7.5f);
                            float st = oa_smoothstep(0.4f, 2.2f, cyc) * (1.0f - oa_smoothstep(4.6f, 6.6f, cyc));
                            float sway = sinf(t * 0.9f);
                            hip_x     =  0.045f * sway;
                            hip_y     = -0.030f * st;
                            hip_z     = -0.012f + 0.016f * st;
                            pel_roll  = -0.090f * sway;
                            pel_pitch = -0.090f * st;
                            sp_pitch  = -0.180f * st - 0.025f * breath;
                            sp_roll   =  0.110f * sway;
                            head_pitch = -0.20f * st;
                            head_roll  =  0.09f * sinf(t * 1.1f) * st;
                        } else if (g_oa_custom_anim == 5) {
                            // 5. KITCHEN TWIRL (7.0s smooth playful 360-degree spin!)
                            float cyc = fmodf(t, 7.0f);
                            float spin_u = oa_smoothstep(0.6f, 5.2f, cyc);
                            float spin_vel = (cyc > 0.6f && cyc < 5.2f) ? sinf(((cyc - 0.6f) / 4.6f) * MN_PI) : 0.0f;
                            extra_yaw = spin_u * (2.0f * MN_PI);
                            hip_x     = 0.052f * sinf(t * 1.6f);
                            hip_z     = -0.018f + 0.014f * spin_vel;
                            pel_roll  = -0.12f * sinf(t * 1.6f);
                            sp_yaw    =  0.18f * spin_vel;
                            sp_pitch  = -0.07f * spin_vel;
                            head_yaw  =  0.22f * spin_vel;
                            head_roll = -0.09f * spin_vel;
                        }

                        char_yaw += extra_yaw;

                        // Pelvis translation + contrapposto rotations
                        live_lt[an_root_idx].x += hip_x;
                        live_lt[an_root_idx].y += hip_y;
                        live_lt[an_root_idx].z += hip_z;
                        rot_char(bi_pelvis, f3(0, 1, 0), pel_roll);
                        rot_char(bi_pelvis, f3(0, 0, 1), pel_yaw);
                        rot_char(bi_pelvis, f3(1, 0, 0), pel_pitch);
                        run_fk();

                        // S-curve counter-rotation distributed cleanly across Spine0..Spine5
                        const int sp_bones[6] = { bi_spine0, bi_spine1, bi_spine2, bi_spine3, bi_spine4, bi_spine5 };
                        const float sp_w[6]   = { 0.14f, 0.16f, 0.18f, 0.20f, 0.18f, 0.14f };
                        for (int k = 0; k < 6; ++k) {
                            if (sp_bones[k] < 0) continue;
                            rot_char(sp_bones[k], f3(0, 1, 0), sp_roll * sp_w[k]);
                            rot_char(sp_bones[k], f3(0, 0, 1), sp_yaw  * sp_w[k]);
                            rot_char(sp_bones[k], f3(1, 0, 0), sp_pitch * sp_w[k]);
                            run_fk();
                        }

                        // Expressive Neck & Head
                        int nk = (bi_neck1 >= 0) ? bi_neck1 : bi_neck0;
                        if (nk >= 0) {
                            rot_char(nk, f3(0, 0, 1), head_yaw   * 0.35f);
                            rot_char(nk, f3(1, 0, 0), head_pitch * 0.35f);
                            rot_char(nk, f3(0, 1, 0), head_roll  * 0.35f);
                            run_fk();
                        }
                        rot_char(bi_head, f3(0, 0, 1), head_yaw   * 0.65f);
                        rot_char(bi_head, f3(1, 0, 0), head_pitch * 0.65f);
                        rot_char(bi_head, f3(0, 1, 0), head_roll  * 0.65f);
                        run_fk();

                        // Graceful natural contrapposto stance (subtle staggered weight shift, soft knees, slight toe turnout!)
                        float l_unload = mn_clamp((-hip_x / 0.085f) * 0.5f + 0.5f, 0.0f, 1.0f);
                        float r_unload = mn_clamp(( hip_x / 0.085f) * 0.5f + 0.5f, 0.0f, 1.0f);
                        for (int side = 0; side < 2; ++side) {
                            float sgn = (side == 0) ? 1.0f : -1.0f;
                            float unl = (side == 0) ? l_unload : r_unload;
                            int bi_th = (side == 0) ? bi_l_thigh : bi_r_thigh;
                            int bi_cf = (side == 0) ? bi_l_calf  : bi_r_calf;
                            int bi_ft = (side == 0) ? bi_l_foot  : bi_r_foot;
                            float stagger = (side == 0) ? -0.06f : 0.04f; // left foot slightly forward for natural feminine silhouette!
                            rot_char(bi_th, f3(0, 1, 0), -pel_roll * 0.88f + sgn * 0.035f);
                            rot_char(bi_th, f3(0, 0, 1), -pel_yaw  * 0.72f + sgn * 0.11f);
                            rot_char(bi_th, f3(1, 0, 0), stagger - 0.06f - 0.22f * unl);
                            run_fk();
                            rot_char(bi_cf, f3(1, 0, 0),  0.10f + 0.34f * unl);
                            run_fk();
                            rot_char(bi_ft, f3(1, 0, 0), -0.04f - 0.11f * unl);
                        }
                        run_fk();

                        if (bi_l_foot >= 0 && bi_r_foot >= 0) {
                            float min_fz = std::min(live_wp[bi_l_foot].z, live_wp[bi_r_foot].z);
                            live_lt[an_root_idx].z += (bind_foot_z - min_fz);
                            run_fk();
                        }

                        // Live anatomical landmarks AFTER pelvis/spine/head motion
                        simd_float3 pel_p  = live_wp[bi_pelvis];
                        simd_float3 head_p = live_wp[bi_head];
                        simd_float3 l_sh   = (bi_l_uarm >= 0) ? live_wp[bi_l_uarm] : (pel_p + f3( 0.16f, 0, 0.42f));
                        simd_float3 r_sh   = (bi_r_uarm >= 0) ? live_wp[bi_r_uarm] : (pel_p + f3(-0.16f, 0, 0.42f));

                        // Natural relaxed hip-rest WRIST positions (wrist sits outside hip so fingertips rest cleanly on hip!)
                        simd_float3 l_hip_w = pel_p + f3( 0.245f,  0.025f, 0.105f);
                        simd_float3 r_hip_w = pel_p + f3(-0.245f,  0.025f, 0.105f);
                        simd_float3 l_hip_p = l_sh  + f3( 0.46f,   0.22f, -0.16f);
                        simd_float3 r_hip_p = r_sh  + f3(-0.46f,   0.22f, -0.16f);
                        simd_float3 l_hip_palm = f3(-0.85f, -0.25f, -0.45f);
                        simd_float3 r_hip_palm = f3( 0.85f, -0.25f, -0.45f);

                        simd_float3 l_w = l_hip_w, r_w = r_hip_w;
                        simd_float3 l_p = l_hip_p, r_p = r_hip_p;
                        simd_float3 l_palm = l_hip_palm, r_palm = r_hip_palm;
                        float l_flex = 0.24f, r_flex = 0.24f;
                        float l_curl = 0.34f, r_curl = 0.34f;

                        // Smooth, anatomically calibrated WRIST trajectory for combing fingers through hair:
                        // Hand glides UP close along her ribcage/shoulder (never poking sideways!) and WRIST stays
                        // 11-13cm outward/below temple/crown so FINGERTIPS comb right through her hair waves!
                        auto hair_comb_wrist = [&](int side, float u, simd_float3 hip_w, simd_float3 sh_p,
                                                   simd_float3 &out_w, simd_float3 &out_p, simd_float3 &out_palm,
                                                   float &out_flex, float &out_curl) {
                            float s = float(side);
                            simd_float3 w_hip    = hip_w;
                            simd_float3 w_lift   = sh_p   + f3(s * 0.145f, -0.110f, -0.165f); // close, graceful upward glide along ribcage!
                            simd_float3 w_cheek  = head_p + f3(s * 0.180f, -0.055f, -0.070f); // fingertips graze cheek/jawline into hair!
                            simd_float3 w_temple = head_p + f3(s * 0.178f, -0.010f,  0.000f); // fingertips comb right through temple waves!
                            simd_float3 w_back   = head_p + f3(s * 0.182f,  0.085f,  0.010f); // fingertips sweep back behind ear!
                            simd_float3 w_drop   = sh_p   + f3(s * 0.195f,  0.030f, -0.085f); // soft feminine float back down to hip!

                            if (u < 0.22f) {
                                float k = oa_smoothstep(0.0f, 0.22f, u);
                                out_w = oa_lerp3(w_hip, w_lift, k);
                            } else if (u < 0.42f) {
                                float k = oa_smoothstep(0.22f, 0.42f, u);
                                out_w = oa_lerp3(w_lift, w_cheek, k);
                            } else if (u < 0.62f) {
                                float k = oa_smoothstep(0.42f, 0.62f, u);
                                out_w = oa_lerp3(w_cheek, w_temple, k);
                            } else if (u < 0.80f) {
                                float k = oa_smoothstep(0.62f, 0.80f, u);
                                out_w = oa_lerp3(w_temple, w_back, k);
                            } else {
                                simd_float3 m = oa_lerp3(w_back, w_drop, oa_smoothstep(0.80f, 0.90f, u));
                                out_w = oa_lerp3(m, w_hip, oa_smoothstep(0.86f, 1.0f, u));
                            }
                            // Elbow stays relaxed & low during initial lift, rising smoothly ONLY as hand nears cheek/hair!
                            float high = oa_smoothstep(0.22f, 0.48f, u) * (1.0f - oa_smoothstep(0.80f, 0.98f, u));
                            out_p = oa_lerp3(sh_p + f3(s * 0.40f, 0.20f, -0.22f), sh_p + f3(s * 0.54f, -0.05f, -0.03f), high);
                            // Palm faces INWARD toward her ribcage/head/hair throughout the entire motion!
                            out_palm = oa_lerp3(f3(-s * 0.85f, -0.25f, -0.45f), f3(-s * 0.92f, 0.22f, -0.18f), high);
                            out_flex = 0.20f - 0.08f * high;
                            out_curl = 0.24f + 0.18f * high; // soft open fingertips during lift, gentle comb curl in hair!
                        };

                        if (g_oa_custom_anim == 1) {
                            // 1. HAIR TOUSLE & HIP SWAY: Beat A (R combs hair) -> Beat B (L combs hair) -> Beat C (Both lift behind nape!)
                            float cyc = fmodf(t, 12.0f);
                            if (cyc < 4.5f) {
                                hair_comb_wrist(-1, cyc / 4.5f, r_hip_w, r_sh, r_w, r_p, r_palm, r_flex, r_curl);
                                l_w = l_hip_w + f3(0.012f * sinf(t * 1.6f), 0.0f, 0.012f * cosf(t * 1.6f));
                            } else if (cyc < 9.0f) {
                                hair_comb_wrist( 1, (cyc - 4.5f) / 4.5f, l_hip_w, l_sh, l_w, l_p, l_palm, l_flex, l_curl);
                                r_w = r_hip_w + f3(-0.012f * sinf(t * 1.6f), 0.0f, 0.012f * cosf(t * 1.6f));
                            } else {
                                // Beat C: Both wrists glide up close along sides & cradle behind nape/hair!
                                float u = (cyc - 9.0f) / 3.0f;
                                float lift = oa_smoothstep(0.0f, 0.32f, u) * (1.0f - oa_smoothstep(0.68f, 1.0f, u));
                                float mid_arc = lift * (1.0f - lift) * 4.0f;
                                float tousle = sinf(u * 3.0f * MN_PI) * 0.018f * lift;
                                simd_float3 l_nape_w = head_p + f3( 0.185f + tousle, 0.075f, -0.020f + 0.022f * lift);
                                simd_float3 r_nape_w = head_p + f3(-0.185f - tousle, 0.075f, -0.020f + 0.022f * lift);
                                l_w = oa_lerp3(l_hip_w, l_nape_w, lift) + f3(-0.03f * mid_arc, -0.07f * mid_arc, 0.0f);
                                r_w = oa_lerp3(r_hip_w, r_nape_w, lift) + f3( 0.03f * mid_arc, -0.07f * mid_arc, 0.0f);
                                float el_hi = oa_smoothstep(0.25f, 0.78f, lift);
                                l_p = oa_lerp3(l_hip_p, l_sh + f3( 0.54f, -0.04f, 0.04f), el_hi);
                                r_p = oa_lerp3(r_hip_p, r_sh + f3(-0.54f, -0.04f, 0.04f), el_hi);
                                l_palm = oa_lerp3(l_hip_palm, f3(-0.85f, -0.45f, -0.15f), lift);
                                r_palm = oa_lerp3(r_hip_palm, f3( 0.85f, -0.45f, -0.15f), lift);
                                l_flex = 0.18f; r_flex = 0.18f;
                                l_curl = 0.28f + 0.18f * lift; r_curl = 0.28f + 0.18f * lift;
                            }
                        } else if (g_oa_custom_anim == 2) {
                            // 2. HIP ROLL & GROOVE: sensual hands gliding gracefully along waist & lower ribs!
                            float w = t * 1.95f;
                            float l_u = 0.5f + 0.5f * sinf(w * 0.5f);
                            float r_u = 0.5f + 0.5f * cosf(w * 0.5f);
                            l_w = oa_lerp3(l_hip_w, pel_p + f3( 0.215f, -0.045f, 0.21f + 0.05f * sinf(w)), l_u * 0.75f);
                            r_w = oa_lerp3(r_hip_w, pel_p + f3(-0.215f, -0.045f, 0.21f + 0.05f * cosf(w)), r_u * 0.75f);
                            l_p = l_sh + f3( 0.42f, 0.20f, -0.22f);
                            r_p = r_sh + f3(-0.42f, 0.20f, -0.22f);
                            l_curl = 0.28f; r_curl = 0.28f;
                        } else if (g_oa_custom_anim == 3) {
                            // 3. HAIR FLIP & EDITORIAL POSE: right fingertips gracefully brush hair behind ear!
                            float cyc = fmodf(t, 6.5f);
                            float tuck = oa_smoothstep(2.0f, 3.0f, cyc) * (1.0f - oa_smoothstep(5.0f, 6.3f, cyc));
                            float arc = tuck * (1.0f - tuck) * 4.0f;
                            r_w = oa_lerp3(r_hip_w, head_p + f3(-0.180f, 0.030f, -0.020f), tuck) + f3(0.03f * arc, -0.07f * arc, 0.0f);
                            r_p = oa_lerp3(r_hip_p, r_sh + f3(-0.54f, -0.04f, -0.02f), oa_smoothstep(0.25f, 0.75f, tuck));
                            r_palm = oa_lerp3(r_hip_palm, f3(0.92f, 0.18f, -0.20f), tuck);
                            r_curl = 0.26f + 0.16f * tuck;
                        } else if (g_oa_custom_anim == 4) {
                            // 4. LATE-NIGHT STRETCH: both arms sweep smoothly overhead (graceful arc, relaxed wrists!)
                            float cyc = fmodf(t, 7.5f);
                            float st = oa_smoothstep(0.4f, 2.2f, cyc) * (1.0f - oa_smoothstep(4.6f, 6.6f, cyc));
                            float arc = st * (1.0f - st) * 4.0f;
                            simd_float3 l_hi = head_p + f3( 0.22f, -0.02f, 0.25f);
                            simd_float3 r_hi = head_p + f3(-0.22f, -0.02f, 0.25f);
                            l_w = oa_lerp3(l_hip_w, l_hi, st) + f3( 0.04f * arc, -0.08f * arc, 0.0f);
                            r_w = oa_lerp3(r_hip_w, r_hi, st) + f3(-0.04f * arc, -0.08f * arc, 0.0f);
                            float el_hi = oa_smoothstep(0.25f, 0.80f, st);
                            l_p = oa_lerp3(l_hip_p, l_sh + f3( 0.50f, -0.10f, 0.24f), el_hi);
                            r_p = oa_lerp3(r_hip_p, r_sh + f3(-0.50f, -0.10f, 0.24f), el_hi);
                            l_palm = oa_lerp3(l_hip_palm, f3(-0.7f, -0.6f, 0.3f), st);
                            r_palm = oa_lerp3(r_hip_palm, f3( 0.7f, -0.6f, 0.3f), st);
                            l_curl = 0.26f * (1.0f - 0.5f * st); r_curl = 0.26f * (1.0f - 0.5f * st);
                        } else if (g_oa_custom_anim == 5) {
                            // 5. KITCHEN TWIRL: graceful arms-out spin then brushing right hair back!
                            float cyc = fmodf(t, 7.0f);
                            float flare = oa_smoothstep(0.5f, 2.0f, cyc) * (1.0f - oa_smoothstep(4.0f, 5.4f, cyc));
                            float brush = oa_smoothstep(4.8f, 5.6f, cyc) * (1.0f - oa_smoothstep(6.2f, 7.0f, cyc));
                            simd_float3 l_out = l_sh + f3( 0.34f, -0.06f, -0.16f);
                            simd_float3 r_out = r_sh + f3(-0.34f, -0.06f, -0.16f);
                            l_w = oa_lerp3(l_hip_w, l_out, flare);
                            r_w = oa_lerp3(oa_lerp3(r_hip_w, r_out, flare), head_p + f3(-0.180f, 0.03f, -0.02f), brush);
                            l_p = oa_lerp3(l_hip_p, l_sh + f3( 0.44f, 0.14f, -0.18f), flare);
                            r_p = oa_lerp3(r_hip_p, r_sh + f3(-0.52f, -0.04f, 0.02f), std::max(flare * 0.5f, brush));
                            r_palm = oa_lerp3(r_hip_palm, f3(0.92f, 0.18f, -0.20f), brush);
                            l_curl = 0.24f; r_curl = 0.28f + 0.12f * brush;
                        }

                        solve_arm_aaa( 1, l_w, l_p, l_palm, l_flex, l_curl);
                        solve_arm_aaa(-1, r_w, r_p, r_palm, r_flex, r_curl);
                    }

                    // Write 3x4 skinning matrix palette (3 float4 rows per bone!)
            """
            guard src.contains(paletteNeedle) else {
                return "Unexpected mandy_metal.mm layout (missing skinning palette hook)"
            }
            src = src.replacingOccurrences(of: paletteNeedle, with: customAnimBody)

            // Add subtle hand capsules so XPBD hair parts naturally over her hands without exploding
            let capNeedle = """
                    if (bi_r_uarm >= 0 && bi_r_farm >= 0)
                        add(rage_to_world(live_wp[bi_r_uarm]), 0.056f, rage_to_world(live_wp[bi_r_farm]), 0.048f);
            """
            let capRepl = """
                    if (bi_r_uarm >= 0 && bi_r_farm >= 0)
                        add(rage_to_world(live_wp[bi_r_uarm]), 0.056f, rage_to_world(live_wp[bi_r_farm]), 0.048f);
                    if (bi_l_farm >= 0 && bi_l_hand >= 0)
                        add(rage_to_world(live_wp[bi_l_farm]), 0.040f, rage_to_world(live_wp[bi_l_hand]), 0.042f);
                    if (bi_r_farm >= 0 && bi_r_hand >= 0)
                        add(rage_to_world(live_wp[bi_r_farm]), 0.040f, rage_to_world(live_wp[bi_r_hand]), 0.042f);
            """
            if src.contains(capNeedle) {
                src = src.replacingOccurrences(of: capNeedle, with: capRepl)
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
                int auto_cam_mode = 1;   // 0=Off, 1=Steadicam Orbit, 2=Key Lento Kitchen Tour
                float auto_cam_t = 0.0f;
                int custom_anim = 1;     // 0=Rockstar Mocap, 1=Hair Tousle & Hips, 2=Hip Groove, 3=Hair Flip, 4=Stretch, 5=Twirl
                float custom_time = 0.0f;
            };

            static void oa_mandy_step_auto_cam(OaMandyCtx *c, float dt) {
                if (c->auto_cam_mode == 0 || c->engine->use_rockstar_cam || c->engine->paused) return;
                c->auto_cam_t += std::min(std::max(dt, 0.0f), 0.05f);
                auto &cam = c->engine->cam;
                if (c->auto_cam_mode == 1) {
                    // Wall-safe Steadicam sweep strictly inside the open Key Lento kitchen corridor (zero pillar clipping!)
                    float t = c->auto_cam_t;
                    float s1 = sinf(t * 0.44f);
                    float c1 = cosf(t * 0.29f);
                    float s2 = sinf(t * 0.22f + 0.4f);
                    cam.yaw      = 0.10f + 0.40f * s1;           // clean -0.30 .. +0.50 rad sweep
                    cam.pitch    = 0.05f + 0.08f * c1;           // gentle boom up/down
                    cam.dist     = 1.88f + 0.26f * s2;           // smooth dolly in/out
                    cam.tx       = 0.14f - 0.08f * s1;
                    cam.ty       = 1.06f + 0.12f * (0.5f - 0.5f * s2);
                    cam.tz       = 0.10f + 0.06f * std::max(0.0f, s1);
                    cam.fov_deg  = 43.0f + 3.0f * s2;
                    cam.aperture = 0.0040f + 0.0025f * (0.5f - 0.5f * s2);
                } else if (c->auto_cam_mode == 2) {
                    // Full Key Lento Kitchen Tour: smooth cosine-interpolated dolly across 6 interior viewpoints!
                    static const CameraState wp[6] = {
                        {  0.42f,  0.06f, 2.05f, -0.08f, 1.02f, -0.02f, 44.0f, 0.0040f }, // Kitchen Counter Wide
                        {  0.14f,  0.03f, 1.24f,  0.02f, 1.44f, -0.02f, 32.0f, 0.0085f }, // Hero Portrait Close-Up
                        { -0.44f,  0.04f, 1.98f, -0.16f, 1.16f,  0.14f, 41.0f, 0.0055f }, // Over-Counter Lean
                        { -0.46f, -0.06f, 1.88f, -0.04f, 0.94f,  0.10f, 52.0f, 0.0030f }, // Low Wide Archway
                        {  0.94f,  0.20f, 2.08f, -0.52f, 0.90f,  0.92f, 54.0f, 0.0030f }, // Two-Shot (Mandy + Jason)
                        {  0.58f,  0.16f, 2.18f, -0.26f, 1.14f,  0.34f, 46.0f, 0.0040f }, // High Chandelier Sweep
                    };
                    float seg_dur = 4.5f;
                    float total = seg_dur * 6.0f;
                    float u = fmodf(c->auto_cam_t, total) / seg_dur;
                    int i0 = ((int)floorf(u)) % 6;
                    int i1 = (i0 + 1) % 6;
                    float frac = u - floorf(u);
                    float a = 0.5f - 0.5f * cosf(frac * MN_PI); // C1 smooth ease in/out
                    auto lerp = [&](float p0, float p1) { return p0 + (p1 - p0) * a; };
                    cam.yaw      = lerp(wp[i0].yaw, wp[i1].yaw);
                    cam.pitch    = lerp(wp[i0].pitch, wp[i1].pitch);
                    cam.dist     = lerp(wp[i0].dist, wp[i1].dist);
                    cam.tx       = lerp(wp[i0].tx, wp[i1].tx);
                    cam.ty       = lerp(wp[i0].ty, wp[i1].ty);
                    cam.tz       = lerp(wp[i0].tz, wp[i1].tz);
                    cam.fov_deg  = lerp(wp[i0].fov_deg, wp[i1].fov_deg);
                    cam.aperture = lerp(wp[i0].aperture, wp[i1].aperture);
                }
                c->engine->cam_dirty = true;
            }

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
                eng->cam = { 0.24f, 0.06f, 2.02f, 0.14f, 1.04f, 0.12f, 44.0f, 0.004f };
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
                    if (!c->engine->paused) c->custom_time += std::min(std::max(dt, 0.0f), 0.05f);
                    g_oa_custom_anim = c->custom_anim;
                    g_oa_custom_time = c->custom_time;
                    oa_mandy_step_auto_cam(c, dt);
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
            void oa_mandy_set_custom_anim(void *raw, int anim_id) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->custom_anim = std::max(0, std::min(5, anim_id));
                c->custom_time = 0.0f;
                g_oa_custom_anim = c->custom_anim;
                g_oa_custom_time = 0.0f;
                if (c->custom_anim > 0) {
                    c->engine->paused = false;
                    c->engine->rig.mode = MODE_CLIP1;
                }
                c->engine->need_hair_reset = true;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_auto_cam(void *raw, int mode) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = std::max(0, std::min(2, mode));
                if (c->auto_cam_mode != 0) {
                    c->engine->use_rockstar_cam = false;
                    c->engine->paused = false;
                }
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_orbit(void *raw, float dx, float dy) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = 0;
                c->engine->use_rockstar_cam = false;
                c->engine->cam.yaw -= dx * 0.0075f;
                c->engine->cam.pitch = mn_clamp(c->engine->cam.pitch - dy * 0.006f, -1.25f, 1.30f);
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_pan(void *raw, float dx, float dy) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = 0;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                float cp = cosf(cam.pitch), sp = sinf(cam.pitch), cy = cosf(cam.yaw), sy = sinf(cam.yaw);
                simd_float3 fwd = simd_normalize(f3(-cp * sy, -sp, -cp * cy));
                simd_float3 right = simd_normalize(simd_cross(fwd, f3(0, 1, 0)));
                simd_float3 up = simd_cross(right, fwd);
                float sc = cam.dist * 0.0022f;
                cam.tx -= right.x * dx * sc + up.x * dy * sc;
                cam.ty -= right.y * dx * sc + up.y * dy * sc;
                cam.tz -= right.z * dx * sc + up.z * dy * sc;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_zoom(void *raw, float delta) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = 0;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                cam.dist = mn_clamp(cam.dist * (1.0f - delta * 0.0045f), 0.35f, 9.0f);
                float zt = mn_clamp((1.8f - cam.dist) / 0.9f, 0.0f, 1.0f);
                cam.fov_deg = 44.0f - 14.0f * zt;
                cam.aperture = 0.003f + 0.009f * zt;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_move_wasd(void *raw, float fwd_in, float right_in, float up_in, float dt) {
                if (!raw) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = 0;
                c->engine->use_rockstar_cam = false;
                auto &cam = c->engine->cam;
                float cp = cosf(cam.pitch), sp = sinf(cam.pitch), cy = cosf(cam.yaw), sy = sinf(cam.yaw);
                simd_float3 fwd = simd_normalize(f3(-cp * sy, -sp, -cp * cy));
                simd_float3 right = simd_normalize(simd_cross(fwd, f3(0, 1, 0)));
                float spd = std::max(0.75f, cam.dist * 1.15f) * dt;
                simd_float3 mv = fwd * (fwd_in * spd) + right * (right_in * spd) + f3(0, up_in * spd, 0);
                cam.tx += mv.x; cam.ty += mv.y; cam.tz += mv.z;
                c->engine->cam_dirty = true;
            }

            extern "C" __attribute__((visibility("default")))
            void oa_mandy_set_preset(void *raw, const char *name) {
                if (!raw || !name) return;
                OaMandyCtx *c = (OaMandyCtx *)raw;
                c->auto_cam_mode = 0;
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
                out_d[1] = (double)c->custom_anim;
                out_d[2] = c->custom_time;
                out_d[3] = (double)c->auto_cam_mode;
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
        fnSetAutoCam = sym("oa_mandy_set_auto_cam", as: FnSetInt.self)
        fnSetCustomAnim = sym("oa_mandy_set_custom_anim", as: FnSetInt.self)
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
        state.customAnim = Int32(dv[1].rounded())
        state.autoCamMode = Int32(dv[3].rounded())
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

    func setCustomAnim(_ animId: Int32) {
        guard let c = ctx else { return }
        Self.fnSetCustomAnim?(c, animId)
        state.customAnim = animId
        if animId > 0 { state.paused = false }
        onStateChanged?()
    }

    func cycleCustomAnim() {
        let ids = Self.customAnims.map { $0.id }
        let curIdx = ids.firstIndex(of: state.customAnim) ?? 0
        setCustomAnim(ids[(curIdx + 1) % ids.count])
    }

    func cycleAutoCam() {
        guard let c = ctx else { return }
        let next = (state.autoCamMode + 1) % 3
        Self.fnSetAutoCam?(c, next)
        state.autoCamMode = next
        if next != 0 { state.rockstarCam = false; state.paused = false }
        onStateChanged?()
    }

    func setAutoCamMode(_ mode: Int32) {
        guard let c = ctx else { return }
        Self.fnSetAutoCam?(c, mode)
        state.autoCamMode = mode
        if mode != 0 { state.rockstarCam = false; state.paused = false }
        onStateChanged?()
    }

    func orbit(dx: Float, dy: Float) {
        guard let c = ctx else { return }
        Self.fnOrbit?(c, dx, dy)
        state.rockstarCam = false
        if state.autoCamMode != 0 { state.autoCamMode = 0; onStateChanged?() }
    }

    func pan(dx: Float, dy: Float) {
        guard let c = ctx else { return }
        Self.fnPan?(c, dx, dy)
        state.rockstarCam = false
        if state.autoCamMode != 0 { state.autoCamMode = 0; onStateChanged?() }
    }

    func zoom(delta: Float) {
        guard let c = ctx else { return }
        Self.fnZoom?(c, delta)
        state.rockstarCam = false
        if state.autoCamMode != 0 { state.autoCamMode = 0; onStateChanged?() }
    }

    func moveWasd(fwd: Float, right: Float, up: Float, dt: Float) {
        guard let c = ctx else { return }
        Self.fnMoveWasd?(c, fwd, right, up, dt)
        state.rockstarCam = false
        if state.autoCamMode != 0 { state.autoCamMode = 0; onStateChanged?() }
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
        let camStr = state.rockstarCam ? "R* CAM" : (state.autoCamMode == 1 ? "AUTO-ORBIT" : (state.autoCamMode == 2 ? "KITCHEN TOUR" : state.presetName))
        let animName: String
        if state.customAnim > 0, let entry = Self.customAnims.first(where: { $0.id == state.customAnim }) {
            animName = entry.shortName
        } else {
            animName = (state.clipIndex >= 0 && Int(state.clipIndex) < Self.clipNames.count) ? Self.clipNames[Int(state.clipIndex)] : "stage"
        }
        let moved = abs(state.offsetX) > 0.005 || abs(state.offsetY) > 0.005 || abs(state.offsetZ) > 0.005 || abs(state.offsetYaw) > 0.01
        let posStr = moved
            ? String(format: " · Mandy (%+.2f, %+.2f, %+.2f) %.0f°", state.offsetX, state.offsetY, state.offsetZ, state.offsetYaw * 180.0 / .pi)
            : ""
        return String(format: " GTA VI Key Lento · %.1f FPS (GPU %.1fms) · %@ · %@ · %@%@ ", smoothFps, state.gpuMs, modeStr, camStr, animName, posStr)
    }
}

#endif
