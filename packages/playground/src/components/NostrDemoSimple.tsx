import { useState } from 'react'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Textarea } from '@openagentsinc/ui/web/components/textarea'
import { Badge } from '@openagentsinc/ui/web/components/badge'
import { Alert, AlertDescription } from '@openagentsinc/ui/web/components/alert'
import { ScrollArea } from '@openagentsinc/ui/web/components/scroll-area'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@openagentsinc/ui/web/components/tabs'
import { Loader2, Send, Wifi, WifiOff, Key, Hash, User, Calendar } from 'lucide-react'

// Simplified Nostr demo without the actual Nostr package imports
export function NostrDemoSimple() {
  const [privateKey, setPrivateKey] = useState<string>('')
  const [publicKey, setPublicKey] = useState<string>('')
  const [relayUrl, setRelayUrl] = useState<string>('ws://localhost:7777')
  const [isConnected, setIsConnected] = useState(false)
  const [isConnecting, setIsConnecting] = useState(false)
  const [eventContent, setEventContent] = useState('')
  const [events, setEvents] = useState<any[]>([])
  const [error, setError] = useState<string>('')

  // Placeholder functions
  const generateKeys = () => {
    // Mock key generation
    const mockPrivateKey = Array(64).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join('')
    const mockPublicKey = Array(64).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join('')
    setPrivateKey(mockPrivateKey)
    setPublicKey(mockPublicKey)
    setError('')
  }

  const connectToRelay = async () => {
    setIsConnecting(true)
    setError('')
    
    // Simulate connection
    setTimeout(() => {
      setIsConnected(true)
      setIsConnecting(false)
    }, 1000)
  }

  const disconnect = () => {
    setIsConnected(false)
  }

  const publishEvent = () => {
    if (!eventContent) return
    
    const mockEvent = {
      id: Array(64).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join(''),
      pubkey: publicKey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 1,
      content: eventContent,
      tags: [],
      sig: Array(128).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join('')
    }
    
    setEvents(prev => [mockEvent, ...prev])
    setEventContent('')
  }

  // Format timestamp
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString()
  }

  // Truncate long strings
  const truncate = (str: string, len: number) => {
    if (!str) return ''
    if (str.length <= len) return str
    return str.slice(0, len) + '...'
  }

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Nostr Protocol Demo (Preview)</CardTitle>
        <CardDescription>
          Preview of the Nostr implementation UI (without actual protocol)
        </CardDescription>
      </CardHeader>
      <CardContent>
        <Tabs defaultValue="keys" className="w-full">
          <TabsList className="grid w-full grid-cols-4">
            <TabsTrigger value="keys">Keys</TabsTrigger>
            <TabsTrigger value="relay">Relay</TabsTrigger>
            <TabsTrigger value="publish">Publish</TabsTrigger>
            <TabsTrigger value="events">Events</TabsTrigger>
          </TabsList>
          
          <TabsContent value="keys" className="space-y-4">
            <div className="space-y-4">
              <Button onClick={generateKeys} className="w-full">
                <Key className="mr-2 h-4 w-4" />
                Generate New Key Pair
              </Button>
              
              <div className="space-y-2">
                <Label htmlFor="privateKey">Private Key</Label>
                <Input
                  id="privateKey"
                  type="password"
                  value={privateKey}
                  onChange={(e) => setPrivateKey(e.target.value)}
                  placeholder="Your private key (keep secret!)"
                />
              </div>
              
              <div className="space-y-2">
                <Label htmlFor="publicKey">Public Key</Label>
                <Input
                  id="publicKey"
                  value={publicKey}
                  readOnly
                  placeholder="Your public key"
                />
              </div>
            </div>
          </TabsContent>
          
          <TabsContent value="relay" className="space-y-4">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="relayUrl">Relay URL</Label>
                <Input
                  id="relayUrl"
                  value={relayUrl}
                  onChange={(e) => setRelayUrl(e.target.value)}
                  placeholder="ws://localhost:7777"
                  disabled={isConnected}
                />
              </div>
              
              <div className="flex items-center space-x-2">
                <Button
                  onClick={isConnected ? disconnect : connectToRelay}
                  disabled={isConnecting}
                  variant={isConnected ? "destructive" : "default"}
                  className="flex-1"
                >
                  {isConnecting ? (
                    <>
                      <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                      Connecting...
                    </>
                  ) : isConnected ? (
                    <>
                      <WifiOff className="mr-2 h-4 w-4" />
                      Disconnect
                    </>
                  ) : (
                    <>
                      <Wifi className="mr-2 h-4 w-4" />
                      Connect
                    </>
                  )}
                </Button>
                
                <Badge variant={isConnected ? "default" : "secondary"}>
                  {isConnected ? "Connected" : "Disconnected"}
                </Badge>
              </div>
            </div>
          </TabsContent>
          
          <TabsContent value="publish" className="space-y-4">
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="eventContent">Event Content</Label>
                <Textarea
                  id="eventContent"
                  value={eventContent}
                  onChange={(e) => setEventContent(e.target.value)}
                  placeholder="Enter your message..."
                  rows={4}
                  disabled={!isConnected || !privateKey}
                />
              </div>
              
              <Button
                onClick={publishEvent}
                disabled={!isConnected || !privateKey || !eventContent}
                className="w-full"
              >
                <Send className="mr-2 h-4 w-4" />
                Publish Event
              </Button>
              
              {(!isConnected || !privateKey) && (
                <Alert>
                  <AlertDescription>
                    {!privateKey
                      ? "Generate or enter a private key first"
                      : "Connect to a relay first"}
                  </AlertDescription>
                </Alert>
              )}
            </div>
          </TabsContent>
          
          <TabsContent value="events" className="space-y-4">
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h4 className="font-medium">Recent Events</h4>
                <Badge>{events.length} events</Badge>
              </div>
              
              <ScrollArea className="h-[400px] w-full rounded-md border p-4">
                {events.length === 0 ? (
                  <div className="text-center text-muted-foreground py-8">
                    No events yet. Connect and publish to see events.
                  </div>
                ) : (
                  <div className="space-y-4">
                    {events.map((event) => (
                      <Card key={event.id} className="p-4">
                        <div className="space-y-2">
                          <div className="flex items-start justify-between">
                            <div className="space-y-1">
                              <div className="flex items-center gap-2">
                                <Hash className="h-3 w-3 text-muted-foreground" />
                                <code className="text-xs">{truncate(event.id, 16)}</code>
                              </div>
                              <div className="flex items-center gap-2">
                                <User className="h-3 w-3 text-muted-foreground" />
                                <code className="text-xs">{truncate(event.pubkey, 16)}</code>
                              </div>
                            </div>
                            <Badge variant="outline">Kind {event.kind}</Badge>
                          </div>
                          
                          <p className="text-sm">{event.content}</p>
                          
                          <div className="flex items-center gap-2 text-xs text-muted-foreground">
                            <Calendar className="h-3 w-3" />
                            {formatTime(event.created_at)}
                          </div>
                        </div>
                      </Card>
                    ))}
                  </div>
                )}
              </ScrollArea>
            </div>
          </TabsContent>
        </Tabs>
        
        {error && (
          <Alert variant="destructive" className="mt-4">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
      </CardContent>
    </Card>
  )
}