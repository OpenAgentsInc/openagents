// Firm reason: 6 (shared values at the UIKit/AppKit SDK projection boundary).
import Foundation

enum NativeFailure: Error, LocalizedError {
    case refused(String)
    var errorDescription: String? { if case .refused(let text) = self { return text }; return "Unavailable" }
}

struct NativeElement:Equatable {
    var id: String
    var role: String
    var label: String
    var value = ""
    var disabled = false
    var complete = ""
    var type = ""
    var children:[NativeElement] = []
    var classes=""
    var attrs:[String:String]=[:]
}

