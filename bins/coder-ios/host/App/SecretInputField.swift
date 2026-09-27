// A masked field for a secret input request, such as an owner key or an SSH
// password. The value lives only in the caller's binding until it is passed
// to Rust. Nothing here echoes, logs, persists, or suggests it.
import SwiftUI

struct SecretInputField: View {
    let label: String
    @Binding var value: String

    var body: some View {
        SecureField(label, text: $value)
            .autocorrectionDisabled().textInputAutocapitalization(.never)
            .privacySensitive()
            .accessibilityLabel(label).accessibilityIdentifier("computers-input")
    }
}
