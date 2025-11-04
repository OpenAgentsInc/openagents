import Foundation

actor TitleCache {
    static let shared = TitleCache()
    // Exact cache by (path|mtime) for reproducibility
    private let exactKey = "title_cache_v1"
    private var exact: [String: String]
    // Latest cache by path to avoid constant regeneration when mtime advances frequently
    private let latestTitleKey = "title_cache_latest_title_v1"
    private let latestMtimeKey = "title_cache_latest_mtime_v1"
    private var latestTitle: [String: String]
    private var latestMtime: [String: Int64]

    init() {
        let d = UserDefaults.standard
        exact = (d.dictionary(forKey: exactKey) as? [String: String]) ?? [:]
        latestTitle = (d.dictionary(forKey: latestTitleKey) as? [String: String]) ?? [:]
        if let mt = d.dictionary(forKey: latestMtimeKey) as? [String: NSNumber] {
            var tmp: [String: Int64] = [:]
            for (k, v) in mt { tmp[k] = v.int64Value }
            latestMtime = tmp
        } else {
            latestMtime = [:]
        }
    }

    private func key(path: String, mtime: Int64) -> String { return path + "|" + String(mtime) }

    /// Return a cached title for this path+mtime.
    /// Falls back to the latest known title for the path when an exact match isn't present.
    func get(path: String, mtime: Int64) -> String? {
        if let t = exact[key(path: path, mtime: mtime)] { return t }
        if let t = latestTitle[path] { return t }
        return nil
    }

    /// Store both the exact title for (path, mtime) and the latest title for path.
    func set(path: String, mtime: Int64, title: String) {
        exact[key(path: path, mtime: mtime)] = title
        if let prev = latestMtime[path] {
            if mtime >= prev { latestTitle[path] = title; latestMtime[path] = mtime }
        } else {
            latestTitle[path] = title; latestMtime[path] = mtime
        }
        persist()
    }

    private func persist() {
        let d = UserDefaults.standard
        d.set(exact, forKey: exactKey)
        d.set(latestTitle, forKey: latestTitleKey)
        var boxed: [String: NSNumber] = [:]
        for (k, v) in latestMtime { boxed[k] = NSNumber(value: v) }
        d.set(boxed, forKey: latestMtimeKey)
    }
}
