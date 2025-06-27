import type { Meta, StoryObj } from '@storybook/nextjs'
import { 
  AnimatorGeneralProvider,
  Animator,
  Animated,
  Text,
  FrameCorners,
  FrameLines,
  FrameUnderline,
  FrameOctagon,
  GridLines
} from '@arwes/react'
import React, { useState, useEffect } from 'react'
import { 
  Settings, User, Bot, Key, Bell, Palette, 
  Volume2, Shield, Database, Zap, Eye,
  Monitor, Moon, Sun, Globe, Save, 
  RefreshCw, Download, Upload, Trash2
} from 'lucide-react'

const meta = {
  title: 'Features/Chat & AI/ChatSettings',
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Settings interface for AI chat applications with model selection, API configuration, themes, and preferences.',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta

export default meta
type Story = StoryObj

// Settings section component
const SettingsSection = ({ 
  title, 
  icon: Icon, 
  children 
}: { 
  title: string
  icon: React.ComponentType<{ size: number, className?: string }>
  children: React.ReactNode 
}) => {
  return (
    <Animator>
      <Animated animated={[['y', 20, 0], ['opacity', 0, 1]]}>
        <div className="relative mb-8">
          <FrameUnderline
            style={{
              // @ts-expect-error css variables
              '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
              '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.6)',
            }}
          />
          <div className="relative p-6">
            <div className="flex items-center gap-3 mb-4">
              <Icon size={20} className="text-cyan-400" />
              <Text as="h3" className="text-xl text-cyan-300">
                {title}
              </Text>
            </div>
            {children}
          </div>
        </div>
      </Animated>
    </Animator>
  )
}

// Settings row component
const SettingsRow = ({ 
  label, 
  description, 
  children 
}: { 
  label: string
  description?: string
  children: React.ReactNode 
}) => {
  return (
    <div className="flex items-center justify-between py-3 border-b border-cyan-500/10 last:border-b-0">
      <div className="flex-1">
        <Text className="text-cyan-300">{label}</Text>
        {description && (
          <Text className="text-cyan-500 text-sm">{description}</Text>
        )}
      </div>
      <div className="ml-4">
        {children}
      </div>
    </div>
  )
}

export const GeneralSettings: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [settings, setSettings] = useState({
      theme: 'dark',
      language: 'en',
      model: 'gpt-4',
      temperature: 0.7,
      maxTokens: 2048,
      streaming: true,
      soundEffects: true,
      notifications: true,
      autoSave: true
    })
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    const updateSetting = (key: string, value: any) => {
      setSettings(prev => ({ ...prev, [key]: value }))
    }
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            {/* Background */}
            <div className="fixed inset-0">
              <GridLines lineColor="hsla(180, 100%, 75%, 0.02)" distance={40} />
            </div>
            
            <div className="relative z-10 max-w-4xl mx-auto">
              {/* Header */}
              <div className="mb-8">
                <Text as="h1" className="text-3xl text-cyan-300 mb-2">
                  Settings
                </Text>
                <Text className="text-cyan-500">
                  Configure your AI chat experience
                </Text>
              </div>
              
              <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
                {/* Sidebar navigation */}
                <div className="lg:col-span-1">
                  <Animator>
                    <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                      <div className="relative">
                        <FrameLines
                          style={{
                            // @ts-expect-error css variables
                            '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.2)',
                            '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.4)',
                          }}
                        />
                        <div className="relative p-4">
                          <Text className="text-cyan-400 text-sm mb-3">Categories</Text>
                          <nav className="space-y-1">
                            {[
                              { icon: Settings, label: 'General', active: true },
                              { icon: Bot, label: 'AI Models' },
                              { icon: Key, label: 'API Keys' },
                              { icon: Bell, label: 'Notifications' },
                              { icon: Palette, label: 'Appearance' },
                              { icon: Shield, label: 'Privacy' },
                            ].map((item) => {
                              const Icon = item.icon
                              return (
                                <button
                                  key={item.label}
                                  className={`w-full flex items-center gap-3 px-3 py-2 text-left transition-colors ${
                                    item.active 
                                      ? 'bg-cyan-500/20 text-cyan-300 border-l-2 border-cyan-500' 
                                      : 'text-cyan-500 hover:bg-cyan-500/10'
                                  }`}
                                >
                                  <Icon size={16} />
                                  <Text>{item.label}</Text>
                                </button>
                              )
                            })}
                          </nav>
                        </div>
                      </div>
                    </Animated>
                  </Animator>
                </div>
                
                {/* Settings content */}
                <div className="lg:col-span-2">
                  <div className="space-y-6">
                    {/* Model Settings */}
                    <SettingsSection title="AI Model" icon={Bot}>
                      <div className="space-y-4">
                        <SettingsRow 
                          label="Default Model" 
                          description="Choose your preferred AI model"
                        >
                          <select 
                            value={settings.model}
                            onChange={(e) => updateSetting('model', e.target.value)}
                            className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-3 py-2"
                          >
                            <option value="gpt-4">GPT-4</option>
                            <option value="gpt-3.5-turbo">GPT-3.5 Turbo</option>
                            <option value="claude-3">Claude 3</option>
                            <option value="claude-2">Claude 2</option>
                          </select>
                        </SettingsRow>
                        
                        <SettingsRow 
                          label="Temperature" 
                          description="Controls randomness in responses (0.0 - 1.0)"
                        >
                          <div className="flex items-center gap-3">
                            <input
                              type="range"
                              min="0"
                              max="1"
                              step="0.1"
                              value={settings.temperature}
                              onChange={(e) => updateSetting('temperature', parseFloat(e.target.value))}
                              className="w-24"
                            />
                            <Text className="text-cyan-300 w-8">{settings.temperature}</Text>
                          </div>
                        </SettingsRow>
                        
                        <SettingsRow 
                          label="Max Tokens" 
                          description="Maximum response length"
                        >
                          <input
                            type="number"
                            value={settings.maxTokens}
                            onChange={(e) => updateSetting('maxTokens', parseInt(e.target.value))}
                            className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-3 py-2 w-24"
                          />
                        </SettingsRow>
                      </div>
                    </SettingsSection>
                    
                    {/* Interface Settings */}
                    <SettingsSection title="Interface" icon={Monitor}>
                      <div className="space-y-4">
                        <SettingsRow 
                          label="Theme" 
                          description="Choose your visual theme"
                        >
                          <div className="flex items-center gap-2">
                            {[
                              { value: 'dark', icon: Moon, label: 'Dark' },
                              { value: 'light', icon: Sun, label: 'Light' },
                              { value: 'auto', icon: Monitor, label: 'Auto' },
                            ].map((theme) => {
                              const Icon = theme.icon
                              return (
                                <button
                                  key={theme.value}
                                  onClick={() => updateSetting('theme', theme.value)}
                                  className={`flex items-center gap-2 px-3 py-2 border ${
                                    settings.theme === theme.value
                                      ? 'bg-cyan-500/20 text-cyan-300 border-cyan-500/50'
                                      : 'text-cyan-500 border-cyan-500/30 hover:bg-cyan-500/10'
                                  }`}
                                >
                                  <Icon size={16} />
                                  <Text className="text-sm">{theme.label}</Text>
                                </button>
                              )
                            })}
                          </div>
                        </SettingsRow>
                        
                        <SettingsRow 
                          label="Streaming Responses" 
                          description="Show responses as they are generated"
                        >
                          <button
                            onClick={() => updateSetting('streaming', !settings.streaming)}
                            className={`relative w-12 h-6 rounded-full border-2 transition-colors ${
                              settings.streaming 
                                ? 'bg-cyan-500/20 border-cyan-500' 
                                : 'bg-gray-500/20 border-gray-500'
                            }`}
                          >
                            <div 
                              className={`absolute w-4 h-4 rounded-full transition-transform ${
                                settings.streaming 
                                  ? 'translate-x-6 bg-cyan-500' 
                                  : 'translate-x-1 bg-gray-500'
                              }`} 
                            />
                          </button>
                        </SettingsRow>
                        
                        <SettingsRow 
                          label="Sound Effects" 
                          description="Play sounds for interactions"
                        >
                          <button
                            onClick={() => updateSetting('soundEffects', !settings.soundEffects)}
                            className={`relative w-12 h-6 rounded-full border-2 transition-colors ${
                              settings.soundEffects 
                                ? 'bg-cyan-500/20 border-cyan-500' 
                                : 'bg-gray-500/20 border-gray-500'
                            }`}
                          >
                            <div 
                              className={`absolute w-4 h-4 rounded-full transition-transform ${
                                settings.soundEffects 
                                  ? 'translate-x-6 bg-cyan-500' 
                                  : 'translate-x-1 bg-gray-500'
                              }`} 
                            />
                          </button>
                        </SettingsRow>
                      </div>
                    </SettingsSection>
                    
                    {/* Save button */}
                    <Animator>
                      <Animated animated={[['scale', 0.95, 1], ['opacity', 0, 1]]}>
                        <div className="flex justify-end gap-3">
                          <button className="px-6 py-3 text-cyan-500 border border-cyan-500/30 hover:bg-cyan-500/10">
                            <Text>Reset</Text>
                          </button>
                          <button className="px-6 py-3 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30 flex items-center gap-2">
                            <Save size={16} />
                            <Text>Save Changes</Text>
                          </button>
                        </div>
                      </Animated>
                    </Animator>
                  </div>
                </div>
              </div>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const APIConfiguration: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    const [apiKeys, setApiKeys] = useState([
      { id: '1', provider: 'OpenAI', key: 'sk-...abc123', status: 'active', usage: '2.4K' },
      { id: '2', provider: 'Anthropic', key: 'claude-...xyz789', status: 'active', usage: '1.8K' },
      { id: '3', provider: 'Cohere', key: 'co-...def456', status: 'inactive', usage: '0' },
    ])
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                API Configuration
              </Text>
              
              {/* API Keys section */}
              <SettingsSection title="API Keys" icon={Key}>
                <div className="space-y-4">
                  {/* Add new key */}
                  <div className="relative">
                    <FrameCorners
                      style={{
                        // @ts-expect-error css variables
                        '--arwes-frames-bg-color': 'hsla(180, 75%, 10%, 0.1)',
                        '--arwes-frames-line-color': 'hsla(180, 75%, 50%, 0.3)',
                      }}
                    />
                    <div className="relative p-4">
                      <Text className="text-cyan-400 mb-3">Add New API Key</Text>
                      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                        <select className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-3 py-2">
                          <option>Select Provider</option>
                          <option>OpenAI</option>
                          <option>Anthropic</option>
                          <option>Cohere</option>
                          <option>Hugging Face</option>
                        </select>
                        <input
                          type="password"
                          placeholder="API Key"
                          className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 placeholder-cyan-600 px-3 py-2"
                        />
                        <button className="px-4 py-2 bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 hover:bg-cyan-500/30">
                          <Text>Add Key</Text>
                        </button>
                      </div>
                    </div>
                  </div>
                  
                  {/* Existing keys */}
                  <div className="space-y-3">
                    <Animator manager="stagger" duration={{ stagger: 0.1 }}>
                      {apiKeys.map((key) => (
                        <Animator key={key.id}>
                          <Animated animated={[['x', -20, 0], ['opacity', 0, 1]]}>
                            <div className="flex items-center justify-between p-4 bg-cyan-500/5 border border-cyan-500/20">
                              <div className="flex items-center gap-4">
                                <div>
                                  <Text className="text-cyan-300 font-semibold">{key.provider}</Text>
                                  <Text className="text-cyan-500 text-sm font-mono">{key.key}</Text>
                                </div>
                                <div className={`px-2 py-1 text-xs ${
                                  key.status === 'active' 
                                    ? 'bg-green-500/20 text-green-300' 
                                    : 'bg-gray-500/20 text-gray-400'
                                }`}>
                                  {key.status}
                                </div>
                              </div>
                              <div className="flex items-center gap-4">
                                <div className="text-right">
                                  <Text className="text-cyan-300 text-sm">{key.usage}</Text>
                                  <Text className="text-cyan-600 text-xs">requests</Text>
                                </div>
                                <div className="flex items-center gap-2">
                                  <button className="text-cyan-500 hover:text-cyan-300 p-1">
                                    <Eye size={16} />
                                  </button>
                                  <button className="text-cyan-500 hover:text-cyan-300 p-1">
                                    <RefreshCw size={16} />
                                  </button>
                                  <button className="text-red-500 hover:text-red-300 p-1">
                                    <Trash2 size={16} />
                                  </button>
                                </div>
                              </div>
                            </div>
                          </Animated>
                        </Animator>
                      ))}
                    </Animator>
                  </div>
                </div>
              </SettingsSection>
              
              {/* Rate Limits */}
              <SettingsSection title="Rate Limits" icon={Zap}>
                <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                  {[
                    { provider: 'OpenAI', limit: '10,000', used: '2,400', percentage: 24 },
                    { provider: 'Anthropic', limit: '5,000', used: '1,800', percentage: 36 },
                    { provider: 'Cohere', limit: '1,000', used: '0', percentage: 0 },
                  ].map((limit) => (
                    <Animator key={limit.provider}>
                      <Animated animated={[['scale', 0.9, 1], ['opacity', 0, 1]]}>
                        <div className="p-4 bg-cyan-500/5 border border-cyan-500/20">
                          <Text className="text-cyan-300 font-semibold mb-2">{limit.provider}</Text>
                          <div className="space-y-2">
                            <div className="flex justify-between text-sm">
                              <Text className="text-cyan-500">Used: {limit.used}</Text>
                              <Text className="text-cyan-600">Limit: {limit.limit}</Text>
                            </div>
                            <div className="w-full h-2 bg-cyan-500/20 rounded-full overflow-hidden">
                              <div 
                                className="h-full bg-cyan-500 transition-all duration-1000"
                                style={{ width: `${limit.percentage}%` }}
                              />
                            </div>
                          </div>
                        </div>
                      </Animated>
                    </Animator>
                  ))}
                </div>
              </SettingsSection>
              
              {/* Usage Analytics */}
              <SettingsSection title="Usage Analytics" icon={Database}>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                  {[
                    { label: 'Requests Today', value: '147', color: 'cyan' },
                    { label: 'Tokens Used', value: '23.5K', color: 'purple' },
                    { label: 'Cost This Month', value: '$4.32', color: 'green' },
                    { label: 'Avg Response Time', value: '1.2s', color: 'yellow' },
                  ].map((stat) => (
                    <Animator key={stat.label}>
                      <Animated animated={[['y', 10, 0], ['opacity', 0, 1]]}>
                        <div className="text-center p-4 bg-cyan-500/5 border border-cyan-500/20">
                          <Text className={`text-2xl text-${stat.color}-300 font-bold`}>
                            {stat.value}
                          </Text>
                          <Text className={`text-${stat.color}-500 text-sm`}>
                            {stat.label}
                          </Text>
                        </div>
                      </Animated>
                    </Animator>
                  ))}
                </div>
              </SettingsSection>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}

export const AdvancedSettings: Story = {
  render: () => {
    const [active, setActive] = useState(false)
    
    useEffect(() => {
      const timer = setTimeout(() => setActive(true), 100)
      return () => clearTimeout(timer)
    }, [])
    
    return (
      <div className="min-h-screen bg-black p-4">
        <AnimatorGeneralProvider duration={{ enter: 0.5, exit: 0.3 }}>
          <Animator active={active}>
            <div className="max-w-4xl mx-auto">
              <Text as="h2" className="text-2xl text-cyan-300 mb-6">
                Advanced Settings
              </Text>
              
              {/* Data Management */}
              <SettingsSection title="Data Management" icon={Database}>
                <div className="space-y-4">
                  <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                    <button className="flex flex-col items-center gap-2 p-6 bg-cyan-500/5 border border-cyan-500/20 hover:bg-cyan-500/10">
                      <Download size={24} className="text-cyan-400" />
                      <Text className="text-cyan-300">Export Data</Text>
                      <Text className="text-cyan-500 text-sm text-center">
                        Download all your conversations
                      </Text>
                    </button>
                    
                    <button className="flex flex-col items-center gap-2 p-6 bg-cyan-500/5 border border-cyan-500/20 hover:bg-cyan-500/10">
                      <Upload size={24} className="text-cyan-400" />
                      <Text className="text-cyan-300">Import Data</Text>
                      <Text className="text-cyan-500 text-sm text-center">
                        Restore from backup
                      </Text>
                    </button>
                    
                    <button className="flex flex-col items-center gap-2 p-6 bg-red-500/5 border border-red-500/20 hover:bg-red-500/10">
                      <Trash2 size={24} className="text-red-400" />
                      <Text className="text-red-300">Clear All Data</Text>
                      <Text className="text-red-500 text-sm text-center">
                        Permanently delete everything
                      </Text>
                    </button>
                  </div>
                </div>
              </SettingsSection>
              
              {/* Privacy Settings */}
              <SettingsSection title="Privacy & Security" icon={Shield}>
                <div className="space-y-4">
                  <SettingsRow 
                    label="Data Retention" 
                    description="How long to keep conversation history"
                  >
                    <select className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-3 py-2">
                      <option>30 days</option>
                      <option>90 days</option>
                      <option>1 year</option>
                      <option>Forever</option>
                    </select>
                  </SettingsRow>
                  
                  <SettingsRow 
                    label="Anonymous Usage" 
                    description="Allow anonymous analytics to improve the service"
                  >
                    <input type="checkbox" className="text-cyan-500" defaultChecked />
                  </SettingsRow>
                  
                  <SettingsRow 
                    label="Data Processing Location" 
                    description="Where your data is processed"
                  >
                    <select className="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-3 py-2">
                      <option>Auto (Nearest)</option>
                      <option>US East</option>
                      <option>EU West</option>
                      <option>Asia Pacific</option>
                    </select>
                  </SettingsRow>
                </div>
              </SettingsSection>
              
              {/* Experimental Features */}
              <SettingsSection title="Experimental Features" icon={Zap}>
                <div className="space-y-4">
                  <div className="p-4 bg-yellow-500/10 border border-yellow-500/30">
                    <Text className="text-yellow-300 font-semibold mb-2">
                      ⚠️ Experimental Features
                    </Text>
                    <Text className="text-yellow-500 text-sm">
                      These features are in beta and may not work as expected. Use at your own risk.
                    </Text>
                  </div>
                  
                  <SettingsRow 
                    label="WebRTC Voice Chat" 
                    description="Real-time voice conversations (Beta)"
                  >
                    <input type="checkbox" className="text-cyan-500" />
                  </SettingsRow>
                  
                  <SettingsRow 
                    label="Code Execution" 
                    description="Allow AI to run code in a sandboxed environment"
                  >
                    <input type="checkbox" className="text-cyan-500" />
                  </SettingsRow>
                  
                  <SettingsRow 
                    label="Memory Persistence" 
                    description="AI remembers context across sessions"
                  >
                    <input type="checkbox" className="text-cyan-500" defaultChecked />
                  </SettingsRow>
                </div>
              </SettingsSection>
            </div>
          </Animator>
        </AnimatorGeneralProvider>
      </div>
    )
  },
}