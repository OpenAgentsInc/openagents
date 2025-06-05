import { useState, useEffect } from 'react'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@openagentsinc/ui/web/components/card'
import { Button } from '@openagentsinc/ui/web/components/button'
import { Input } from '@openagentsinc/ui/web/components/input'
import { Label } from '@openagentsinc/ui/web/components/label'
import { Textarea } from '@openagentsinc/ui/web/components/textarea'
import { Badge } from '@openagentsinc/ui/web/components/badge'
import { Alert, AlertDescription } from '@openagentsinc/ui/web/components/alert'
import { ScrollArea } from '@openagentsinc/ui/web/components/scroll-area'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@openagentsinc/ui/web/components/tabs'
import { Effect, Stream, Duration, Layer } from 'effect'
import * as Nostr from '@openagentsinc/nostr'
import { Loader2, Send, Wifi, WifiOff, Key, Hash, User, Calendar } from 'lucide-react'

interface NostrEvent {
  id: string
  pubkey: string
  created_at: number
  kind: number
  content: string
  tags: string[][]
  sig: string
}

export function NostrDemo() {
  const [privateKey, setPrivateKey] = useState<string>('')
  const [publicKey, setPublicKey] = useState<string>('')
  const [relayUrl, setRelayUrl] = useState<string>('ws://localhost:7777')
  const [isConnected, setIsConnected] = useState(false)
  const [isConnecting, setIsConnecting] = useState(false)
  const [eventContent, setEventContent] = useState('')
  const [events, setEvents] = useState<NostrEvent[]>([])
  const [subscriptionId, setSubscriptionId] = useState<string>('')
  const [filterKind, setFilterKind] = useState<string>('')
  const [error, setError] = useState<string>('')
  
  // Service references
  const [connection, setConnection] = useState<any>(null)
  const [subscription, setSubscription] = useState<any>(null)

  // Generate new key pair
  const generateKeys = async () => {
    try {
      const program = Effect.gen(function* () {
        const crypto = yield* Nostr.CryptoService.CryptoService
        const privateKey = yield* crypto.generatePrivateKey()
        const publicKey = yield* crypto.getPublicKey(privateKey)
        return { privateKey, publicKey }
      }).pipe(
        Effect.provide(Nostr.CryptoService.CryptoServiceLive)
      )
      
      const { privateKey: priv, publicKey: pub } = await Effect.runPromise(program)
      setPrivateKey(priv)
      setPublicKey(pub)
      setError('')
    } catch (err) {
      setError(`Failed to generate keys: ${err}`)
    }
  }

  // Connect to relay
  const connectToRelay = async () => {
    try {
      setIsConnecting(true)
      setError('')
      
      const program = Effect.gen(function* () {
        const relayService = yield* Nostr.RelayService.RelayService
        const conn = yield* relayService.connect(relayUrl)
        return conn
      }).pipe(
        Effect.provide(Layer.mergeAll(
          Nostr.WebSocketService.WebSocketServiceLive,
          Nostr.RelayService.RelayServiceLive
        )),
        Effect.scoped
      )
      
      const conn = await Effect.runPromise(program)
      setConnection(conn)
      setIsConnected(true)
    } catch (err) {
      setError(`Failed to connect: ${err}`)
    } finally {
      setIsConnecting(false)
    }
  }

  // Disconnect from relay
  const disconnect = async () => {
    try {
      if (connection) {
        await Effect.runPromise(connection.close())
        setConnection(null)
        setIsConnected(false)
        setSubscription(null)
        setSubscriptionId('')
      }
    } catch (err) {
      setError(`Failed to disconnect: ${err}`)
    }
  }

  // Publish event
  const publishEvent = async () => {
    if (!connection || !privateKey || !eventContent) return
    
    try {
      const program = Effect.gen(function* () {
        const eventService = yield* Nostr.EventService.EventService
        const event = yield* eventService.create({
          kind: 1,
          content: eventContent,
          tags: []
        }, privateKey as any)
        
        const success = yield* connection.publish(event)
        return { event, success }
      }).pipe(
        Effect.provide(Layer.mergeAll(
          Nostr.CryptoService.CryptoServiceLive,
          Nostr.EventService.EventServiceLive
        ))
      )
      
      const { event, success } = await Effect.runPromise(program)
      if (success) {
        setEventContent('')
        // Add to local events
        setEvents(prev => [event as any, ...prev])
      } else {
        setError('Failed to publish event')
      }
    } catch (err) {
      setError(`Failed to publish: ${err}`)
    }
  }

  // Subscribe to events
  const subscribe = async () => {
    if (!connection) return
    
    try {
      // Unsubscribe from previous subscription
      if (subscription && subscriptionId) {
        await Effect.runPromise(connection.unsubscribe(subscriptionId))
      }
      
      const subId = `sub-${Date.now()}`
      const filters: any[] = [{}]
      
      if (filterKind) {
        filters[0].kinds = [parseInt(filterKind)]
      }
      
      const sub = await Effect.runPromise(
        connection.subscribe(subId, filters)
      )
      
      setSubscription(sub)
      setSubscriptionId(subId)
      
      // Process incoming events
      Effect.runPromise(
        sub.events.pipe(
          Stream.tap((event: NostrEvent) => {
            setEvents(prev => {
              // Avoid duplicates
              if (prev.some(e => e.id === event.id)) return prev
              return [event, ...prev].slice(0, 50) // Keep last 50
            })
          }),
          Stream.runDrain
        )
      ).catch(err => {
        console.error('Subscription error:', err)
      })
    } catch (err) {
      setError(`Failed to subscribe: ${err}`)
    }
  }

  // Format timestamp
  const formatTime = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString()
  }

  // Truncate long strings
  const truncate = (str: string, len: number) => {
    if (str.length <= len) return str
    return str.slice(0, len) + '...'
  }

  return (
    <Card className="w-full">
      <CardHeader>
        <CardTitle>Nostr Protocol Demo</CardTitle>
        <CardDescription>
          Test the Effect-based Nostr implementation
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
              
              {isConnected && (
                <div className="space-y-4 pt-4 border-t">
                  <h4 className="font-medium">Subscription Settings</h4>
                  
                  <div className="space-y-2">
                    <Label htmlFor="filterKind">Filter by Kind (optional)</Label>
                    <Input
                      id="filterKind"
                      type="number"
                      value={filterKind}
                      onChange={(e) => setFilterKind(e.target.value)}
                      placeholder="e.g., 1 for text notes"
                    />
                  </div>
                  
                  <Button onClick={subscribe} className="w-full">
                    {subscriptionId ? 'Update Subscription' : 'Subscribe to Events'}
                  </Button>
                  
                  {subscriptionId && (
                    <Alert>
                      <AlertDescription>
                        Active subscription: {subscriptionId}
                      </AlertDescription>
                    </Alert>
                  )}
                </div>
              )}
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
                    No events yet. Subscribe to a relay to see events.
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