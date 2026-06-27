import SwiftUI

public struct ConnectionSettingsView: View {
    public let connections: [ActiveConnection]

    @Environment(\.dismiss) private var dismiss

    public init(connections: [ActiveConnection]) {
        self.connections = connections
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("Connection Settings")
                    .font(.title2.bold())

                Spacer()

                Button("Done") {
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
            .padding()

            Divider()

            HStack(spacing: 0) {
                List {
                    Label("Connections", systemImage: "externaldrive.connected.to.line.below")
                    Label("General", systemImage: "gearshape")
                    Label("Shortcuts", systemImage: "keyboard")
                }
                .listStyle(.sidebar)
                .frame(width: 190)

                Divider()

                Form {
                    Section("Active Connections") {
                        ForEach(connections) { connection in
                            LabeledContent(connection.name) {
                                Text("\(connection.kind) / \(connection.status)")
                            }
                        }
                    }
                }
                .formStyle(.grouped)
                .padding()
            }
        }
        .frame(minWidth: 720, minHeight: 520)
    }
}
