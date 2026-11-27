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
    MacAccount(store: FileTokenStore(url: dir.appendingPathComponent("desktop/login.json")),
               device: DeviceKeys(store: FileDeviceKeyStore(url: dir.appendingPathComponent("desktop/device-key"))),
               origin: origin, send: send)
}

func offline(_ dir: URL) async {
    let fake = Fake()
    let me = account(dir, origin: "https://example.test", send: { try await fake.send($0) })
    guard me.title == "Log In…", !me.loggedIn else { fail("logged in before logging in") }

    // An email code: the code goes with the pending value; a wrong one is the server's own words.
    let tokens = dir.appendingPathComponent("desktop/login.json")
    fake.answers["/api/auth/email/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"p1\"}")]
    fake.answers["/api/auth/email/verify"] = [(401, "{\"error\":\"That code isn't right. 4 tries left.\"}"),
                                              (200, "{\"account\":\"acct_b\",\"access_token\":\"oa_at_B1\",\"refresh_token\":\"oa_rt_B1\",\"expires_in\":3600,\"device_bound\":true}")]
    fake.answers["/api/auth/account"] = [(200, "{\"logged_in\":true,\"account\":\"acct_b\",\"emails\":[\"a•••@example.com\"],\"keys\":[]}")]
    fake.answers["/api/auth/life"] = [(200, "{\"credit_msat\":42000,\"promo_micros\":15000000,\"promo_expires_at\":\"2027-02-25\"}")]
    fake.answers["/api/integrations/github/status"] = [(200, "{\"connected\":true,\"login\":\"ada\"}")]
    fake.answers["/api/projects/v1"] = [(200, "{\"projects\":[{\"name\":\"openagents\",\"repo\":\"OpenAgentsInc/openagents\"}]}")]
    guard let pending = try? await me.auth.emailStart("ada@example.com"), pending == "p1" else { fail("email start") }
    do { try await me.auth.emailVerify(code: "000000", pending: pending); fail("a wrong code logged in") }
    catch { guard error.localizedDescription == "That code isn't right. 4 tries left." else { fail("the wrong code's words: \(error.localizedDescription)") } }
    do { try await me.auth.emailVerify(code: "482913", pending: pending) } catch { fail("email verify: \(error)") }
    await me.look()
    guard me.paidCreditLabel == "42 sats", me.promoCreditLabel == "$15.00 USD", me.promoExpiresAt == "2027-02-25",
          me.githubStatus == "Connected (@ada)", me.projectsSummary == "1 project" else {
        fail("v1 account credits/pipes/projects: paid=\(me.paidCreditLabel) promo=\(me.promoCreditLabel) gh=\(me.githubStatus) proj=\(me.projectsSummary)")
    }
    let verify = fake.asked.last { $0.url?.path == "/api/auth/email/verify" }
    guard me.title == "Logged in as a•••@example.com", String(decoding: verify?.httpBody ?? Data(), as: UTF8.self) == "{\"code\":\"482913\",\"pending\":\"p1\"}",
          verify?.value(forHTTPHeaderField: "OA-Device-Platform") == "mac" else { fail("the email login") }
    // The tokens: 0600 in a 0700 folder, device-bound.
    guard mode(tokens) == 0o600, mode(tokens.deletingLastPathComponent()) == 0o700, me.store.load()?.deviceBound == true else { fail("the token file isn't 0600, or lost the binding") }
    // Security (#13233): the sheet's words from the recovery and the notices; this login cancels
    // the recovery; and the push target is Apple's token as hex.
    fake.answers["/api/auth/recovery"] = [(200, "{\"state\":\"pending\",\"ready_ms\":1800259200000}")]
    fake.answers["/api/auth/notices"] = [(200, "{\"notices\":[{\"at_ms\":1800000000000,\"what\":\"recovery.start\",\"words\":\"Someone asked to recover it.\"}]}")]
    guard let sec = try? await me.auth.security(), sec.recoveryOpen,
          MacAccount.securityWords(sec, zone: TimeZone(identifier: "America/Chicago")!) ==
            "A recovery of this account is waiting. After Jan 18, 2:00 AM CST a new key can replace every key and passkey on it.\n\nNotices\nJan 15, 2:00 AM\nSomeone asked to recover it."
    else { fail("the Security sheet's words") }
    fake.answers["/api/auth/recovery/cancel"] = [(200, "{\"state\":\"cancelled\"}")]
    do { try await me.auth.cancelRecovery() } catch { fail("cancel recovery: \(error)") }
    let cancel = fake.asked.last { $0.url?.path == "/api/auth/recovery/cancel" }
    guard cancel?.value(forHTTPHeaderField: "Authorization") == "Bearer oa_at_B1" else { fail("the recovery wasn't cancelled with this login") }
    fake.answers["/api/auth/devices/push"] = [(200, "{\"state\":\"on\",\"device\":1}")]
    do { try await me.auth.pushTarget(Data([0xab, 0x01])) } catch { fail("push target: \(error)") }
    let push = fake.asked.last { $0.url?.path == "/api/auth/devices/push" }
    guard String(decoding: push?.httpBody ?? Data(), as: UTF8.self) == "{\"service\":\"\(AppAuth.pushService)\",\"token\":\"ab01\"}" else { fail("the push target's body") }
    // a notice's push opens the Security sheet for its own page and nothing else
    guard AppAuth.noticeOpens(["aps": ["alert": ["title": "t", "body": "b"]], "oa": ["notice": 12, "what": "recovery.start", "open": "/settings/security"]]),
          !AppAuth.noticeOpens(["oa": ["open": "/settings/wallet"]]), !AppAuth.noticeOpens(["oa": ["open": "https://example.com/settings/security"]]),
          !AppAuth.noticeOpens(["open": "/settings/security"]), !AppAuth.noticeOpens([:]) else { fail("what a notice's push opens") }
    // Freeze: a refusal leaves the login; a freeze ends it here too (the tokens and the device key go)
    fake.answers["/api/auth/freeze"] = [(403, "{\"error\":\"Only a person can do this, not an agent.\"}"), (200, "{\"state\":\"frozen\"}")]
    do { try await me.auth.freeze(); fail("a refused freeze passed") }
    catch { guard error.localizedDescription == "Only a person can do this, not an agent.", me.loggedIn else { fail("a refused freeze: \(error.localizedDescription)") } }
    do { try await me.auth.freeze() } catch { fail("freeze: \(error)") }
    guard !me.loggedIn, !FileManager.default.fileExists(atPath: tokens.path) else { fail("a freeze left the login") }
    // log in again for the logout below
    fake.answers["/api/auth/email/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"p2\"}")]
    fake.answers["/api/auth/email/verify"] = [(200, "{\"account\":\"acct_b\",\"access_token\":\"oa_at_B2\",\"refresh_token\":\"oa_rt_B2\",\"expires_in\":3600,\"device_bound\":true}")]
    guard let again = try? await me.auth.emailStart("ada@example.com"), (try? await me.auth.emailVerify(code: "482913", pending: again)) != nil, me.loggedIn else { fail("the login after a freeze") }
    fake.answers["/api/auth/logout"] = [(200, "{}")]
    await me.logout()

    // WorkOS Phase 4 (#14101): config, Magic Auth + multi-org selection, PKCE S256 + state guard +
    // /api/auth/workos/exchange (OA-Device-Key + 0600 file), SSO discovery, orgs switch, and
    // single-flight refresh preserving tokens on 429/503/offline while revoking on 401.
    fake.answers["/api/auth/workos/config"] = [(200, "{\"enabled\":true,\"client_id\":\"client_01TEST\",\"providers\":[\"magic\",\"password\",\"google\",\"github\",\"apple\",\"sso\"]}")]
    guard let wcfg = try? await me.auth.workosConfig(), wcfg.enabled, wcfg.clientID == "client_01TEST", wcfg.providers.contains("apple"), wcfg.providers.contains("password") else { fail("workos config") }

    // WorkOS Email + Password login, register (verify-email), and reset
    fake.answers["/api/auth/workos/password/register"] = [(200, "{\"state\":\"verify-email\",\"pending\":\"wv_pw\"}")]
    fake.answers["/api/auth/workos/password/login"] = [(200, "{\"state\":\"need-org\",\"pending\":\"wo_pw\",\"organizations\":[{\"id\":\"org_1\",\"name\":\"Acme\"}]}")]
    fake.answers["/api/auth/workos/password/reset"] = [(200, "{\"state\":\"sent\"}")]
    guard case .verifyEmail(let vPend) = try? await me.auth.workosPasswordRegister(email: "pw@acme.test", password: "correct horse battery"),
          vPend == "wv_pw" else { fail("workos password register verify-email") }
    let pwRegReq = fake.asked.last { $0.url?.path == "/api/auth/workos/password/register" }
    guard pwRegReq?.value(forHTTPHeaderField: "OA-Device-Key")?.count == 130 else { fail("workos password register missing OA-Device-Key") }
    guard case .needOrg(let pwPend, let pwOrgs) = try? await me.auth.workosPasswordLogin(email: "pw@acme.test", password: "correct horse battery"),
          pwPend == "wo_pw", pwOrgs.count == 1 else { fail("workos password login need-org") }
    do { try await me.auth.workosPasswordReset(email: "pw@acme.test") } catch { fail("workos password reset: \(error)") }

    fake.answers["/api/auth/workos/magic/start"] = [(200, "{\"state\":\"sent\",\"pending\":\"wp_1\"}")]
    fake.answers["/api/auth/workos/magic/verify"] = [(200, "{\"state\":\"need-org\",\"pending\":\"wo_1\",\"organizations\":[{\"id\":\"org_1\",\"name\":\"Acme\"},{\"id\":\"org_2\",\"name\":\"Beta\"}]}")]
    fake.answers["/api/auth/workos/org/select"] = [(200, "{\"state\":\"logged-in\",\"account\":\"acct_w1\",\"access_token\":\"oa_at_W1\",\"refresh_token\":\"oa_rt_W1\",\"expires_in\":3600,\"device_bound\":true}")]
    guard let wp = try? await me.auth.workosMagicStart("alice@acme.test"), wp == "wp_1" else { fail("workos magic start") }
    guard case .needOrg(let orgPend, let orgs) = try? await me.auth.workosMagicVerify(code: "123456", pending: wp),
          orgPend == "wo_1", orgs.count == 2, orgs[0].id == "org_1" else { fail("workos need-org") }
    guard (try? await me.auth.workosSelectOrg(pending: orgPend, organizationID: "org_1")) != nil,
          me.loggedIn, mode(tokens) == 0o600 else { fail("workos org select") }
    let orgSelReq = fake.asked.last { $0.url?.path == "/api/auth/workos/org/select" }
    guard orgSelReq?.value(forHTTPHeaderField: "OA-Device-Platform") == "mac",
          orgSelReq?.value(forHTTPHeaderField: "OA-Device-Key")?.count == 130 else { fail("workos org select missing OA-Device-Key") }

    // WorkOS PKCE S256 start, callback constant-time state guard, and /exchange
    guard let pkce = try? AppAuth.workosPKCEStart(clientID: "client_01TEST", provider: "AppleOAuth", loginHint: "alice@acme.test"),
          pkce.authorizeURL.absoluteString.contains("code_challenge_method=S256"),
          pkce.authorizeURL.absoluteString.contains("provider=AppleOAuth"),
          pkce.verifier.count == 43, pkce.state.count == 43 else { fail("workos pkce start") }
    let goodCB = URL(string: "openagents://auth/callback?code=code_99&state=\(pkce.state)")!
    let badStateCB = URL(string: "openagents://auth/callback?code=code_99&state=wrong_state")!
    let errCB = URL(string: "openagents://auth/callback?error=access_denied&state=\(pkce.state)")!
    guard (try? AppAuth.workosCallbackCode(callbackURL: goodCB, expectedState: pkce.state)) == "code_99" else { fail("workos callback valid") }
    guard (try? AppAuth.workosCallbackCode(callbackURL: badStateCB, expectedState: pkce.state)) == nil else { fail("workos callback accepted bad state") }
    guard (try? AppAuth.workosCallbackCode(callbackURL: errCB, expectedState: pkce.state)) == nil else { fail("workos callback accepted error") }
    fake.answers["/api/auth/workos/exchange"] = [(200, "{\"state\":\"logged-in\",\"account\":\"acct_w2\",\"access_token\":\"oa_at_W2\",\"refresh_token\":\"oa_rt_W2\",\"expires_in\":3600,\"device_bound\":true}")]
    guard case .loggedIn(let exTok) = try? await me.auth.workosExchangePKCE(code: "code_99", pkce: pkce),
          exTok.account == "acct_w2", exTok.deviceBound == true else { fail("workos pkce exchange") }
    let exReq = fake.asked.last { $0.url?.path == "/api/auth/workos/exchange" }
    guard exReq?.value(forHTTPHeaderField: "OA-Device-Key")?.count == 130 else { fail("workos exchange missing OA-Device-Key") }

    // Enterprise SSO discovery + orgs list & switch
    fake.answers["/api/auth/workos/sso/discover"] = [(200, "{\"sso\":true,\"connection_id\":\"conn_1\",\"organization_id\":\"org_1\",\"organization_name\":\"Acme\"}")]
    guard let disc = try? await me.auth.workosDiscoverSSO(email: "alice@acme.test"), disc.sso, disc.connectionID == "conn_1" else { fail("workos sso discover") }
    fake.answers["/api/auth/workos/orgs"] = [(200, "{\"organizations\":[{\"id\":\"org_1\",\"name\":\"Acme\"},{\"id\":\"org_2\",\"name\":\"Beta\"}]}")]
    guard let myOrgs = try? await me.auth.workosOrgs(), myOrgs.count == 2 else { fail("workos orgs") }
    fake.answers["/api/auth/workos/orgs/switch"] = [(200, "{\"organization_id\":\"org_2\"}")]
    guard (try? await me.auth.workosSwitchOrg("org_2")) == "org_2" else { fail("workos org switch") }

    // Transient 429 / 503 during refresh MUST preserve tokens; 401 MUST clear tokens and device key.
    var staleW = exTok; staleW.accessUntil = Date(timeIntervalSince1970: 0); me.store.save(staleW)
    fake.answers["/api/auth/refresh"] = [
        (429, "{\"error\":\"Slow down.\"}"),
        (503, "{\"error\":\"Temporarily unavailable.\"}"),
        (401, "{\"error\":\"Log in again.\"}"),
    ]
    do { _ = try await me.auth.refreshed(); fail("429 refreshed") } catch {}
    guard me.loggedIn, me.store.load()?.refresh == "oa_rt_W2" else { fail("429 cleared tokens") }
    do { _ = try await me.auth.refreshed(); fail("503 refreshed") } catch {}
    guard me.loggedIn, me.store.load()?.refresh == "oa_rt_W2" else { fail("503 cleared tokens") }
    do { _ = try await me.auth.refreshed(); fail("401 refreshed") } catch {}
    guard !me.loggedIn, !FileManager.default.fileExists(atPath: tokens.path) else { fail("401 left tokens") }

    print("account-check: offline ok (email code, title, tokens 0600, security, push target, freeze, WorkOS magic/PKCE/SSO/orgs/transient-refresh, logout)")
}

@main enum AccountCheck {
    static func main() async {
        let args = CommandLine.arguments
        guard args.count >= 3 else { fail("usage: offline DIR") }
        let dir = URL(fileURLWithPath: args[2], isDirectory: true)
        switch args[1] {
        case "offline": await offline(dir)
        default: fail("usage: offline DIR")
        }
    }
}
