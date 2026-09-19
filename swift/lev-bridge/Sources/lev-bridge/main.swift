import Foundation
import FoundationModels

// A line-delimited JSON bridge to Apple's on-device foundation model.
//
// One request object per line on stdin, one response object per line on
// stdout. Diagnostics go to stderr so the supervisor can capture them.
// Earlier bridges in this workspace hand-parsed HTTP on a socket, which an
// audit called naive and truncation-prone; a line protocol removes that
// surface entirely.

setvbuf(stdout, nil, _IOLBF, 0)

let decoder = JSONDecoder()
let encoder = JSONEncoder()
encoder.outputFormatting = [.sortedKeys]

func emit(_ response: Response) {
    guard let data = try? encoder.encode(response),
          let line = String(data: data, encoding: .utf8)
    else {
        FileHandle.standardError.write(Data("lev-bridge: response encode failed\n".utf8))
        return
    }
    print(line)
}

while let line = readLine(strippingNewline: true) {
    if line.isEmpty { continue }
    guard let data = line.data(using: .utf8),
          let request = try? decoder.decode(Request.self, from: data)
    else {
        emit(Response.failure(id: "", code: "invalid_request", message: "request did not parse"))
        continue
    }
    let response = await handle(request)
    emit(response)
}
