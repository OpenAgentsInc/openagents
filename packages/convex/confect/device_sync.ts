import { Effect, Option } from "effect";
import {
  ConfectMutationCtx,
  ConfectQueryCtx,
  mutation,
  query,
} from "./confect";
import {
  ConnectDeviceArgs,
  ConnectDeviceResult,
  DeviceHeartbeatArgs,
  DeviceHeartbeatResult,
  GetUserDevicesArgs,
  GetUserDevicesResult,
  DisconnectDeviceArgs,
  DisconnectDeviceResult,
  UpdateDeviceStatusArgs,
  UpdateDeviceStatusResult,
  CreatePresenceRoomArgs,
  CreatePresenceRoomResult,
} from "./device_sync.schemas";

// Constants
const HEARTBEAT_TIMEOUT = 30000; // 30 seconds

// Helper function to get authenticated user with Effect-TS patterns (for mutations)
const getAuthenticatedUserEffectMutation = Effect.gen(function* () {
  const { db, auth } = yield* ConfectMutationCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new Error("Not authenticated"));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  
  const user = yield* db
    .query("users")
    .withIndex("by_openauth_subject", (q) => q.eq("openAuthSubject", authSubject))
    .first();

  return yield* Option.match(user, {
    onSome: (u) => Effect.succeed(u),
    onNone: () => 
      // Fallback: try looking up by GitHub ID (for backwards compatibility)
      Effect.gen(function* () {
        const fallbackUser = yield* db
          .query("users")
          .withIndex("by_github_id", (q) => q.eq("githubId", authSubject))
          .first();
        
        return yield* Option.match(fallbackUser, {
          onSome: (u) => Effect.succeed(u),
          onNone: () => Effect.fail(new Error("User not found"))
        });
      })
  });
});


// Connect device to user's presence room
export const connectDevice = mutation({
  args: ConnectDeviceArgs,
  returns: ConnectDeviceResult,
  handler: ({ deviceInfo }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`🔗 [DEVICE-SYNC] Connecting device for user: ${user._id}`);

      // Generate unique identifiers
      const deviceId = `device_${Date.now()}_${Math.random().toString(36).substring(2)}`;
      const sessionToken = `session_${Date.now()}_${Math.random().toString(36).substring(2)}`;
      const roomToken = `room_${user._id}`;

      // Create or update presence room
      const existingRoom = yield* db
        .query("devicePresenceRooms")
        .withIndex("by_room_id", (q) => q.eq("roomId", roomToken))
        .first();

      if (Option.isNone(existingRoom)) {
        // Create new presence room
        yield* db.insert("devicePresenceRooms", {
          roomId: roomToken,
          createdAt: Date.now(),
          updatedAt: Date.now(),
          activeDevices: [deviceId],
        });
        yield* Effect.logInfo(`📡 [DEVICE-SYNC] Created presence room: ${roomToken}`);
      } else {
        // Update room with new device
        const currentDevices = existingRoom.value.activeDevices || [];
        yield* db.patch(existingRoom.value._id, {
          updatedAt: Date.now(),
          activeDevices: [...currentDevices, deviceId],
        });
      }

      // Create device connection record
      yield* db.insert("deviceConnections", {
        deviceId,
        userId: user._id,
        deviceType: deviceInfo.deviceType,
        platform: deviceInfo.platform,
        appVersion: deviceInfo.appVersion,
        userAgent: deviceInfo.userAgent,
        status: "online",
        sessionToken,
        roomToken,
        connectedAt: Date.now(),
        lastHeartbeat: Date.now(),
        capabilities: deviceInfo.capabilities,
      });

      yield* Effect.logInfo(`✅ [DEVICE-SYNC] Device connected: ${deviceId}`);

      return { deviceId, sessionToken, roomToken };
    }),
});

// Send heartbeat to keep connection alive
export const deviceHeartbeat = mutation({
  args: DeviceHeartbeatArgs,
  returns: DeviceHeartbeatResult,
  handler: ({ deviceId, sessionToken }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`💓 [DEVICE-SYNC] Processing heartbeat for device: ${deviceId}`);

      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .filter((q) => q.eq(q.field("deviceId"), deviceId))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) =>
          Effect.gen(function* () {
            yield* db.patch(conn._id, {
              lastHeartbeat: Date.now(),
              status: "online",
            });
            yield* Effect.logInfo(`💓 [DEVICE-SYNC] Heartbeat updated for device: ${deviceId}`);
          }),
        onNone: () => 
          Effect.logWarning(`⚠️ [DEVICE-SYNC] Connection not found for heartbeat: ${deviceId}`)
      });

      return {};
    }),
});

// Get all devices for a user
export const getUserDevices = query({
  args: GetUserDevicesArgs,
  returns: GetUserDevicesResult,
  handler: ({ userId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      yield* Effect.logInfo(`🔍 [DEVICE-SYNC] Getting devices for user: ${userId}`);

      const devices = yield* db
        .query("deviceConnections")
        .withIndex("by_user_id", (q) => q.eq("userId", userId))
        .filter((q) => q.neq(q.field("status"), "offline"))
        .order("desc")
        .collect();

      // Check for stale connections (no heartbeat in 30 seconds)
      const now = Date.now();
      const activeDevices = devices.filter(device => 
        now - device.lastHeartbeat < HEARTBEAT_TIMEOUT
      );

      yield* Effect.logInfo(`📊 [DEVICE-SYNC] Found ${activeDevices.length} active devices`);

      // Transform to expected return format
      return activeDevices.map(device => ({
        userId: device.userId,
        deviceId: device.deviceId,
        status: device.status,
        sessionToken: device.sessionToken,
        roomToken: device.roomToken,
        connectedAt: device.connectedAt,
        lastHeartbeat: device.lastHeartbeat,
        deviceInfo: {
          deviceType: device.deviceType,
          platform: device.platform,
          appVersion: device.appVersion,
          userAgent: device.userAgent,
          lastSeen: device.lastHeartbeat,
          capabilities: device.capabilities,
        },
      }));
    }),
});

// Disconnect device and clean up
export const disconnectDevice = mutation({
  args: DisconnectDeviceArgs,
  returns: DisconnectDeviceResult,
  handler: ({ sessionToken }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`🔌 [DEVICE-SYNC] Disconnecting device with session: ${sessionToken}`);

      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) =>
          Effect.gen(function* () {
            // Mark device as offline
            yield* db.patch(conn._id, {
              status: "offline",
              lastHeartbeat: Date.now(),
            });

            // Remove from presence room
            const room = yield* db
              .query("devicePresenceRooms")
              .withIndex("by_room_id", (q) => q.eq("roomId", conn.roomToken))
              .first();

            yield* Option.match(room, {
              onSome: (r) => {
                const updatedDevices = (r.activeDevices || []).filter(id => id !== conn.deviceId);
                return db.patch(r._id, {
                  updatedAt: Date.now(),
                  activeDevices: updatedDevices,
                });
              },
              onNone: () => Effect.void
            });

            yield* Effect.logInfo(`✅ [DEVICE-SYNC] Device disconnected: ${conn.deviceId}`);
          }),
        onNone: () => 
          Effect.logWarning(`⚠️ [DEVICE-SYNC] Connection not found for disconnect: ${sessionToken}`)
      });

      return {};
    }),
});

// Update device status (online/offline/idle)
export const updateDeviceStatus = mutation({
  args: UpdateDeviceStatusArgs,
  returns: UpdateDeviceStatusResult,
  handler: ({ sessionToken, status }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`📊 [DEVICE-SYNC] Updating device status to: ${status}`);

      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) => {
          return db.patch(conn._id, {
            status,
            lastHeartbeat: Date.now(),
          });
        },
        onNone: () => 
          Effect.logWarning(`⚠️ [DEVICE-SYNC] Connection not found for status update: ${sessionToken}`)
      });

      return {};
    }),
});

// Create or get presence room for device sync
export const createPresenceRoom = mutation({
  args: CreatePresenceRoomArgs,
  returns: CreatePresenceRoomResult,
  handler: ({ roomId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`📡 [DEVICE-SYNC] Creating/getting presence room: ${roomId}`);

      const existingRoom = yield* db
        .query("devicePresenceRooms")
        .withIndex("by_room_id", (q) => q.eq("roomId", roomId))
        .first();

      return yield* Option.match(existingRoom, {
        onSome: (room) => Effect.succeed({
          roomId: room.roomId,
          createdAt: room.createdAt,
        }),
        onNone: () => 
          Effect.gen(function* () {
            const now = Date.now();
            yield* db.insert("devicePresenceRooms", {
              roomId,
              createdAt: now,
              updatedAt: now,
              activeDevices: [],
            });
            
            return { roomId, createdAt: now };
          })
      });
    }),
});