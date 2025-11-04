import Foundation

actor TitleCache {
    static let shared = TitleCache()
    private let defaultsKey = "title_cache_v1"
    private var store: [String: String]

    init() {
        if let dict = UserDefaults.standard.dictionary(forKey: defaultsKey) as? [String: String] {
            store = dict
        } else {
            store = [:]
        }
    }

    private func key(path: String, mtime: Int64) -> String { return path + "|" + String(mtime) }

    func get(path: String, mtime: Int64) -> String? {
        store[key(path: path, mtime: mtime)]
    }

    func set(path: String, mtime: Int64, title: String) {
        store[key(path: path, mtime: mtime)] = title
        UserDefaults.standard.set(store, forKey: defaultsKey)
    }
}

