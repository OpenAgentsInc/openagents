// Firm reason: 6 (Bonjour, Network.framework and the tailscale CLI are Apple-platform effects; these are
// the few rules their two ends share, kept beside them and checked by tailnet-check.swift).
// Zero-config pairing between Coder Desktop and the phone over Tailscale (GitHub #13778), the
// NIP-HOST tailnet admission (nostr/nips/openagents/NIP-HOST.md#tailnet-admission):
//
//   1. The Mac advertises `_openagents-coder._tcp` on the local network. Its TXT record carries
//      its tailnet address, its MagicDNS name, the admission port and its host key.
//   2. The phone on the same Wi-Fi reads the record once and keeps it (AppTailnetBook). From then
//      on it reaches the Mac at that tailnet address, from anywhere its Tailscale is on.
//   3. The phone sends one admission request line. The Mac asks its own `tailscale whois` who is
//      calling and answers with a pairing link only for an untagged device of the Mac's own
//      Tailscale user (AppTailnet.decide). The phone opens the link the way a scanned one opens:
//      the ordinary relay pairing (AppDeviceLink.pair) gets the grant, which the host signs.
//
// Tailscale identifies the caller; it never grants anything by itself. Anyone else (another user,
// a tagged device, an address outside the tailnet) is refused and can still pair with a link.
// Foundation only: the rules here build and run on the Mac in apps/shared/host/apple/tailnet-check.sh.
import Foundation

enum AppTailnet {
    /// The Bonjour service type the Mac advertises and the phone browses (Info.plist NSBonjourServices).
    static let service = "_openagents-coder._tcp"
    /// The TXT record's version.
    static let version = "1"
    /// NIP-HOST's default admission port; a Mac that cannot have it advertises the one it got.
    static let port: UInt16 = 47109
    /// The request line's bytes at most (NIP-HOST), and the answer's.
    static let requestMax = 1024
    static let answerMax = 4096
    /// A pairing link the answer may carry starts with this (AppDeviceLink.pair reads it).
    static let linkPrefix = "openagents://pair?"
    /// The marker the Mac adds to the link, so the phone says it is pairing with its Mac rather
    /// than pointing at a code on the Mac's screen (apps/shared/app/device/lane.coax dl_pair).
    static let linkMark = "&auto=1"

    // ------------------------------------------------------------------ addresses

    /// Whether `ip` is in Tailscale's ranges: 100.64.0.0/10 or fd7a:115c:a1e0::/48.
    static func isTailnet(_ ip: String) -> Bool { isTailnetIPv4(ip) || isTailnetIPv6(ip) }

    /// The address with any IPv6 zone, brackets or IPv4-mapped prefix taken off.
    static func plain(_ ip: String) -> String {
        var s = ip.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("[") && s.hasSuffix("]") { s = String(s.dropFirst().dropLast()) }
        if let zone = s.firstIndex(of: "%") { s = String(s[..<zone]) }
        if s.lowercased().hasPrefix("::ffff:"), s.contains(".") { s = String(s.dropFirst(7)) }
        return s
    }

    /// Four decimal octets, each 0...255, in 100.64.0.0/10.
    static func isTailnetIPv4(_ ip: String) -> Bool {
        guard let o = octets(plain(ip)) else { return false }
        return o[0] == 100 && (o[1] & 0xC0) == 64
    }
    static func octets(_ s: String) -> [Int]? {
        let parts = s.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 4 else { return nil }
        var out: [Int] = []
        for p in parts {
            guard (1...3).contains(p.count), p.allSatisfy({ $0.isASCII && $0.isNumber }), let v = Int(p), v <= 255 else { return nil }
            out.append(v)
        }
        return out
    }
    static func isTailnetIPv6(_ ip: String) -> Bool {
        var a = in6_addr()
        guard inet_pton(AF_INET6, plain(ip), &a) == 1 else { return false }
        let b = withUnsafeBytes(of: &a) { Array($0) }
        return Array(b[0..<6]) == [0xfd, 0x7a, 0x11, 0x5c, 0xa1, 0xe0]
    }

    /// A MagicDNS name: lowercase letters, digits, hyphens and dots, without its final dot.
    static func dnsName(_ raw: String) -> String? {
        var s = raw.lowercased()
        if s.hasSuffix(".") { s.removeLast() }
        guard (1...253).contains(s.utf8.count), s.contains("."), !s.hasPrefix("."), !s.contains(".."),
              s.allSatisfy({ ($0.isASCII && ($0.isLetter || $0.isNumber)) || $0 == "-" || $0 == "." }) else { return nil }
        return s
    }
    /// A host key: 64 lowercase hex characters.
    static func isHostKey(_ s: String) -> Bool { s.count == 64 && s.allSatisfy { $0.isHexDigit && !$0.isUppercase } }

    // ------------------------------------------------------------------ admission

    /// Why the Mac refuses a caller (NIP-HOST's codes). nil admits.
    enum Refusal: String { case malformed, notTailnet = "not_tailnet", unavailable, tagged, notOwner = "not_owner" }

    /// The Tailscale user who owns this Mac, from `tailscale status --json`. nil when Tailscale is not
    /// running, the Mac has no tailnet address, or the Mac itself is a tagged device (it has no owner).
    struct Owner: Equatable { var user: Int64; var login: String; var ip: String; var dns: String; var name: String }
    static func owner(status: Data) -> Owner? {
        guard let s = try? JSONSerialization.jsonObject(with: status) as? [String: Any],
              s["BackendState"] as? String == "Running", let me = s["Self"] as? [String: Any],
              let user = (me["UserID"] as? NSNumber)?.int64Value, user != 0 else { return nil }
        if let tags = me["Tags"] as? [Any], !tags.isEmpty { return nil }
        let ips = (me["TailscaleIPs"] as? [String]) ?? (s["TailscaleIPs"] as? [String]) ?? []
        guard let ip = ips.first(where: isTailnetIPv4) else { return nil }
        let users = s["User"] as? [String: Any]
        let login = ((users?[String(user)] as? [String: Any])?["LoginName"] as? String) ?? ""
        let host = (me["HostName"] as? String) ?? ""
        return Owner(user: user, login: login, ip: ip, dns: dnsName((me["DNSName"] as? String) ?? "") ?? "", name: host)
    }

    /// Whether the Mac admits the device calling from `remote`, given what its own
    /// `tailscale whois --json REMOTE` printed (nil: it printed nothing usable) and its owner.
    static func decide(remote: String, whois: Data?, owner: Owner?) -> Refusal? {
        guard isTailnet(remote) else { return .notTailnet }
        guard let owner, let whois, let w = try? JSONSerialization.jsonObject(with: whois) as? [String: Any],
              let node = w["Node"] as? [String: Any], let profile = w["UserProfile"] as? [String: Any] else { return .unavailable }
        // whois must name the address that called, not some other device
        let addresses = ((node["Addresses"] as? [String]) ?? []).map { plain(String($0.split(separator: "/").first ?? "")) }
        guard addresses.contains(plain(remote)) else { return .unavailable }
        if let tags = node["Tags"] as? [Any], !tags.isEmpty { return .tagged }
        let nodeUser = (node["User"] as? NSNumber)?.int64Value
        let profileUser = (profile["ID"] as? NSNumber)?.int64Value
        guard profileUser == owner.user, nodeUser == nil || nodeUser == owner.user else { return .notOwner }
        let login = (profile["LoginName"] as? String) ?? ""
        if !owner.login.isEmpty, !login.isEmpty, login != owner.login { return .notOwner }
        return nil
    }

    // ------------------------------------------------------------------ the wire lines (Coax codec)

    /// The phone's admission request line (encoded by coax/lib/nostr/host/tailnet.coax).
    static func requestLine() -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_request_write(0, &buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }
    /// Whether `line` (without its newline) is an admission request this Mac serves.
    static func isRequest(_ line: Data) -> Bool {
        guard line.count <= requestMax else { return false }
        return line.withUnsafeBytes { raw in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_request_read(p, Int64(line.count)) >= 0
        }
    }

    struct Answer: Equatable { var host: String; var label: String; var invitation: String?; var refused: String? }
    /// The Mac's admission answer line: a pairing link, or why not.
    static func answerLine(_ a: Answer) -> Data {
        var buf = [UInt8](repeating: 0, count: answerMax)
        let n = oa_tailnet_answer_write(a.host, a.label, a.invitation ?? "", a.refused ?? "", &buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }
    /// The admission answer read on the phone, checked against the host key the TXT record named.
    static func answer(_ line: Data, host: String) -> Answer? {
        guard line.count <= answerMax else { return nil }
        var val = [UInt8](repeating: 0, count: answerMax)
        var lbl = [UInt8](repeating: 0, count: 128)
        var ln: Int64 = 0
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_answer_read(p, Int64(line.count), host, &val, Int64(val.count), &lbl, Int64(lbl.count), &ln)
        }
        guard rc != -1 else { return nil }
        let label = ln > 0 ? String(decoding: lbl[0..<Int(ln)], as: UTF8.self) : ""
        if rc <= -2 {
            let rn = Int(-rc - 2)
            let refused = String(decoding: val[0..<rn], as: UTF8.self)
            return Answer(host: host, label: label, invitation: nil, refused: refused)
        }
        let link = String(decoding: val[0..<Int(rc)], as: UTF8.self)
        guard link.hasPrefix(linkPrefix), link.utf8.count <= 1024,
              let parts = URLComponents(string: link), parts.queryItems?.first(where: { $0.name == "host" })?.value == host else { return nil }
        return Answer(host: host, label: label, invitation: link, refused: nil)
    }

    /// NIP-SESS direct transport handshake line.
    static func directHelloLine() -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_direct_hello_write(&buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }
    static func isDirectHello(_ line: Data) -> Bool {
        guard line.count <= requestMax else { return false }
        return line.withUnsafeBytes { raw in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_direct_hello_read(p, Int64(line.count)) != 0
        }
    }
    static func directAckLine(refused: String?) -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_direct_ack_write(refused ?? "", &buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }
    /// Parses a NIP-SESS direct ack line. Returns (ok: true, refused: nil) when admitted, (ok: false, refused: code) when refused, or (ok: false, refused: nil) when malformed.
    static func directAck(_ line: Data) -> (ok: Bool, refused: String?) {
        guard line.count <= requestMax else { return (false, nil) }
        var buf = [UInt8](repeating: 0, count: 64)
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_direct_ack_read(p, Int64(line.count), &buf, Int64(buf.count))
        }
        if rc == 0 { return (true, nil) }
        if rc > 0 { return (false, String(decoding: buf[0..<Int(rc)], as: UTF8.self)) }
        return (false, nil)
    }

    // ------------------------------------------------------------------ relay-free direct Tailscale grants (#13993)

    struct DirectGrantRequest: Equatable { var device: String; var label: String }
    struct DirectGrantAnswer: Equatable {
        var host: String
        var label: String
        var grant: String?
        var epoch: Int64
        var expires: Int64
        var refused: String?
    }

    static func directGrantRequestLine(device: String, label: String) -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_grant_request_write(device, label, &buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }

    static func parseDirectGrantRequest(_ line: Data) -> DirectGrantRequest? {
        guard line.count <= requestMax else { return nil }
        var dev = [UInt8](repeating: 0, count: 64)
        var lbl = [UInt8](repeating: 0, count: 128)
        var ln: Int64 = 0
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_grant_request_read(p, Int64(line.count), &dev, Int64(dev.count), &lbl, Int64(lbl.count), &ln)
        }
        guard rc == 64, ln > 0 else { return nil }
        return DirectGrantRequest(
            device: String(decoding: dev, as: UTF8.self),
            label: String(decoding: lbl[0..<Int(ln)], as: UTF8.self)
        )
    }

    static func directGrantAnswerLine(_ answer: DirectGrantAnswer) -> Data {
        var buf = [UInt8](repeating: 0, count: answerMax)
        let n = oa_tailnet_grant_answer_write(
            answer.host,
            answer.label,
            answer.grant ?? "",
            answer.epoch,
            answer.expires,
            answer.refused ?? "",
            &buf,
            Int64(buf.count)
        )
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }

    static func directGrantAnswer(_ line: Data) -> DirectGrantAnswer? {
        guard line.count <= answerMax else { return nil }
        var host = [UInt8](repeating: 0, count: 64)
        var lbl = [UInt8](repeating: 0, count: 128)
        var val = [UInt8](repeating: 0, count: 64)
        var meta: [Int64] = [0, 0, 0]
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_grant_answer_read(p, Int64(line.count), &host, Int64(host.count), &lbl, Int64(lbl.count), &val, Int64(val.count), &meta)
        }
        guard rc != -1 else { return nil }
        let label = meta[2] > 0 ? String(decoding: lbl[0..<Int(meta[2])], as: UTF8.self) : ""
        if rc == 64 {
            return DirectGrantAnswer(
                host: String(decoding: host, as: UTF8.self),
                label: label,
                grant: String(decoding: val, as: UTF8.self),
                epoch: meta[0],
                expires: meta[1],
                refused: nil
            )
        }
        if rc <= -2 {
            let rn = Int(-rc - 2)
            let refused = String(decoding: val[0..<rn], as: UTF8.self)
            return DirectGrantAnswer(host: "", label: label, grant: nil, epoch: 0, expires: 0, refused: refused)
        }
        return nil
    }

    // ------------------------------------------------------------------ real-time push frames (#13993)

    struct PushEvent: Equatable {
        var seq: Int64
        var kind: String
        var session: String
        var revision: Int64
        var busy: Bool
        var hasInteraction: Bool
    }

    static func subscribeLine(session: String = "") -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_subscribe_write(session, &buf, Int64(buf.count))
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }

    static func parseSubscribe(_ line: Data) -> String? {
        guard line.count <= requestMax else { return nil }
        var buf = [UInt8](repeating: 0, count: 128)
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_subscribe_read(p, Int64(line.count), &buf, Int64(buf.count))
        }
        guard rc >= 0 else { return nil }
        return String(decoding: buf[0..<Int(rc)], as: UTF8.self)
    }

    static func pushLine(_ ev: PushEvent) -> Data {
        var buf = [UInt8](repeating: 0, count: requestMax)
        let n = oa_tailnet_push_write(
            ev.seq,
            ev.kind,
            ev.session,
            ev.revision,
            ev.busy ? 1 : 0,
            ev.hasInteraction ? 1 : 0,
            &buf,
            Int64(buf.count)
        )
        guard n > 0 else { return Data() }
        var d = Data(buf[0..<Int(n)])
        d.append(0x0A)
        return d
    }

    static func parsePush(_ line: Data) -> PushEvent? {
        guard line.count <= requestMax else { return nil }
        var kind = [UInt8](repeating: 0, count: 32)
        var sess = [UInt8](repeating: 0, count: 128)
        var meta: [Int64] = [0, 0, 0, 0, 0]
        let rc = line.withUnsafeBytes { raw -> Int64 in
            let p = raw.baseAddress?.assumingMemoryBound(to: UInt8.self)
            return oa_tailnet_push_read(p, Int64(line.count), &kind, Int64(kind.count), &sess, Int64(sess.count), &meta)
        }
        guard rc > 0 else { return nil }
        return PushEvent(
            seq: meta[0],
            kind: String(decoding: kind[0..<Int(rc)], as: UTF8.self),
            session: meta[4] > 0 ? String(decoding: sess[0..<Int(meta[4])], as: UTF8.self) : "",
            revision: meta[1],
            busy: meta[2] != 0,
            hasInteraction: meta[3] != 0
        )
    }
}

/// One Mac the phone found: what its TXT record said, and when it was last seen or reached.
struct AppTailnetComputer: Codable, Equatable {
    var host: String; var ip: String; var dns: String; var port: UInt16; var name: String; var seen: Int64

    /// The TXT record's keys (issue #13778): v, ts_ip, ts_dns, port, host_id, name.
    var txt: [String: String] {
        ["v": AppTailnet.version, "ts_ip": ip, "ts_dns": dns, "port": String(port), "host_id": host, "name": name]
    }
    init(host: String, ip: String, dns: String, port: UInt16, name: String, seen: Int64 = 0) {
        self.host = host; self.ip = ip; self.dns = dns; self.port = port; self.name = name; self.seen = seen
    }
    /// A record the phone read; nil unless every field reads.
    init?(txt: [String: String], seen: Int64 = 0) {
        guard txt["v"] == AppTailnet.version, let host = txt["host_id"], AppTailnet.isHostKey(host),
              let ip = txt["ts_ip"], AppTailnet.isTailnetIPv4(ip), AppTailnet.octets(ip) != nil,
              let p = txt["port"], let port = UInt16(p), port > 0 else { return nil }
        let rawDNS = txt["ts_dns"] ?? ""
        let dns = rawDNS.isEmpty ? "" : (AppTailnet.dnsName(rawDNS) ?? "")
        guard rawDNS.isEmpty || !dns.isEmpty else { return nil }
        let name = AppTailnetComputer.name(txt["name"] ?? "")
        self.init(host: host, ip: ip, dns: dns, port: port, name: name.isEmpty ? "My Mac" : name, seen: seen)
    }
    /// A name to show: no control characters, at most 63 bytes (one DNS label), cut on a character.
    static func name(_ raw: String) -> String {
        var out = ""
        for ch in raw where !ch.unicodeScalars.contains(where: { $0.properties.generalCategory == .control || $0.properties.generalCategory == .format }) {
            if out.utf8.count + String(ch).utf8.count > 63 { break }
            out.append(ch)
        }
        return out.trimmingCharacters(in: .whitespaces)
    }
    /// Where the phone dials: the tailnet address, then the MagicDNS name — filtering empty strings (#14007).
    var dialOrder: [String] {
        var out: [String] = []
        let cleanIP = ip.trimmingCharacters(in: .whitespacesAndNewlines)
        let cleanDNS = dns.trimmingCharacters(in: .whitespacesAndNewlines)
        if !cleanIP.isEmpty { out.append(cleanIP) }
        if !cleanDNS.isEmpty && cleanDNS != cleanIP { out.append(cleanDNS) }
        return out
    }
}

/// The Macs the phone found, kept in a file of its own (not the Keychain: none of it is a secret;
/// the grant stays in AppDeviceLink's store). At most 32, newest first (#14007).
final class AppTailnetBook {
    static let limit = 32
    let url: URL
    init(url: URL) { self.url = url }
    var computers: [AppTailnetComputer] {
        get { (try? Data(contentsOf: url)).flatMap { try? JSONDecoder().decode([AppTailnetComputer].self, from: $0) } ?? [] }
        set {
            guard let data = try? JSONEncoder().encode(Array(newValue.prefix(AppTailnetBook.limit))) else { return }
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? data.write(to: url, options: .atomic)
        }
    }
    /// A record seen again replaces the old one for that host (its address may have changed).
    @discardableResult func remember(_ c: AppTailnetComputer) -> [AppTailnetComputer] {
        var list = computers.filter { $0.host != c.host }
        list.insert(c, at: 0)
        list.sort { $0.seen > $1.seen }
        computers = list
        return computers
    }
    func forget(_ host: String) { computers = computers.filter { $0.host != host } }

    /// Whether the phone should ask Mac `host` to admit it now: it holds no grant for it, or the
    /// grant ends within a day, and it has not asked in the last minute.
    static func wantsAdmission(expires: Int64?, lastAsked: Int64?, now: Int64) -> Bool {
        if let lastAsked, now - lastAsked < 60 { return false }
        guard let expires else { return true }
        return expires - now < 86400
    }
}
