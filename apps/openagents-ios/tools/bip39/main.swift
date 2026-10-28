import Foundation
let data = try! Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
guard let words = BIP39.englishWords(fileContents: data) else { fatalError("list rejected") }
func hex(_ s: String) -> Data { var d = Data(); var i = s.startIndex; while i < s.endIndex { let j = s.index(i, offsetBy: 2); d.append(UInt8(s[i..<j], radix: 16)!); i = j }; return d }
let vectors: [(String, String)] = [
 ("00000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"),
 ("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f", "legal winner thank year wave sausage worth useful legal winner thank yellow"),
 ("80808080808080808080808080808080", "letter advice cage absurd amount doctor acoustic avoid letter advice cage above"),
 ("ffffffffffffffffffffffffffffffff", "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong"),
 ("0000000000000000000000000000000000000000000000000000000000000000", "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art"),
 ("7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f", "legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth useful legal winner thank year wave sausage worth title"),
 ("9e885d952ad362caeb4efe34a8e91bd2", "ozone drill grab fiber curtain grace pudding thank cruise elder eight picnic"),
 ("f585c11aec520db57dd353c69554b21a89b20fb0650966fa0a9d6f74fd989d8f", "void come effort suffer camp survey warrior heavy shoot primary clutch crush open amazing screen patrol group space point ten exist slush involve unfold"),
]
var ok = 0
for (e, m) in vectors { if BIP39.mnemonic(entropy: hex(e), words: words) == m { ok += 1 } else { print("FAIL", e) } }
precondition(BIP39.mnemonic(entropy: Data(count: 20), words: words) == nil)
var bad = data; bad[0] ^= 1; precondition(BIP39.englishWords(fileContents: bad) == nil)
print("\(ok)/\(vectors.count) vectors pass; bad lengths and a changed list refused")
