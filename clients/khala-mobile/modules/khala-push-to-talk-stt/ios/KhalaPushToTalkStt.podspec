Pod::Spec.new do |s|
  s.name           = 'KhalaPushToTalkStt'
  s.version        = '0.1.0'
  s.summary        = 'Khala push-to-talk speech recognition Expo module.'
  s.description    = 'Expo module shell for Khala push-to-talk STT readiness and device proof.'
  s.author         = 'OpenAgents'
  s.homepage       = 'https://openagents.com'
  s.license        = { :type => 'MIT' }
  s.platforms      = { :ios => '15.1' }
  s.source         = { :git => 'https://github.com/OpenAgentsInc/openagents.git' }
  s.source_files   = '**/*.{h,m,mm,swift}'
  s.swift_version  = '5.0'

  s.dependency 'ExpoModulesCore'
end
