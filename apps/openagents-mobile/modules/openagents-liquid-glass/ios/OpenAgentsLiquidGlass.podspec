Pod::Spec.new do |s|
  s.name           = 'OpenAgentsLiquidGlass'
  s.version        = '0.1.0'
  s.summary        = 'OpenAgents SwiftUI Liquid Glass island Expo module.'
  s.description    = 'SwiftUI (iOS 26 Liquid Glass) island rendered inside the OpenAgents Effect Native mobile app as the SwiftUI renderer seam test.'
  s.author         = 'OpenAgents'
  s.homepage       = 'https://openagents.com'
  s.license        = { :type => 'MIT' }
  s.platforms      = { :ios => '15.1' }
  s.source         = { :git => 'https://github.com/OpenAgentsInc/openagents.git' }
  s.source_files   = '**/*.{h,m,mm,swift}'
  s.swift_version  = '5.0'

  s.dependency 'ExpoModulesCore'
end
