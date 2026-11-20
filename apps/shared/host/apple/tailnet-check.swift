import Foundation
import Network

// The Mac's check of zero-config pairing over Tailscale (GitHub #13778; tailnet-check.sh builds it
// with apps/shared/native/AppTailnet.swift and AppTailnetLink.swift and runs it):
//   - the TXT record, written and read back, and every record the phone must not keep;
//   - the Mac's admission decision on `tailscale whois` output: the owner's untagged device is
//     admitted; a tagged device, another user, an address outside the tailnet and an unreadable
//     answer are not;
//   - the two lines, and the phone's saved Macs and when it asks one again;
//   - the phone's link against a stand-in Mac on this machine's loopback: a saved Mac is reached
//     with no discovery, its name is tried when its address fails, a network change asks again only
//     when the grant nears its end, and a refusal opens nothing.
// It touches no Keychain, starts no Tailscale and browses nothing.

var failures = 0
func expect(_ ok: Bool, _ what: String, line: Int = #line) {
    if !ok { failures += 1; FileHandle.standardError.write(Data("tailnet-check: line \(line): \(what)\n".utf8)) }
}
func json(_ o: Any) -> Data { try! JSONSerialization.data(withJSONObject: o) }

let host = String(repeating: "ab", count: 32)
let other = String(repeating: "cd", count: 32)

// ---------------------------------------------------------------- addresses
expect(AppTailnet.isTailnet("100.64.0.1") && AppTailnet.isTailnet("100.127.255.254") && AppTailnet.isTailnet("100.100.2.70"), "100.64.0.0/10 is the tailnet")
expect(!AppTailnet.isTailnet("100.63.255.255") && !AppTailnet.isTailnet("100.128.0.1") && !AppTailnet.isTailnet("10.0.0.5") && !AppTailnet.isTailnet("192.168.1.122"), "outside 100.64.0.0/10 is not")
expect(!AppTailnet.isTailnet("100.64.0") && !AppTailnet.isTailnet("100.64.0.256") && !AppTailnet.isTailnet("100.64.0.1.2") && !AppTailnet.isTailnet(""), "a malformed address is not")
expect(AppTailnet.isTailnet("fd7a:115c:a1e0::4837:6b1f") && AppTailnet.isTailnet("fd7a:115c:a1e0::1%utun4") && AppTailnet.isTailnet("[fd7a:115c:a1e0::2]"), "fd7a:115c:a1e0::/48 is the tailnet")
expect(!AppTailnet.isTailnet("fd7a:115c:a1e1::1") && !AppTailnet.isTailnet("fe80::1%en0") && !AppTailnet.isTailnet("::1"), "other IPv6 is not")
expect(AppTailnet.isTailnet("::ffff:100.80.1.2") && !AppTailnet.isTailnet("::ffff:192.168.1.2"), "an IPv4-mapped address is read as IPv4")
expect(AppTailnet.dnsName("Macbook-Pro-M5.tailaeab8f.ts.net.") == "macbook-pro-m5.tailaeab8f.ts.net", "a MagicDNS name loses its final dot")
expect(AppTailnet.dnsName("mac") == nil && AppTailnet.dnsName("a..b") == nil && AppTailnet.dnsName("bad name.ts.net") == nil, "a malformed name is refused")

// ---------------------------------------------------------------- the TXT record
let mac = AppTailnetComputer(host: host, ip: "100.127.107.31", dns: "macbook-pro-m5.tailaeab8f.ts.net", port: 47109, name: "Chris’s MacBook Pro")
let txt = mac.txt
expect(txt == ["v": "1", "ts_ip": "100.127.107.31", "ts_dns": "macbook-pro-m5.tailaeab8f.ts.net", "port": "47109", "host_id": host, "name": "Chris’s MacBook Pro"], "the record's keys")
expect(AppTailnetComputer(txt: txt, seen: 5) == AppTailnetComputer(host: host, ip: mac.ip, dns: mac.dns, port: 47109, name: mac.name, seen: 5), "a record reads back")
func without(_ k: String) -> [String: String] { var t = txt; t[k] = nil; return t }
func with(_ k: String, _ v: String) -> [String: String] { var t = txt; t[k] = v; return t }
expect(AppTailnetComputer(txt: without("ts_dns"))?.dns == "", "a record without a name reads")
expect(AppTailnetComputer(txt: without("name"))?.name == "My Mac", "a record without a name shows My Mac")
for bad in [with("v", "2"), without("v"), without("host_id"), with("host_id", String(repeating: "AB", count: 32)), with("host_id", "abcd"),
            without("ts_ip"), with("ts_ip", "192.168.1.122"), with("ts_ip", "100.128.0.1"), with("ts_ip", "fd7a:115c:a1e0::1"),
            without("port"), with("port", "0"), with("port", "70000"), with("port", "x"), with("ts_dns", "bad name")] {
    expect(AppTailnetComputer(txt: bad) == nil, "a bad record is refused: \(bad)")
}
let long = AppTailnetComputer(txt: with("name", String(repeating: "é", count: 40) + "\u{7}"))!
expect(long.name.utf8.count <= 63 && long.name.allSatisfy { $0 == "é" }, "a name is one DNS label at most, cut on a character, with no control characters")
expect(mac.dialOrder == [mac.ip, mac.dns] && AppTailnetComputer(txt: without("ts_dns"))!.dialOrder == [mac.ip], "the phone dials the address, then the name")

// ---------------------------------------------------------------- the owner and admission
let status = json(["BackendState": "Running", "TailscaleIPs": ["100.127.107.31", "fd7a:115c:a1e0::4837:6b1f"],
                   "Self": ["UserID": 59352136409861, "HostName": "Chris’s MacBook Pro", "DNSName": "macbook-pro-m5.tailaeab8f.ts.net.",
                            "TailscaleIPs": ["100.127.107.31", "fd7a:115c:a1e0::4837:6b1f"]] as [String: Any],
                   "User": ["59352136409861": ["ID": 59352136409861, "LoginName": "owner@example.com"]]] as [String: Any])
let owner = AppTailnet.owner(status: status)
expect(owner == AppTailnet.Owner(user: 59352136409861, login: "owner@example.com", ip: "100.127.107.31", dns: "macbook-pro-m5.tailaeab8f.ts.net", name: "Chris’s MacBook Pro"), "the owner from tailscale status")
var stopped = try! JSONSerialization.jsonObject(with: status) as! [String: Any]; stopped["BackendState"] = "Stopped"
expect(AppTailnet.owner(status: json(stopped)) == nil, "a stopped Tailscale has no owner")
var taggedMac = try! JSONSerialization.jsonObject(with: status) as! [String: Any]
var me = taggedMac["Self"] as! [String: Any]; me["Tags"] = ["tag:server"]; taggedMac["Self"] = me
expect(AppTailnet.owner(status: json(taggedMac)) == nil, "a tagged Mac has no owner to match")
expect(AppTailnet.owner(status: Data("not json".utf8)) == nil, "unreadable status has no owner")

func whois(ip: String, user: Int64 = 59352136409861, login: String = "owner@example.com", tags: [String]? = nil) -> Data {
    var node: [String: Any] = ["ID": 1, "Name": "iphone.tailaeab8f.ts.net.", "User": user, "Addresses": ["\(ip)/32", "fd7a:115c:a1e0::9/128"]]
    if let tags { node["Tags"] = tags }
    return json(["Node": node, "UserProfile": ["ID": user, "LoginName": login]] as [String: Any])
}
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.2.70"), owner: owner) == nil, "the owner's untagged phone is admitted")
expect(AppTailnet.decide(remote: "fd7a:115c:a1e0::9%utun4", whois: whois(ip: "100.100.2.70"), owner: owner) == nil, "over IPv6 too")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.2.70", tags: ["tag:ci"]), owner: owner) == .tagged, "a tagged device is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.2.70", user: 7, login: "else@example.com"), owner: owner) == .notOwner, "another user's device is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.2.70", login: "else@example.com"), owner: owner) == .notOwner, "a different login is refused")
expect(AppTailnet.decide(remote: "192.168.1.40", whois: whois(ip: "192.168.1.40"), owner: owner) == .notTailnet, "a LAN address is refused")
expect(AppTailnet.decide(remote: "10.0.0.7", whois: whois(ip: "10.0.0.7"), owner: owner) == .notTailnet, "a 10/8 address is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: nil, owner: owner) == .unavailable, "no whois answer is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: Data("{}".utf8), owner: owner) == .unavailable, "an unreadable whois answer is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.9.9"), owner: owner) == .unavailable, "whois naming another address is refused")
expect(AppTailnet.decide(remote: "100.100.2.70", whois: whois(ip: "100.100.2.70"), owner: nil) == .unavailable, "a Mac with no owner admits no one")

// ---------------------------------------------------------------- the two lines
let request = AppTailnet.requestLine()
expect(request.last == 0x0A && AppTailnet.isRequest(request.dropLast()), "the phone's request is one line the Mac serves")
expect(!AppTailnet.isRequest(json(["v": AppTailnet.requestSchema, "requires": ["x"]])) && !AppTailnet.isRequest(json(["v": "other", "requires": []])), "a request naming a feature or another version is refused")
expect(!AppTailnet.isRequest(json(["v": AppTailnet.requestSchema, "requires": [], "extra": 1])) && !AppTailnet.isRequest(Data(repeating: 0x20, count: 2000)), "an unknown field or a long line is refused")
let link = "openagents://pair?host=\(host)&relay=wss%3A%2F%2Frelay.openagents.com&code=ABCDEFGH&name=Mac" + AppTailnet.linkMark
let invited = AppTailnet.answerLine(.init(host: host, label: "Mac", invitation: link, refused: nil))
expect(invited.last == 0x0A && AppTailnet.answer(invited.dropLast(), host: host) == .init(host: host, label: "Mac", invitation: link, refused: nil), "an invitation reads back")
expect(AppTailnet.answer(invited.dropLast(), host: other) == nil, "an answer from another host is refused")
let wrong = AppTailnet.answerLine(.init(host: host, label: "Mac", invitation: "openagents://pair?host=\(other)&code=X", refused: nil))
expect(AppTailnet.answer(wrong.dropLast(), host: host) == nil, "an invitation to another host is refused")
let refusedLine = AppTailnet.answerLine(.init(host: host, label: "Mac", invitation: nil, refused: "not_owner"))
expect(AppTailnet.answer(refusedLine.dropLast(), host: host)?.refused == "not_owner", "a refusal reads back")

// ---------------------------------------------------------------- the saved Macs
let work = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : NSTemporaryDirectory()).appendingPathComponent("tailnet-\(getpid())")
try? FileManager.default.removeItem(at: work)
let file = work.appendingPathComponent("tailnet-computers-v1.json")
let book = AppTailnetBook(url: file)
expect(book.computers.isEmpty, "a new phone has no saved Macs")
book.remember(AppTailnetComputer(host: host, ip: "100.127.107.31", dns: mac.dns, port: 47109, name: "Mac", seen: 10))
book.remember(AppTailnetComputer(host: other, ip: "100.70.0.2", dns: "", port: 47109, name: "Other", seen: 20))
book.remember(AppTailnetComputer(host: host, ip: "100.127.107.99", dns: mac.dns, port: 47110, name: "Mac", seen: 30))
let reopened = AppTailnetBook(url: file).computers
expect(reopened.map(\.host) == [host, other] && reopened[0].ip == "100.127.107.99" && reopened[0].port == 47110, "saved Macs survive a relaunch; a Mac seen again replaces its old address")
for i in 0..<12 { book.remember(AppTailnetComputer(host: String(format: "%064x", i), ip: "100.64.0.\(i + 1)", dns: "", port: 1, name: "n", seen: Int64(100 + i))) }
expect(book.computers.count == AppTailnetBook.limit && book.computers.first?.seen == 111, "at most eight, newest first")
book.forget(String(format: "%064x", 11))
expect(!book.computers.contains { $0.host == String(format: "%064x", 11) }, "a Mac can be forgotten")
expect(AppTailnetBook.wantsAdmission(expires: nil, lastAsked: nil, now: 1000), "no grant: ask")
expect(!AppTailnetBook.wantsAdmission(expires: nil, lastAsked: 990, now: 1000), "asked a moment ago: wait")
expect(!AppTailnetBook.wantsAdmission(expires: 1000 + 7 * 86400, lastAsked: nil, now: 1000), "a grant for days: no need")
expect(AppTailnetBook.wantsAdmission(expires: 1000 + 3600, lastAsked: 100, now: 1000), "a grant ending within a day: ask again")

// ---------------------------------------------------------------- the phone's link against a stand-in Mac
// A stand-in Mac on 127.0.0.1 answers each request with `reply`. The saved record's address is one
// no one listens on, so the phone must fall back to its name (here the loopback address).
@MainActor final class StandIn {
    var requests = 0
    var reply: AppTailnet.Answer
    let listener: NWListener
    init(reply: AppTailnet.Answer) throws {
        self.reply = reply
        let params = NWParameters.tcp
        params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: params)
        listener.newConnectionHandler = { [weak self] conn in
            conn.start(queue: .main)
            conn.receive(minimumIncompleteLength: 1, maximumLength: 2048) { data, _, _, _ in
                MainActor.assumeIsolated {
                    guard let self, let data, let nl = data.firstIndex(of: 0x0A), AppTailnet.isRequest(data[data.startIndex..<nl]) else { conn.cancel(); return }
                    self.requests += 1
                    conn.send(content: AppTailnet.answerLine(self.reply), completion: .contentProcessed { _ in conn.cancel() })
                }
            }
        }
        listener.start(queue: .main)
    }
    var port: UInt16? { listener.port?.rawValue }
}
@MainActor func spin(_ seconds: TimeInterval, until: () -> Bool) -> Bool {
    let end = Date().addingTimeInterval(seconds)
    while Date() < end { if until() { return true }; RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    return until()
}

@MainActor func linkChecks() {
    guard let mac = try? StandIn(reply: .init(host: host, label: "Mac", invitation: link, refused: nil)),
          spin(5, until: { (mac.port ?? 0) != 0 }), let port = mac.port else { expect(false, "the stand-in Mac listens"); return }
    // what discovery saved earlier, read by a phone just launched: no browsing in this check
    let saved = work.appendingPathComponent("link.json")
    AppTailnetBook(url: saved).remember(AppTailnetComputer(host: host, ip: "127.0.0.2", dns: "127.0.0.1", port: port, name: "Mac", seen: 1))
    var opened: [String] = []
    var expires: Int64? = nil
    let phone = AppTailnetLink(book: AppTailnetBook(url: saved), expires: { _ in expires }, open: { opened.append($0) })
    phone.browses = false
    phone.resume()
    expect(spin(15, until: { opened.count == 1 }) && opened.first == link && mac.requests == 1, "a saved Mac is reached with no discovery, by name when its address fails")
    // paired now: a network change (Wi-Fi to cellular) needs nothing
    let now = Int64(Date().timeIntervalSince1970)
    expires = now + 7 * 86400
    phone.networkChanged()
    _ = spin(1, until: { mac.requests > 1 })
    expect(mac.requests == 1 && opened.count == 1, "a network change with a long grant asks nothing")
    // the grant nears its end: the next network change asks the saved Mac again
    expires = now + 600
    phone.networkChanged()
    expect(spin(15, until: { opened.count == 2 }) && mac.requests == 2, "a network change re-pairs from the saved address when the grant nears its end")
    // a Mac that refuses opens nothing
    mac.reply = .init(host: host, label: "Mac", invitation: nil, refused: "not_owner")
    expires = nil
    phone.networkChanged()
    expect(spin(15, until: { mac.requests == 3 }), "the refusing Mac is asked")
    _ = spin(0.5, until: { false })
    expect(opened.count == 2, "a refusal opens no link")
    phone.suspend()
    expires = nil
    phone.networkChanged()
    _ = spin(1, until: { mac.requests > 3 })
    expect(mac.requests == 3, "a phone in the background asks nothing")
    mac.listener.cancel()
}

MainActor.assumeIsolated { linkChecks() }
try? FileManager.default.removeItem(at: work)
if failures > 0 { FileHandle.standardError.write(Data("tailnet-check: \(failures) failed\n".utf8)); exit(1) }
print("tailnet-check: ok")
