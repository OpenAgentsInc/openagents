import SwiftUI

// SCR-21 Test set (sheet). Every test in a test set: read-only for a
// published one (DONE), and a draft the player approves (LOOKS GOOD, the
// same as CARD-02.E05). Opened from CARD-01.E06, CARD-02.E04, CARD-04.E06,
// and SCR-05.E13.

struct SCR21TestSet: View {
    @Environment(MockApp.self) private var app
    let setID: String
    var draft = false

    private var set: MockData.TestSet { MockData.testSet(setID) }

    var body: some View {
        VStack(spacing: 0) {
            // E01
            HStack {
                Text("\(set.toolName) · \(set.tests.count) tests\(draft ? " · draft" : "")")
                    .condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                    .lineLimit(1).minimumScaleFactor(0.8)
                Spacer()
                Button { app.sheet = nil } label: {
                    Image(systemName: "xmark").font(.paper(16, weight: .bold)).frame(width: 44, height: 44)
                }
                .accessibilityLabel("Close")
            }
            .padding(.horizontal, Theme.Space.page)
            .padding(.top, Theme.Space.xs)
            Divider().overlay(Theme.Colors.divider)

            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(set.tests.enumerated()), id: \.offset) { i, test in
                        HStack(alignment: .firstTextBaseline, spacing: 12) {
                            Text("\(i + 1)").font(Theme.Fonts.bodyBold).frame(width: 18, alignment: .trailing)
                            VStack(alignment: .leading, spacing: 3) {
                                // E02
                                Text(test.name).font(Theme.Fonts.bodyBold)
                                    .fixedSize(horizontal: false, vertical: true)
                                if test.stayOut {
                                    Text(MockData.stayOutNote).font(Theme.Fonts.body)
                                        .foregroundStyle(Theme.Colors.textSecondary)
                                }
                                // E03
                                Text(test.checked).font(Theme.Fonts.body)
                                    .foregroundStyle(Theme.Colors.textSecondary)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                        .padding(.vertical, 10)
                        Divider().overlay(Theme.Colors.divider)
                    }
                    // E04
                    Text(set.madeBy).font(Theme.Fonts.caption).foregroundStyle(Theme.Colors.textTertiary)
                        .padding(.top, Theme.Space.s)
                }
                .padding(.horizontal, Theme.Space.page)
            }

            // E05
            VStack(spacing: Theme.Space.xs) {
                NextLine(text: draft ? "approve these tests, or close to keep changing them." : "close when you're done reading.")
                PrimaryButton(title: draft ? "Looks good" : "Done") {
                    if draft { app.post(.approveDraft) }
                    app.sheet = nil
                }
            }
            .padding(.horizontal, Theme.Space.page)
            .padding(.vertical, Theme.Space.xs)
        }
        .foregroundStyle(Theme.Colors.textPrimary)
    }
}

#Preview("SCR-21 Test set") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR21TestSet(setID: "project-map").presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}

#Preview("SCR-21 Draft") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR21TestSet(setID: "changelog", draft: true).presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}

#Preview("SCR-21 Another trainer's") {
    Color.black.sheet(isPresented: .constant(true)) {
        SCR21TestSet(setID: "test-reader-2px").presentationBackground(Theme.Colors.surface)
    }
    .environment(MockApp())
}
