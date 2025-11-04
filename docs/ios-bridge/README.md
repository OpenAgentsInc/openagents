# LAN WebSocket Bridge Between Desktop and iOS

This document explains the LAN WebSocket bridge architecture that enables communication between a desktop server and an iOS client over a local network. It covers the protocol details, authentication, setup and running instructions, discovery methods, security considerations, and testing.

---

## Overview

The LAN WebSocket bridge allows an iOS app to connect to a desktop application over a local network using WebSockets. This enables real-time bi-directional communication without relying on internet connectivity.

- **Desktop** runs a WebSocket server.
- **iOS** acts as a WebSocket client.
- Communication occurs over LAN IP addresses.
- Authentication is done via a pairing token exchanged through a QR code.
- Support for future enhancements like Bonjour/mDNS discovery.

---

## Protocol

### Connection Setup

1. **Hello/Ack Exchange**
   Upon connection, the client sends a `"hello"` message with a client identifier and version.
   The server responds with an `"ack"` message confirming readiness.

   ```json
   // Client -> Server
   { "type": "hello", "clientID": "iOSDevice123", "version": "1.0" }

   // Server -> Client
   { "type": "ack", "message": "Welcome" }
