import AppKit
import SwiftUI

struct MarkdownMessage: View {
    let content: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(parseBlocks(), id: \.id) { block in
                switch block.kind {
                case .code(let language, let code):
                    CodeBlockView(language: language, code: code)
                case .heading(let level, let text):
                    Text(attributed(text))
                        .font(headingFont(level))
                        .textSelection(.enabled)
                case .paragraph(let text):
                    Text(attributed(text))
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func parseBlocks() -> [MarkdownBlock] {
        var blocks: [MarkdownBlock] = []
        var paragraph: [String] = []
        var code: [String] = []
        var codeLanguage: String?
        var inCode = false

        func flushParagraph() {
            let text = paragraph.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty {
                blocks.append(MarkdownBlock(kind: .paragraph(text)))
            }
            paragraph.removeAll()
        }

        for line in content.components(separatedBy: .newlines) {
            if line.hasPrefix("```") {
                if inCode {
                    blocks.append(MarkdownBlock(kind: .code(codeLanguage, code.joined(separator: "\n"))))
                    code.removeAll()
                    codeLanguage = nil
                    inCode = false
                } else {
                    flushParagraph()
                    codeLanguage = String(line.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
                    inCode = true
                }
                continue
            }

            if inCode {
                code.append(line)
            } else if line.hasPrefix("#") {
                flushParagraph()
                let level = line.prefix { $0 == "#" }.count
                let text = line.dropFirst(level).trimmingCharacters(in: .whitespaces)
                blocks.append(MarkdownBlock(kind: .heading(level, String(text))))
            } else if line.trimmingCharacters(in: .whitespaces).isEmpty {
                flushParagraph()
            } else {
                paragraph.append(line)
            }
        }

        if inCode {
            blocks.append(MarkdownBlock(kind: .code(codeLanguage, code.joined(separator: "\n"))))
        }
        flushParagraph()
        if blocks.isEmpty {
            blocks.append(MarkdownBlock(kind: .paragraph(content)))
        }
        return blocks
    }

    private func attributed(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text)) ?? AttributedString(text)
    }

    private func headingFont(_ level: Int) -> Font {
        switch level {
        case 1: return .title2.weight(.bold)
        case 2: return .title3.weight(.bold)
        case 3: return .headline
        default: return .subheadline.weight(.semibold)
        }
    }
}

private struct MarkdownBlock: Identifiable {
    let id = UUID()
    let kind: Kind

    enum Kind {
        case paragraph(String)
        case heading(Int, String)
        case code(String?, String)
    }
}

private struct CodeBlockView: View {
    let language: String?
    let code: String

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text((language?.isEmpty == false ? language! : "code").uppercased())
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(.secondary)
                Spacer()
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(code, forType: .string)
                } label: {
                    Label("Copy", systemImage: "doc.on.doc")
                }
                .buttonStyle(.plain)
                .font(.caption2.weight(.semibold))
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)

            Divider()

            ScrollView(.horizontal) {
                Text(code)
                    .font(.system(.footnote, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(12)
                    .fixedSize(horizontal: true, vertical: true)
            }
        }
        .background(Color.black.opacity(0.35), in: RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(Color.white.opacity(0.10), lineWidth: 1)
        )
    }
}
