// Firm reason: 6 (Swift-only host code: the Mac and iOS hosts share this table; it holds no product logic).
// The worlds a host runs as the web's realms, through Aethyr's page host (aethyr/host/apple/
// page-host.h), by their world's number in scenes.coax (1 Everglade, 3 the Archery Range,
// 4 Experimental). Each row is what the web's page for that world says of itself, its script tag
// in apps/openagents-web/realms/: the module, the page's path on the site, its realm and slot, and
// where its pack is in the app. The Mac host (apps/everglade-desktop) and the iOS host
// (apps/everglade-mobile) read the same rows; a host says it runs a row's world with eg_runs.
struct Realm {
  let module: String, path: String, realm: String, slot: String, pack: String
}

enum Realms {
  static let table: [Int32: Realm] = [
    1: Realm(module: "everglade2", path: "/worlds/character-creator", realm: "everglade2", slot: "everglade2", pack: "everglade2.pack"),
    3: Realm(module: "fivelab", path: "/worlds/archery-range", realm: "archery-range", slot: "archery-range", pack: "fivelab/glade.pack.gz"),
    4: Realm(module: "fivelab", path: "/worlds/everglade-experimental", realm: "everglade-experimental", slot: "everglade-experimental", pack: "fivelab/glade.pack.gz"),
  ]
  // The other worlds whose realm is the same module. A module's library is loaded once in a
  // process and its face keeps some of a page's state beside the state it is handed (your body,
  // the driver), so only one page of a module is open at a time: opening one closes the others.
  // Each page's save slot keeps what is saved.
  static func sharing(_ world: Int32) -> [Int32] {
    guard let r = table[world] else { return [] }
    return table.filter { $0.key != world && $0.value.module == r.module }.map { $0.key }.sorted()
  }
}
