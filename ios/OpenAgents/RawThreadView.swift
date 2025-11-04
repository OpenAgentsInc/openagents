import SwiftUI

struct RawThreadView: View {
    let url: URL?

    var body: some View {
        Group {
            if let u = url {
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(u.lastPathComponent)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text(truncatedText(from: u))
                            .textSelection(.enabled)
                            .font(.system(.body, design: .monospaced))
                            .padding(.top, 2)
                        Text("(Truncated to first 1000 characters)")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .padding()
                }
            } else {
                Text("Select a thread")
                    .font(.headline)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private func truncatedText(from url: URL) -> String {
        // Read up to 8192 bytes and then cut to first 1000 characters
        do {
            let fh = try FileHandle(forReadingFrom: url)
            defer { try? fh.close() }
            let data = try fh.read(upToCount: 8192) ?? Data()
            // Decode UTF-8; fallback to lossy decoding if needed
            let decoded = String(data: data, encoding: .utf8) ?? String(decoding: data, as: UTF8.self)
            let prefix = decoded.prefix(1000)
            return String(prefix)
        } catch {
            return "(failed to read: \(error.localizedDescription))"
        }
    }
}

#Preview { RawThreadView(url: nil) }
