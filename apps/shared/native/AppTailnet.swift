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
    static let requestSchema = "openagents.host-tailnet-admission-request.v1"
    static let answerSchema = "openagents.host-tailnet-admission.v1"
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

    // ------------------------------------------------------------------ the two lines

    /// The phone's request line.
    static func requestLine() -> Data { Data("{\"v\":\"\(requestSchema)\",\"requires\":[],\"chats\":false}\n".utf8) }
    /// Whether `line` (without its newline) is a request this Mac serves.
    static func isRequest(_ line: Data) -> Bool {
        guard line.count <= requestMax, let r = try? JSONSerialization.jsonObject(with: line) as? [String: Any],
              r["v"] as? String == requestSchema, let requires = r["requires"] as? [Any], requires.isEmpty else { return false }
        let known: Set<String> = ["v", "requires", "chats"]
        return Set(r.keys).isSubset(of: known) && (r["chats"] == nil || r["chats"] is Bool)
    }

    struct Answer: Equatable { var host: String; var label: String; var invitation: String?; var refused: String? }
    /// The Mac's answer line: a pairing link, or why not.
    static func answerLine(_ a: Answer) -> Data {
        var o: [String: Any] = ["v": answerSchema, "host": a.host, "label": a.label, "chats": NSNull()]
        o["invitation"] = a.invitation ?? NSNull(); o["refused"] = a.refused ?? NSNull()
        var d = (try? JSONSerialization.data(withJSONObject: o, options: [.sortedKeys, .withoutEscapingSlashes])) ?? Data()
        d.append(0x0A)
        return d
    }
    /// The answer read on the phone, checked against the host key the TXT record named: an
    /// invitation must be a pairing link to that same host.
    static func answer(_ line: Data, host: String) -> Answer? {
        guard line.count <= answerMax, let o = try? JSONSerialization.jsonObject(with: line) as? [String: Any],
              o["v"] as? String == answerSchema, let h = o["host"] as? String, h == host else { return nil }
        let label = String(((o["label"] as? String) ?? "").prefix(80))
        if let refused = o["refused"] as? String { return Answer(host: h, label: label, invitation: nil, refused: refused) }
        guard let link = o["invitation"] as? String, link.hasPrefix(linkPrefix), link.utf8.count <= 1024,
              let parts = URLComponents(string: link), parts.queryItems?.first(where: { $0.name == "host" })?.value == host else { return nil }
        return Answer(host: h, label: label, invitation: link, refused: nil)
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
    /// Where the phone dials: the tailnet address, then the MagicDNS name (the address can change).
    var dialOrder: [String] { dns.isEmpty || dns == ip ? [ip] : [ip, dns] }
}

/// The Macs the phone found, kept in a file of its own (not the Keychain: none of it is a secret;
/// the grant stays in AppDeviceLink's store). At most eight, newest first.
final class AppTailnetBook {
    static let limit = 8
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
