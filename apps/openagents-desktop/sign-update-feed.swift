// Firm reason: 6 (Swift-only CryptoKit Ed25519 signing matching Updater.swift's UpdateVerifier).
// Emits a signed desktop update envelope (openagents-desktop-update.json) via coax/lib/agent/desktop/update.coax (#13785).
// Usage:
//   swift apps/openagents-desktop/sign-update-feed.swift \
//     --version 0.2.0 --build 2 --url https://... --archive OpenAgents.zip \
//     --key-id openagents-desktop-2026-v1 --seed-hex <64-hex-chars> [--notes "..."] [--out openagents-desktop-update.json]
import CryptoKit
import Foundation

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write(Data(("sign-update-feed: " + msg + "\n").utf8))
    exit(1)
}

func hexToData(_ hex: String) -> Data? {
    let clean = hex.trimmingCharacters(in: .whitespacesAndNewlines)
    guard clean.count % 2 == 0 else { return nil }
    var out = Data(capacity: clean.count / 2)
    var i = clean.startIndex
    while i < clean.endIndex {
        let next = clean.index(i, offsetBy: 2)
        guard let byte = UInt8(clean[i..<next], radix: 16) else { return nil }
        out.append(byte)
        i = next
    }
    return out
}

func runCodec(_ root: URL, _ args: [String]) {
    let proc = Process()
    proc.executableURL = root.appendingPathComponent("coax/web/tool.sh")
    proc.arguments = [root.appendingPathComponent("apps/openagents-desktop/update-feed.coax").path] + args
    proc.currentDirectoryURL = root
    let errPipe = Pipe()
    proc.standardError = errPipe
    do { try proc.run() } catch {
        fail("failed to run update-feed.coax: \(error.localizedDescription)")
    }
    let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
    proc.waitUntilExit()
    if proc.terminationStatus != 0 {
        let errStr = String(data: errData, encoding: .utf8) ?? ""
        fail("update-feed.coax exited \(proc.terminationStatus): \(errStr.trimmingCharacters(in: .whitespacesAndNewlines))")
    }
}

var version: String?
var buildNum: String?
var urlStr: String?
var archivePath: String?
var keyID = "openagents-desktop-2026-v1"
var bundleID = "com.openagents.desktop"
var teamID = "HQWSG26L43"
var seedHex = ProcessInfo.processInfo.environment["OA_UPDATE_SIGN_SEED_HEX"]
var notes: String?
var outPath: String?

var args = Array(CommandLine.arguments.dropFirst())
var idx = 0
while idx < args.count {
    let flag = args[idx]
    guard idx + 1 < args.count else { fail("missing value for \(flag)") }
    let val = args[idx + 1]
    idx += 2
    switch flag {
    case "--version": version = val
    case "--build": buildNum = val
    case "--url": urlStr = val
    case "--archive": archivePath = val
    case "--key-id": keyID = val
    case "--bundle-id": bundleID = val
    case "--team-id": teamID = val
    case "--seed-hex": seedHex = val
    case "--notes": notes = val
    case "--out": outPath = val
    default: fail("unknown option: \(flag)")
    }
}

guard let version, !version.isEmpty else { fail("--version is required") }
guard let urlStr, URL(string: urlStr)?.scheme == "https" else {
    fail("--url must be an https:// URL")
}
guard let archivePath, FileManager.default.fileExists(atPath: archivePath) else {
    fail("--archive file not found")
}
guard let seedHex, let seed = hexToData(seedHex), seed.count == 32 else {
    fail("32-byte Ed25519 seed hex required via --seed-hex or OA_UPDATE_SIGN_SEED_HEX")
}

let scriptURL = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
let repoRoot = scriptURL.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
let absArchive = URL(fileURLWithPath: archivePath).standardizedFileURL.path

let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("oa-update-feed-\(UUID().uuidString)")
try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
defer { try? FileManager.default.removeItem(at: tmpDir) }

let tmpPayload = tmpDir.appendingPathComponent("payload.json")
let tmpSig = tmpDir.appendingPathComponent("sig.bin")
let tmpEnv = tmpDir.appendingPathComponent("envelope.json")

var manifestArgs = [
    "manifest",
    "--version", version,
    "--bundle-id", bundleID,
    "--team-id", teamID,
    "--url", urlStr,
    "--archive", absArchive,
    "--out", tmpPayload.path,
]
if let buildNum { manifestArgs += ["--build", buildNum] }
if let notes { manifestArgs += ["--notes", notes] }

runCodec(repoRoot, manifestArgs)
let payloadBytes = try Data(contentsOf: tmpPayload)
let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
let sigBytes = try privateKey.signature(for: payloadBytes)
try sigBytes.write(to: tmpSig, options: .atomic)

runCodec(repoRoot, [
    "envelope",
    "--key-id", keyID,
    "--payload-file", tmpPayload.path,
    "--sig-file", tmpSig.path,
    "--out", tmpEnv.path,
])

let json = try Data(contentsOf: tmpEnv)
if let outPath {
    try json.write(to: URL(fileURLWithPath: outPath), options: .atomic)
} else {
    FileHandle.standardOutput.write(json)
}
