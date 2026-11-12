import CryptoKit
import Foundation

// The Mac's check of DeviceKey.swift (device-key-check.sh builds and runs it):
//   device-key-check check DIR          makes or opens the key in DIR/device-key and checks it
//   device-key-check proof DIR METHOD URL MS TOKEN
//                                       prints the key, where it lives and an OA-Device-Proof
//                                       for a request, as an app would send them
// It never touches the Keychain: a rebuilt binary opens the same key with no password prompt.

func fail(_ words: String) -> Never { FileHandle.standardError.write(Data(("device-key-check: " + words + "\n").utf8)); exit(1) }
func verify(_ key: DeviceKey, _ message: String, _ signature: Data) -> Bool {
    guard let pub = try? P256.Signing.PublicKey(x963Representation: key.publicKey), let sig = try? P256.Signing.ECDSASignature(rawRepresentation: signature) else { return false }
    return pub.isValidSignature(sig, for: Data(message.utf8))
}

@main enum DeviceKeyCheck {
    static func main() throws {
        let args = CommandLine.arguments
        guard args.count >= 3 else { fail("usage: check DIR | proof DIR METHOD URL MS TOKEN") }
        let store = FileDeviceKeyStore(url: URL(fileURLWithPath: args[2]).appendingPathComponent("device-key"))
        let software = ProcessInfo.processInfo.environment["OA_DEVICE_KEY_SOFTWARE"] == "1"
        let keys = software ? DeviceKeys(store: store, enclave: false) : DeviceKeys(store: store)

        if args[1] == "proof" {
            guard args.count == 7, let ms = Int64(args[5]) else { fail("proof DIR METHOD URL MS TOKEN") }
            guard let key = try? keys.current(), let proof = try? DeviceKeys.proof(key: key, method: args[3], url: args[4], ms: ms, token: args[6]) else { fail("no key") }
            print("key \(key.publicKeyHex)\nstore \(key.store.rawValue)\nproof \(proof)")
            exit(0)
        }
        guard args[1] == "check" else { fail("usage: check DIR | proof DIR METHOD URL MS TOKEN") }

        // The message is the server's (auth-device-crypto.coax's own vector).
        let vector = "OA-Device-Proof-v1\nPOST\nhttps://openagents.com/api/auth/refresh\n1800000000000\n9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
        guard DeviceKeys.message(method: "POST", url: "https://openagents.com/api/auth/refresh", ms: 1_800_000_000_000, token: "test") == vector else { fail("the message differs from the server's") }

        let had = store.load()
        guard let key = try? keys.current() else { fail("no key could be made") }
        guard key.publicKey.count == 65, key.publicKey.first == 4 else { fail("the public key isn't 04 || x || y") }
        if !software && SecureEnclave.isAvailable && key.store != .enclave { fail("this Mac has a Secure Enclave, but the key is software") }
        guard let blob = store.load(), blob.count > 1 else { fail("the key wasn't stored") }
        if key.store == .enclave {
            // What is on disk is the enclave's wrapped blob, never the private key: it doesn't open as one.
            if (try? P256.Signing.PrivateKey(rawRepresentation: Data(blob.dropFirst()))) != nil { fail("the stored blob opens as a private key") }
        }
        let attributes = try? FileManager.default.attributesOfItem(atPath: store.url.path)
        guard (attributes?[.posixPermissions] as? NSNumber)?.intValue == 0o600 else { fail("the key file isn't 0600") }

        let proof = try DeviceKeys.proof(key: key, method: "POST", url: "https://openagents.com/api/auth/refresh", ms: 1_800_000_000_000, token: "test")
        let parts = proof.split(separator: " ")
        guard parts.count == 3, parts[0] == "v1", parts[1] == "1800000000000", parts[2].count == 128 else { fail("the proof isn't v1 <ms> <128 hex>") }
        let signature = try key.sign(Data(vector.utf8))
        guard signature.count == 64, verify(key, vector, signature) else { fail("the signature doesn't verify under the public key") }
        guard !verify(key, vector + "x", signature) else { fail("the signature verifies for another message") }

        // Another process's view: the same key from the same file.
        let again = software ? DeviceKeys(store: store, enclave: false) : DeviceKeys(store: store)
        guard let reopened = again.existing(), reopened.publicKey == key.publicKey, reopened.store == key.store else { fail("the stored key didn't reopen as itself") }
        guard verify(key, vector, try reopened.sign(Data(vector.utf8))) else { fail("the reopened key's signature doesn't verify") }

        if ProcessInfo.processInfo.environment["OA_DEVICE_KEY_FORGET"] == "1" {
            again.forget()
            guard store.load() == nil, again.existing() == nil else { fail("forget left the key") }
            guard let fresh = try? again.current(), fresh.publicKey != key.publicKey else { fail("a new login got the old key") }
            again.forget()
            print("device key: forgotten, and the next one is new")
            exit(0)
        }
        print("device key: \(key.store.rawValue), \(had == nil ? "made" : "reopened"), \(DeviceKeys.hex(key.publicKey.prefix(6)))…")
    }
}
