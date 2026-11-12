import CryptoKit
import Foundation

// The Mac app's account login (Account.swift on the iPhone app's AppAuth.swift; #13229), built and run
// by account-check.sh:
//   account-check offline DIR          the key file, the token file, the requests a login makes (a fake server)
//   account-check live DIR ORIGIN      a key login against a real API (staging), who is logged in, a refresh
//                                      with the device key's proof, a reused refresh token ending the login
// Everything is kept in DIR, never in ~/.openagents, and nothing touches the Keychain.

func fail(_ words: String) -> Never { FileHandle.standardError.write(Data(("account-check: " + words + "\n").utf8)); exit(1) }
func mode(_ url: URL) -> Int { ((try? FileManager.default.attributesOfItem(atPath: url.path))?[.posixPermissions] as? NSNumber)?.intValue ?? -1 }
func hex(_ d: Data) -> String { DeviceKeys.hex(d) }

/// A fake API: answers by path, first in first out; keeps what it was asked.
final class Fake: @unchecked Sendable {
    let lock = NSLock()
    var asked: [URLRequest] = []
    var answers: [String: [(Int, String)]] = [:]
    func send(_ r: URLRequest) async throws -> (Data, Int) {
        lock.lock(); defer { lock.unlock() }
        asked.append(r)
        let path = r.url?.path ?? ""
        var q = answers[path] ?? []
        guard !q.isEmpty else { return (Data("{\"error\":\"Not found.\"}".utf8), 404) }
        let a = q.removeFirst(); answers[path] = q
        return (Data(a.1.utf8), a.0)
    }
}

func account(_ dir: URL, origin: String, send: @escaping AppAuth.Send = AppAuth.network) -> MacAccount {
    MacAccount(keyFile: dir.appendingPathComponent("cli/keys/default"),
               store: FileTokenStore(url: dir.appendingPathComponent("desktop/login.json")),
               device: DeviceKeys(store: FileDeviceKeyStore(url: dir.appendingPathComponent("desktop/device-key"))),
               origin: origin, send: send)
}

func offline(_ dir: URL) async {
    // The key file: the CLI's format, 0600 in a 0700 folder, never made over one that is there.
    let keyFile = dir.appendingPathComponent("cli/keys/default")
    guard var key = try? MacKey.make(keyFile) else { fail("no key made") }
    guard mode(keyFile) == 0o600, mode(keyFile.deletingLastPathComponent()) == 0o700 else { fail("the key file isn't 0600 in a 0700 folder") }
    let text = (try? Data(contentsOf: keyFile)) ?? Data()
    guard text.count == 81, String(decoding: text.prefix(16), as: UTF8.self) == "nostr-secret-v1 ", text.last == 10 else { fail("the key file isn't the CLI's format") }
    guard MacKey.load(keyFile) == key else { fail("the key didn't read back") }
    if (try? MacKey.make(keyFile)) != nil { fail("a key was made over the one there") }
    guard MacKey.load(keyFile) == key else { fail("the key changed") }
    MacKey.wipe(&key)

    // A NIP-98 event for a POST: kind 27235, the URL, the method and the body's hash, its id over its fields.
    let url = "https://example.test/api/auth/nostr", body = "{\"challenge\":\"c1\",\"label\":\"app\"}"
    guard let header = try? MacKey.signer(keyFile)(url, body, Date(timeIntervalSince1970: 1_800_000_000)), header.hasPrefix("Nostr "),
          let raw = Data(base64Encoded: String(header.dropFirst(6))),
          let e = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] else { fail("no NIP-98 header") }
    let tags = e["tags"] as? [[String]] ?? []
    let payload = hex(Data(SHA256.hash(data: Data(body.utf8))))
    guard (e["kind"] as? Int) == 27235, (e["created_at"] as? Int) == 1_800_000_000,
          tags == [["u", url], ["method", "POST"], ["payload", payload]] else { fail("the event's fields: \(e)") }
    let pubkey = e["pubkey"] as? String ?? ""
    let fields: [Any] = [0, pubkey, 1_800_000_000, 27235, tags, ""]
    guard let serial = try? JSONSerialization.data(withJSONObject: fields, options: [.withoutEscapingSlashes]),
          hex(Data(SHA256.hash(data: serial))) == (e["id"] as? String), (e["sig"] as? String)?.count == 128 else { fail("the event's id isn't over its fields") }

    // A key login: the challenge, then /api/auth/nostr signed over its body, with the device key's headers.
    let fake = Fake()
    fake.answers["/api/auth/nostr/challenge"] = [(200, "{\"challenge\":\"c1\"}")]
    fake.answers["/api/auth/nostr"] = [(200, "{\"account\":\"acct_a\",\"access_token\":\"oa_at_A1\",\"refresh_token\":\"oa_rt_R1\",\"expires_in\":3600,\"device_bound\":true}")]
    fake.answers["/api/auth/account"] = [(200, "{\"logged_in\":true,\"account\":\"acct_a\",\"emails\":[],\"keys\":[{}]}"),
                                         (200, "{\"logged_in\":true,\"account\":\"acct_a\",\"emails\":[\"a•••@example.com\"],\"keys\":[]}")]
    let me = account(dir, origin: "https://example.test", send: { try await fake.send($0) })
    guard me.title == "Log In…", !me.loggedIn else { fail("logged in before logging in") }
    do { try await me.keyLogin() } catch { fail("key login: \(error)") }
    let login = fake.asked.first { $0.url?.path == "/api/auth/nostr" }
    guard let login, String(decoding: login.httpBody ?? Data(), as: UTF8.self) == body,
          login.value(forHTTPHeaderField: "OA-Device-Platform") == "mac", login.value(forHTTPHeaderField: "OA-Device-Name")?.hasPrefix("Mac") == true,
          login.value(forHTTPHeaderField: "OA-Device-Key")?.count == 130, login.value(forHTTPHeaderField: "Authorization")?.hasPrefix("Nostr ") == true
    else { fail("the login request: \(String(describing: login?.allHTTPHeaderFields))") }
    guard me.title == "Logged in with this Mac's key" else { fail("the title: \(me.title)") }
    let account = fake.asked.last { $0.url?.path == "/api/auth/account" }
    guard account?.value(forHTTPHeaderField: "Authorization") == "Bearer oa_at_A1" else { fail("who is logged in wasn't asked with the access token") }
    await me.look()
    guard me.title == "Logged in as a•••@example.com" else { fail("the title: \(me.title)") }

    // The tokens: 0600, read back, and gone at logout with the device key.
    let tokens = dir.appendingPathComponent("desktop/login.json")
    guard mode(tokens) == 0o600, mode(tokens.deletingLastPathComponent()) == 0o700, me.store.load()?.deviceBound == true else { fail("the token file isn't 0600, or lost the binding") }
    fake.answers["/api/auth/logout"] = [(200, "{}")]
    await me.logout()
    guard !FileManager.default.fileExists(atPath: tokens.path), !FileManager.default.fileExists(atPath: dir.appendingPathComponent("desktop/device-key").path),
          me.title == "Log In…" else { fail("logout left the tokens or the device key") }

    // An email code: the code goes with the pending value; a wrong one is the server's own words.
    fake.answers["/api/auth/email/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"p1\"}")]
    fake.answers["/api/auth/email/verify"] = [(401, "{\"error\":\"That code isn't right. 4 tries left.\"}"),
                                              (200, "{\"account\":\"acct_b\",\"access_token\":\"oa_at_B1\",\"refresh_token\":\"oa_rt_B1\",\"expires_in\":3600,\"device_bound\":true}")]
    fake.answers["/api/auth/account"] = [(200, "{\"logged_in\":true,\"account\":\"acct_b\",\"emails\":[\"a•••@example.com\"],\"keys\":[]}")]
    guard let pending = try? await me.auth.emailStart("ada@example.com"), pending == "p1" else { fail("email start") }
    do { try await me.auth.emailVerify(code: "000000", pending: pending); fail("a wrong code logged in") }
    catch { guard error.localizedDescription == "That code isn't right. 4 tries left." else { fail("the wrong code's words: \(error.localizedDescription)") } }
    do { try await me.auth.emailVerify(code: "482913", pending: pending) } catch { fail("email verify: \(error)") }
    await me.look()
    let verify = fake.asked.last { $0.url?.path == "/api/auth/email/verify" }
    guard me.title == "Logged in as a•••@example.com", String(decoding: verify?.httpBody ?? Data(), as: UTF8.self) == "{\"code\":\"482913\",\"pending\":\"p1\"}",
          verify?.value(forHTTPHeaderField: "OA-Device-Platform") == "mac" else { fail("the email login") }
    fake.answers["/api/auth/logout"] = [(200, "{}")]
    await me.logout()
    print("account-check: offline ok (key file, NIP-98, key login, email code, title, tokens 0600, logout)")
}

func live(_ dir: URL, origin: String) async {
    let me = account(dir, origin: origin)
    if MacKey.load(me.keyFile) == nil { guard var k = try? MacKey.make(me.keyFile) else { fail("no key made") }; MacKey.wipe(&k) }
    do { try await me.keyLogin() } catch { fail("key login against \(origin): \(error.localizedDescription)") }
    guard let first = me.store.load(), first.deviceBound == true else { fail("the login isn't bound to the device key") }
    guard let who = try? await me.auth.account() else { fail("who is logged in") }
    print("account-check: logged in to \(origin) as \(who.account) (device bound), menu: \(me.title)")

    // A stale access token: the next request refreshes first, with the device key's proof.
    var stale = first; stale.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(stale)
    guard let again = try? await me.auth.account(), again.account == who.account, let second = me.store.load(), second.refresh != first.refresh else { fail("the refresh") }
    print("account-check: refreshed with the device key's proof")

    // The first refresh token again: the server ends the whole login, and the app forgets it.
    var reused = second; reused.refresh = first.refresh; reused.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(reused)
    do { _ = try await me.auth.account(); fail("a reused refresh token still worked") } catch {}
    guard !me.loggedIn else { fail("the app kept a login the server ended") }
    var ended = second; ended.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(ended)
    do { _ = try await me.auth.account(); fail("the newer refresh token still worked after the reuse") } catch {}
    print("account-check: a reused refresh token ended the login")

    do { try await me.keyLogin() } catch { fail("logging in again: \(error.localizedDescription)") }
    await me.logout()
    guard !me.loggedIn else { fail("logout") }
    print("account-check: live ok")
}

@main enum AccountCheck {
    static func main() async {
        let args = CommandLine.arguments
        guard args.count >= 3 else { fail("usage: offline DIR | live DIR ORIGIN") }
        let dir = URL(fileURLWithPath: args[2], isDirectory: true)
        switch args[1] {
        case "offline": await offline(dir)
        case "live" where args.count == 4: await live(dir, origin: args[3])
        default: fail("usage: offline DIR | live DIR ORIGIN")
        }
    }
}
