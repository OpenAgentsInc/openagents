//
//  Item.swift
//  Inbox Autopilot
//
//  Created by Christopher David on 2/19/26.
//

import Foundation
import SwiftData

@Model
final class Item {
    var timestamp: Date
    
    init(timestamp: Date) {
        self.timestamp = timestamp
    }
}
