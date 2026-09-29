import SwiftUI

// CARD-05 Gym news card: what's new and what's in progress (CHAT-9), from
// the Gym's records. Every item names where it came from; at most one offer.

enum CARD05State: String, Hashable, CaseIterable {
    case items, empty
}

struct CARD05News: View {
    var state: CARD05State = .items
    /// Opens an item's card (an answer id), or sends a chip.
    var onAsk: (String) -> Void = { _ in }

    var body: some View {
        if state == .empty {
            VStack(alignment: .leading, spacing: Theme.Space.s) {
                ChatCardFrame {
                    Text(MockData.newsEmpty).font(Theme.Fonts.body)
                    CardNote(text: MockData.newsSource, tertiary: true)
                }
                Chip(icon: "dumbbell", text: "Test a tool") { onAsk("testATool") }
            }
        } else {
            ChatCardFrame(highlighted: true) {
                // E01: up to 5 items, each from one record.
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(MockData.news.prefix(5)) { item in
                        Button { if let id = item.opens { onAsk(id) } } label: {
                            HStack(alignment: .top, spacing: 10) {
                                Circle().fill(Theme.Colors.textPrimary).frame(width: 7, height: 7).padding(.top, 8)
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(item.text).font(Theme.Fonts.body)
                                        .multilineTextAlignment(.leading)
                                        .fixedSize(horizontal: false, vertical: true)
                                    Text(item.detail).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textSecondary)
                                }
                                Spacer(minLength: 0)
                                if item.opens != nil {
                                    Image(systemName: "chevron.right").font(.system(size: 13, weight: .bold))
                                        .foregroundStyle(Theme.Colors.textTertiary).padding(.top, 5)
                                }
                            }
                            .foregroundStyle(Theme.Colors.textPrimary)
                            .padding(.vertical, 8)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(PressStyle())
                        .disabled(item.opens == nil)
                    }
                }
                // E02: the one offer.
                PrimaryButton(title: MockData.newsOffer) { onAsk("checkAResult") }
                // E03
                CardNote(text: MockData.newsSource, tertiary: true)
            }
        }
    }
}

#Preview("CARD-05 News") {
    ScrollView { CARD05News().padding() }.background(Theme.Colors.background)
}

#Preview("CARD-05 Nothing new") {
    ScrollView { CARD05News(state: .empty).padding() }.background(Theme.Colors.background)
}
