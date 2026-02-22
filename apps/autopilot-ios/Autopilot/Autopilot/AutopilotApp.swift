//
//  AutopilotApp.swift
//  Autopilot
//
//  Created by Christopher David on 2/19/26.
//

import SwiftUI

@main
struct AutopilotApp: App {
    init() {
        applyOpenAgentsDarkTheme()
    }

    var body: some Scene {
        WindowGroup {
            WgpuiCodexRootView()
        }
    }

    private func applyOpenAgentsDarkTheme() {
        let background = UIColor(red: 16 / 255, green: 16 / 255, blue: 17 / 255, alpha: 1)
        let card = UIColor(red: 26 / 255, green: 26 / 255, blue: 26 / 255, alpha: 1)
        let foreground = UIColor(red: 216 / 255, green: 222 / 255, blue: 233 / 255, alpha: 1)
        let mutedForeground = UIColor(red: 153 / 255, green: 153 / 255, blue: 153 / 255, alpha: 1)
        let ring = UIColor(red: 136 / 255, green: 192 / 255, blue: 208 / 255, alpha: 1)
        let sidebar = UIColor(red: 9 / 255, green: 9 / 255, blue: 9 / 255, alpha: 1)

        let navigationAppearance = UINavigationBarAppearance()
        navigationAppearance.configureWithOpaqueBackground()
        navigationAppearance.backgroundColor = background
        navigationAppearance.titleTextAttributes = [.foregroundColor: foreground]
        navigationAppearance.largeTitleTextAttributes = [.foregroundColor: foreground]
        UINavigationBar.appearance().standardAppearance = navigationAppearance
        UINavigationBar.appearance().compactAppearance = navigationAppearance
        UINavigationBar.appearance().scrollEdgeAppearance = navigationAppearance
        UINavigationBar.appearance().tintColor = ring

        let tabAppearance = UITabBarAppearance()
        tabAppearance.configureWithOpaqueBackground()
        tabAppearance.backgroundColor = sidebar
        tabAppearance.stackedLayoutAppearance.normal.iconColor = mutedForeground
        tabAppearance.stackedLayoutAppearance.normal.titleTextAttributes = [.foregroundColor: mutedForeground]
        tabAppearance.stackedLayoutAppearance.selected.iconColor = ring
        tabAppearance.stackedLayoutAppearance.selected.titleTextAttributes = [.foregroundColor: ring]
        UITabBar.appearance().standardAppearance = tabAppearance
        UITabBar.appearance().scrollEdgeAppearance = tabAppearance
        UITabBar.appearance().tintColor = ring
        UITabBar.appearance().unselectedItemTintColor = mutedForeground

        UITableView.appearance().backgroundColor = background
        UITableViewCell.appearance().backgroundColor = card
    }
}
