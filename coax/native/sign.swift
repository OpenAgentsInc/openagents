// Firm reason: 6 (CryptoKit's Ed25519 signing SDK and native file permissions).
import Foundation
import CryptoKit
let args=CommandLine.arguments
guard args.count>=3 else { fatalError("sign keygen KEY PUBLIC or sign KEY MANIFEST SIGNATURE") }
let keyURL=URL(fileURLWithPath:args[2])
if args[1]=="keygen" {
    guard args.count==4 else { fatalError("keygen KEY PUBLIC") }
    try FileManager.default.createDirectory(at:keyURL.deletingLastPathComponent(),withIntermediateDirectories:true)
    let key:Curve25519.Signing.PrivateKey
    if FileManager.default.fileExists(atPath:keyURL.path) { key=try .init(rawRepresentation:Data(contentsOf:keyURL)) }
    else { key=Curve25519.Signing.PrivateKey(); try key.rawRepresentation.write(to:keyURL,options:.atomic); try FileManager.default.setAttributes([.posixPermissions:0o600],ofItemAtPath:keyURL.path) }
    try Data(key.publicKey.rawRepresentation.base64EncodedString().utf8).write(to:URL(fileURLWithPath:args[3]),options:.atomic)
} else {
    guard args.count==5 else { fatalError("sign KEY MANIFEST SIGNATURE") }
    let key=try Curve25519.Signing.PrivateKey(rawRepresentation:Data(contentsOf:keyURL))
    let raw=try Data(contentsOf:URL(fileURLWithPath:args[3])); guard raw.count<=16384 else { fatalError("manifest too large") }
    let out=try key.signature(for:Data("Coax Native Package v1\0".utf8)+raw)
    try out.write(to:URL(fileURLWithPath:args[4]),options:.atomic)
}
