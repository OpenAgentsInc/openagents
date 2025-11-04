import SwiftUI

struct RawThreadView: View {
    let url: URL?

    var body: some View {
        Group {
            if let u = url {
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(u.lastPathComponent)
                            .font(Font.custom(BerkeleyFont.defaultName(), size: 12, relativeTo: .caption))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        Text(truncatedText(from: u))
                            .textSelection(.enabled)
                            .font(BerkeleyFont.font(relativeTo: .body, size: 14))
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .padding(.top, 2)
                        Text("(Truncated to first 1000 characters)")
                            .font(Font.custom(BerkeleyFont.defaultName(), size: 10, relativeTo: .caption2))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                    .padding()
                    .background(OATheme.Colors.background)
                }
            } else {
                Text("Select a thread")
                    .font(.headline)
                    .foregroundStyle(OATheme.Colors.textSecondary)
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
