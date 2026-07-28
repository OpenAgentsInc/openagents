Pod::Spec.new do |s|
  s.name             = 'ExpoRealtimeAudio'
  s.version          = '0.1.0'
  s.summary          = 'Bounded streaming PCM playback for OpenAgents mobile voice.'
  s.description      = 'A local Expo module that plays validated mono PCM16 speech frames.'
  s.author           = 'OpenAgents, Inc.'
  s.homepage         = 'https://openagents.com'
  s.platforms        = { :ios => '16.4' }
  s.source           = { :path => '.' }
  s.static_framework = true
  s.dependency 'ExpoModulesCore'
  s.pod_target_xcconfig = { 'DEFINES_MODULE' => 'YES' }
  s.source_files = '**/*.{h,m,mm,swift,hpp,cpp}'
end
