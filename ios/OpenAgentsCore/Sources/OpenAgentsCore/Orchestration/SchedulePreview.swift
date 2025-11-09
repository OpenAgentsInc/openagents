import Foundation

// MARK: - Schedule Preview

/// Utilities for previewing cron schedules in human-readable format
public struct SchedulePreview {
    // MARK: - Next Run Calculation

    /// Calculate the next N run times for a schedule
    ///
    /// Supports cross-midnight windows (e.g., 23:00 → 05:00)
    public static func nextRuns(
        schedule: OrchestrationConfig.Schedule,
        count: Int = 5,
        from date: Date = Date()
    ) -> [Date] {
        guard let windowStart = schedule.windowStart,
              let windowEnd = schedule.windowEnd else {
            return []
        }

        // Parse time window
        guard let startComponents = parseTime(windowStart),
              let endComponents = parseTime(windowEnd) else {
            return []
        }

        // Parse cron expression (basic support for */N patterns)
        let parts = schedule.expression.split(separator: " ")
        guard parts.count == 5 else { return [] }

        let minutePart = String(parts[0])
        let hourPart = String(parts[1])

        // Extract interval from */N pattern
        let interval: Int
        if minutePart.hasPrefix("*/") {
            interval = Int(minutePart.dropFirst(2)) ?? 30
        } else {
            interval = 30 // Default
        }

        var runs: [Date] = []
        var current = date

        let calendar = Calendar.current

        // Generate next N runs
        while runs.count < count {
            // Move to next potential run time
            current = calendar.date(byAdding: .minute, value: interval, to: current) ?? current

            // Check if within time window
            if isWithinWindow(
                date: current,
                windowStart: startComponents,
                windowEnd: endComponents,
                calendar: calendar
            ) {
                runs.append(current)
            }

            // Safety: don't loop forever
            if calendar.dateComponents([.day], from: date, to: current).day ?? 0 > 7 {
                break
            }
        }

        return runs
    }

    /// Format schedule as human-readable description
    ///
    /// Example: "Every 30 minutes between 1:00 AM and 5:00 AM"
    public static func humanReadable(schedule: OrchestrationConfig.Schedule) -> String {
        guard let windowStart = schedule.windowStart,
              let windowEnd = schedule.windowEnd else {
            return "Invalid schedule"
        }

        // Parse cron to extract interval
        let parts = schedule.expression.split(separator: " ")
        guard parts.count == 5 else {
            return "Custom schedule: \(schedule.expression)"
        }

        let minutePart = String(parts[0])
        let interval: Int
        if minutePart.hasPrefix("*/") {
            interval = Int(minutePart.dropFirst(2)) ?? 30
        } else {
            interval = 30
        }

        // Format start/end times
        let startFormatted = formatTime(windowStart)
        let endFormatted = formatTime(windowEnd)

        // Check for cross-midnight
        let crossesMidnight = windowStart >= windowEnd

        if crossesMidnight {
            return "Every \(interval) minutes between \(startFormatted) and \(endFormatted) (overnight)"
        } else {
            return "Every \(interval) minutes between \(startFormatted) and \(endFormatted)"
        }
    }

    /// Generate safe cron expression from time window
    ///
    /// Example: windowStart="01:00", windowEnd="05:00", interval=30
    ///          → "*/30 1-5 * * *"
    public static func deriveCron(
        windowStart: String,
        windowEnd: String,
        interval: Int = 30
    ) -> String {
        guard let startComponents = parseTime(windowStart),
              let endComponents = parseTime(windowEnd) else {
            return "*/30 1-5 * * *" // Safe default
        }

        let startHour = startComponents.hour
        let endHour = endComponents.hour

        // Handle cross-midnight
        if startHour >= endHour {
            // Cross-midnight: use two ranges
            // Example: 23:00-05:00 → "*/30 23,0-5 * * *"
            return "*/\(interval) \(startHour),0-\(endHour) * * *"
        } else {
            // Normal range
            return "*/\(interval) \(startHour)-\(endHour) * * *"
        }
    }

    // MARK: - Private Helpers

    private static func parseTime(_ time: String) -> (hour: Int, minute: Int)? {
        let parts = time.split(separator: ":")
        guard parts.count == 2,
              let hour = Int(parts[0]),
              let minute = Int(parts[1]),
              hour >= 0, hour < 24,
              minute >= 0, minute < 60 else {
            return nil
        }
        return (hour, minute)
    }

    private static func formatTime(_ time: String) -> String {
        guard let components = parseTime(time) else { return time }

        let hour = components.hour
        let minute = components.minute

        // 12-hour format
        let period = hour >= 12 ? "PM" : "AM"
        let displayHour = hour == 0 ? 12 : (hour > 12 ? hour - 12 : hour)

        return String(format: "%d:%02d %@", displayHour, minute, period)
    }

    private static func isWithinWindow(
        date: Date,
        windowStart: (hour: Int, minute: Int),
        windowEnd: (hour: Int, minute: Int),
        calendar: Calendar
    ) -> Bool {
        let components = calendar.dateComponents([.hour, .minute], from: date)
        guard let hour = components.hour, let minute = components.minute else {
            return false
        }

        let currentMinutes = hour * 60 + minute
        let startMinutes = windowStart.hour * 60 + windowStart.minute
        let endMinutes = windowEnd.hour * 60 + windowEnd.minute

        // Cross-midnight case
        if startMinutes >= endMinutes {
            // Current time is either >= start OR <= end
            return currentMinutes >= startMinutes || currentMinutes <= endMinutes
        } else {
            // Normal case: current time is between start and end
            return currentMinutes >= startMinutes && currentMinutes <= endMinutes
        }
    }
}
