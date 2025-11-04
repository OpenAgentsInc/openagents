#if os(macOS)
import XCTest
import Network
import Foundation
import Testing

final class DesktopServerMobileClientHandshakeTests: XCTestCase {
    let correctToken = "valid_token_123"
    var listener: NWListener!
    var listenerPort: NWEndpoint.Port!

    override func tearDown() {
        listener?.cancel()
        listener = nil
    }

    func startServer(expectation: XCTestExpectation, token: String) async throws {
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true

        listener = try NWListener(using: parameters, on: .any)
        listener.service = NWListener.Service(name: "TestService", type: "_ws._tcp")

        listener.newConnectionHandler = { connection in
            connection.start(queue: .global())

            self.handleConnection(connection, expectedToken: token, expectation: expectation)
        }

        listener.start(queue: .global())

        // Wait briefly for listener to start and get port
        for _ in 0..<10 {
            if let port = listener.port {
                listenerPort = port
                expectation.fulfill()
                return
            }
            try await Task.sleep(nanoseconds: 50_000_000) // 50ms
        }
        XCTFail("Listener did not start and get port in time")
    }

    func handleConnection(_ connection: NWConnection, expectedToken: String, expectation: XCTestExpectation) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 1024) { data, _, isComplete, error in
            guard let data = data, error == nil else {
                expectation.fulfill()
                connection.cancel()
                return
            }
            // Expect: first message is token string
            let receivedToken = String(data: data, encoding: .utf8) ?? ""
            if receivedToken == expectedToken {
                // Send back "OK"
                connection.send(content: "OK".data(using: .utf8), completion: .contentProcessed { _ in
                    connection.cancel()
                    expectation.fulfill()
                })
            } else {
                // Send back "FAIL"
                connection.send(content: "FAIL".data(using: .utf8), completion: .contentProcessed { _ in
                    connection.cancel()
                    expectation.fulfill()
                })
            }
        }
    }

    func clientConnect(token: String) async throws -> Bool {
        guard let port = listenerPort else { return false }

        let connection = NWConnection(host: NWEndpoint.Host("127.0.0.1"), port: port, using: .tcp)

        return try await withCheckedThrowingContinuation { continuation in
            connection.stateUpdateHandler = { newState in
                switch newState {
                case .ready:
                    // Send token
                    connection.send(content: token.data(using: .utf8), completion: .contentProcessed { error in
                        if let error = error {
                            continuation.resume(returning: false)
                            connection.cancel()
                            return
                        }
                        // Wait for response
                        connection.receive(minimumIncompleteLength: 1, maximumLength: 10) { data, _, _, _ in
                            if let data = data, let response = String(data: data, encoding: .utf8) {
                                connection.cancel()
                                continuation.resume(returning: response == "OK")
                            } else {
                                connection.cancel()
                                continuation.resume(returning: false)
                            }
                        }
                    })
                case .failed(_), .cancelled:
                    continuation.resume(returning: false)
                default:
                    break
                }
            }
            connection.start(queue: .global())
        }
    }

    func testSuccessfulHandshake() async throws {
        let serverStarted = expectation(description: "Server started")
        try await startServer(expectation: serverStarted, token: correctToken)
        wait(for: [serverStarted], timeout: 1)

        let clientConnected = try await clientConnect(token: correctToken)
        XCTAssertTrue(clientConnected, "Client should connect and handshake successfully with correct token")
    }

    func testFailedHandshakeWrongToken() async throws {
        let serverStarted = expectation(description: "Server started")
        try await startServer(expectation: serverStarted, token: correctToken)
        wait(for: [serverStarted], timeout: 1)

        let clientConnected = try await clientConnect(token: "wrong_token")
        XCTAssertFalse(clientConnected, "Client connection should fail with wrong token")
    }
}

#endif
