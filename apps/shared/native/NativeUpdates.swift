// Firm reason: 6 (WebKit, CryptoKit, URLSession and the iOS sandbox SDK).
import Foundation
import CryptoKit

enum NativeFailure: Error, LocalizedError {
    case refused(String)
    var errorDescription: String? { if case .refused(let text) = self { return text }; return "Unavailable" }
}

struct NativePackage {
    struct File { let path: String; let hash: String; let bytes: Int }
    let envelope: Data
    let manifest: Data
    let identity: String
    let generation: Int64
    let files: [File]
    static func hex(_ data: Data) -> String { data.map { String(format: "%02x", $0) }.joined() }
    static func number(_ raw: Data, _ index: Int64) -> Int64 {
        var b = [UInt8](raw)
        return b.withUnsafeMutableBufferPointer { cnu_number(coax_v_u8(data: $0.baseAddress, count: Int64($0.count)), index) }
    }
    static func text(_ raw: Data, _ row: Int64, _ field: Int64) -> String {
        var b = [UInt8](raw), out = [UInt8](repeating: 0, count: 128)
        let n = b.withUnsafeMutableBufferPointer { input in out.withUnsafeMutableBufferPointer { output in
            cnu_text(coax_v_u8(data: input.baseAddress,count: Int64(input.count)),row,field,coax_v_u8(data: output.baseAddress,count: Int64(output.count)))
        } }
        return n < 0 ? "" : String(decoding: out.prefix(Int(n)), as: UTF8.self)
    }
    init(envelope: Data, app: String, runtime: String, publicKey: Data, floor: Int64 = 0) throws {
        var encoded=[UInt8](envelope)
        func part(_ index:Int64,_ capacity:Int) throws -> Data {
            var out=[UInt8](repeating:0,count:capacity)
            let n=encoded.withUnsafeMutableBufferPointer { input in out.withUnsafeMutableBufferPointer { output in cnu_unwrap(coax_v_u8(data:input.baseAddress,count:Int64(input.count)),index,coax_v_u8(data:output.baseAddress,count:Int64(output.count))) } }
            guard n>=0 else {throw NativeFailure.refused("Invalid update envelope")};return Data(out.prefix(Int(n)))
        }
        let raw=try part(0,16384),signature=try part(1,64)
        let key = try Curve25519.Signing.PublicKey(rawRepresentation: publicKey)
        let signed = Data("Coax Native Package v1\0".utf8) + raw
        let valid = key.isValidSignature(signature, for: signed)
        guard valid else { throw NativeFailure.refused("Update signature refused") }
        let n = Self.number(raw,-2), generation = Self.number(raw,-1)
        guard n >= 1 && n <= 24 else { throw NativeFailure.refused("Invalid package manifest") }
        var files: [File] = []; var total: Int64 = 0
        for i in 0..<n { let size = Self.number(raw,i); total += size; files.append(File(path: Self.text(raw,i,0),hash: Self.text(raw,i,1),bytes: Int(size))) }
        guard cn_stage(valid,Self.text(raw,-1,0)==app,Self.text(raw,-1,1)==runtime,generation,floor,total,n) else { throw NativeFailure.refused("Update signature, runtime or generation refused") }
        self.envelope = envelope; manifest = raw; identity = Self.hex(Data(SHA256.hash(data: raw))); self.generation = generation; self.files = files
    }
    func validate(_ directory: URL) throws {
        for file in files {
            let data = try Data(contentsOf: directory.appendingPathComponent(file.path), options: .mappedIfSafe)
            guard data.count == file.bytes, Self.hex(Data(SHA256.hash(data: data))) == file.hash else { throw NativeFailure.refused("Update file failed integrity check") }
        }
    }
}

@MainActor final class NativeUpdates {
    let enabled: Bool
    let runtime: String
    let publicKey: Data
    let endpoints: [URL]
    let app: String
    let home: URL
    let embedded: URL
    private(set) var current: NativePackage?
    private(set) var pending: NativePackage?
    private(set) var trial = false
    private var checking = false
    private(set) var message = "Ready offline."

    init(bundle: Bundle = .main, home: URL? = nil, verificationKey: Data? = nil) throws {
        let configURL = bundle.url(forResource: "Native", withExtension: "plist")!
        let config = try PropertyListSerialization.propertyList(from: Data(contentsOf: configURL),format: nil) as! [String:Any]
        runtime = config["runtime"] as! String
        enabled = config["updatesEnabled"] as? Bool ?? false
        publicKey = verificationKey ?? Data(base64Encoded: config["publicKey"] as! String)!
        endpoints = (config["endpoints"] as! [String]).compactMap(URL.init(string:))
        app = bundle.bundleIdentifier ?? "com.openagents.app"
        embedded = bundle.url(forResource: "Starter", withExtension: nil)!
        self.home = home ?? FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("CoaxNativeUpdates",isDirectory: true)
        try FileManager.default.createDirectory(at: self.home,withIntermediateDirectories: true)
    }
    func read(_ name: String) -> NativePackage? {
        guard let data = try? Data(contentsOf: home.appendingPathComponent(name)) else { return nil }
        return try? NativePackage(envelope:data,app:app,runtime:runtime,publicKey:publicKey)
    }
    func directory(_ p: NativePackage) -> URL { home.appendingPathComponent(p.identity,isDirectory: true) }
    func boot() -> URL {
        current = nil; trial = false
        guard enabled else { message = "Updates are disabled. Running the embedded starter."; return embedded }
        if let staged=read("pending.cnu") { pending=staged; try? activate() }
        for name in ["current.cnu","previous.cnu"] {
            guard let p = read(name), (try? p.validate(directory(p))) != nil else { continue }
            let bad = home.appendingPathComponent(p.identity + ".failed")
            let attempt = home.appendingPathComponent(p.identity + ".trial")
            let healthy = home.appendingPathComponent(p.identity + ".healthy")
            let action=cn_launch(true,FileManager.default.fileExists(atPath:bad.path),FileManager.default.fileExists(atPath:healthy.path),FileManager.default.fileExists(atPath:attempt.path))
            if action==0 {continue}
            if action==3 {
                try? Data().write(to:bad,options:.atomic); continue
            }
            current = p; trial = action==1
            if trial { try? Data().write(to:attempt,options:.atomic) }
            message = "Signed update \(p.generation)."; return directory(p)
        }
        message = "Embedded starter. Ready offline."; return embedded
    }
    func healthy() {
        guard let p = current, trial else { return }
        guard cn_health(true,true,1) else { return }
        try? Data().write(to:home.appendingPathComponent(p.identity + ".healthy"),options:.atomic)
        try? FileManager.default.removeItem(at:home.appendingPathComponent(p.identity + ".trial"))
        trial = false
    }
    func failed() {
        if let p = current { try? Data().write(to:home.appendingPathComponent(p.identity + ".failed"),options:.atomic) }
    }
    static func download(_ url: URL, limit: Int) async throws -> Data {
        var request = URLRequest(url:url,cachePolicy:.reloadIgnoringLocalAndRemoteCacheData,timeoutInterval:25)
        request.setValue("no-cache",forHTTPHeaderField:"Cache-Control")
        let (bytes,response) = try await URLSession.shared.bytes(for:request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200, response.url?.host == url.host,
              response.expectedContentLength <= limit else { throw NativeFailure.refused("Update download refused") }
        var out = Data(); out.reserveCapacity(min(limit,Int(max(0,response.expectedContentLength))))
        for try await byte in bytes { if out.count >= limit { throw NativeFailure.refused("Update exceeds its declared size") }; out.append(byte) }
        return out
    }
    func check() async throws -> String {
        guard enabled else { return "Updates are disabled in this starter." }
        guard !checking else { return "Already checking for updates." }
        checking = true; defer { checking = false }
        let floor = read("highwater.cnu")?.generation ?? 0
        var last: Error = NativeFailure.refused("Updates unavailable")
        for base in endpoints {
            do {
                let envelope = try await Self.download(base.appendingPathComponent("latest.cnu"),limit:16456)
                let advertised = try NativePackage(envelope:envelope,app:app,runtime:runtime,publicKey:publicKey)
                if advertised.generation <= floor { return "You have the latest compatible update." }
                let p = try NativePackage(envelope:envelope,app:app,runtime:runtime,publicKey:publicKey,floor:floor)
                guard !FileManager.default.fileExists(atPath:home.appendingPathComponent(p.identity + ".failed").path) else { throw NativeFailure.refused("This update failed its trial") }
                let staging = home.appendingPathComponent("staging-"+UUID().uuidString,isDirectory:true)
                try FileManager.default.createDirectory(at:staging,withIntermediateDirectories:true)
                defer { try? FileManager.default.removeItem(at:staging) }
                for file in p.files {
                    let data = try await Self.download(base.appendingPathComponent("objects/"+file.hash),limit:file.bytes)
                    guard data.count == file.bytes, NativePackage.hex(Data(SHA256.hash(data:data))) == file.hash else { throw NativeFailure.refused("Update artifact digest mismatch") }
                    let target = staging.appendingPathComponent(file.path)
                    try FileManager.default.createDirectory(at:target.deletingLastPathComponent(),withIntermediateDirectories:true)
                    try data.write(to:target,options:.atomic)
                }
                try p.validate(staging)
                let target = directory(p)
                if !FileManager.default.fileExists(atPath:target.path) { try FileManager.default.moveItem(at:staging,to:target) }
                try envelope.write(to:home.appendingPathComponent("pending.cnu"),options:.atomic)
                try envelope.write(to:home.appendingPathComponent("highwater.cnu"),options:.atomic)
                pending = p; message = "Update \(p.generation) downloaded and verified. Apply when ready."
                return message
            } catch { last = error }
        }
        throw last
    }
    func activate() throws {
        guard let p = pending ?? read("pending.cnu") else { throw NativeFailure.refused("No update is pending") }
        try p.validate(directory(p))
        if let old=current ?? read("current.cnu") { try old.envelope.write(to:home.appendingPathComponent("previous.cnu"),options:.atomic) }
        try p.envelope.write(to:home.appendingPathComponent("current.cnu"),options:.atomic)
        try? FileManager.default.removeItem(at:home.appendingPathComponent("pending.cnu"))
        pending = nil
    }
    func details() -> String {
        let build = Bundle.main.object(forInfoDictionaryKey:"CFBundleVersion") as? String ?? "?"
        return "OpenAgents 1.0.0 (\(build))\nRuntime \(runtime.prefix(12))\n\(current.map { "Signed update \($0.generation)" } ?? "Embedded starter")\n\(message)"
    }
}
