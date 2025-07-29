import { Effect, Option, Data } from "effect";
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
  GetRoomDevicesArgs,
  GetRoomDevicesResult,
} from "./device-sync.schemas";

// Device-specific error types
export class DeviceError extends Data.TaggedError("DeviceError")<{
  operation: string;
  message: string;
  cause?: unknown;
}> {}

export class DeviceNotFoundError extends Data.TaggedError("DeviceNotFoundError")<{
  deviceId: string;
}> {}

export class DeviceAuthError extends Data.TaggedError("DeviceAuthError")<{
  operation: string;
  deviceId?: string;
}> {}

// Helper function to get authenticated user with Effect-TS patterns (for mutations)
const getAuthenticatedUserEffectMutation = Effect.gen(function* () {
  const { db, auth } = yield* ConfectMutationCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new DeviceAuthError({
      operation: "getAuthenticatedUser",
    }));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [DEVICE-SYNC] Looking for user with OpenAuth subject: ${authSubject}`);
  
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
          onNone: () => Effect.fail(new DeviceAuthError({
            operation: "getAuthenticatedUser",
          }))
        });
      })
  });
});

// Helper function to get authenticated user with Effect-TS patterns (for queries)
const getAuthenticatedUserEffectQuery = Effect.gen(function* () {
  const { db, auth } = yield* ConfectQueryCtx;
  
  const identity = yield* auth.getUserIdentity();
  if (Option.isNone(identity)) {
    return yield* Effect.fail(new DeviceAuthError({
      operation: "getAuthenticatedUser",
    }));
  }

  // Look up user by OpenAuth subject first
  const authSubject = identity.value.subject;
  console.log(`🔍 [DEVICE-SYNC] Looking for user with OpenAuth subject: ${authSubject}`);
  
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
          onNone: () => Effect.fail(new DeviceAuthError({
            operation: "getAuthenticatedUser",
          }))
        });
      })
  });
});

// Helper function to generate unique device ID
const generateDeviceId = () => {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substring(2, 15);
  return `device_${timestamp}_${random}`;
};

// Helper function to generate session token
const generateSessionToken = () => {
  const timestamp = Date.now();
  const random = Math.random().toString(36).substring(2, 15);
  return `session_${timestamp}_${random}`;
};

// Connect device to user's presence room
export const connectDevice = mutation({
  args: ConnectDeviceArgs,
  returns: ConnectDeviceResult,
  handler: ({ deviceInfo }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;
      const user = yield* getAuthenticatedUserEffectMutation;

      yield* Effect.logInfo(`🔗 [DEVICE-SYNC] connectDevice called:`, {
        deviceType: deviceInfo.deviceType,
        platform: deviceInfo.platform,
        userId: user._id,
      });

      // Generate unique identifiers
      const deviceId = generateDeviceId();
      const sessionToken = generateSessionToken();
      const roomToken = `room_${user._id}`;

      // Ensure presence room exists for user
      const existingRoom = yield* db
        .query("devicePresenceRooms")
        .withIndex("by_room_id", (q) => q.eq("roomId", roomToken))
        .first();

      if (Option.isNone(existingRoom)) {
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
      const connectionId = yield* db.insert("deviceConnections", {
        deviceId,
        userId: user._id,
        deviceType: deviceInfo.deviceType,
        platform: deviceInfo.platform,
        appVersion: deviceInfo.appVersion,
        userAgent: deviceInfo.userAgent,
        status: "online" as const,
        sessionToken,
        roomToken,
        connectedAt: Date.now(),
        lastHeartbeat: Date.now(),
        capabilities: deviceInfo.capabilities || [],
      });

      yield* Effect.logInfo(`✅ [DEVICE-SYNC] Device connected:`, {
        deviceId,
        sessionToken,
        roomToken,
        connectionId,
      });

      return { deviceId, sessionToken, roomToken };
    }),
});

// Send heartbeat to keep device connection alive
export const deviceHeartbeat = mutation({
  args: DeviceHeartbeatArgs,
  returns: DeviceHeartbeatResult,
  handler: ({ deviceId, sessionToken }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      // Find device connection
      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .filter((q) => q.eq(q.field("deviceId"), deviceId))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) =>
          Effect.gen(function* () {
            yield* db.update(conn._id, {
              lastHeartbeat: Date.now(),
              status: "online" as const,
            });
            yield* Effect.logInfo(`💓 [DEVICE-SYNC] Heartbeat updated for device: ${deviceId}`);
          }),
        onNone: () => 
          Effect.fail(new DeviceNotFoundError({ deviceId })),
      });

      return null;
    }),
});

// Get all connected devices for a user
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
      const HEARTBEAT_TIMEOUT = 30000; // 30 seconds
      
      const activeDevices = devices.filter(device => 
        now - device.lastHeartbeat < HEARTBEAT_TIMEOUT
      );

      // Mark stale devices as offline
      const staleDevices = devices.filter(device => 
        now - device.lastHeartbeat >= HEARTBEAT_TIMEOUT
      );

      for (const staleDevice of staleDevices) {
        yield* db.update(staleDevice._id, {
          status: "offline" as const,
          lastHeartbeat: now,
        });
      }

      yield* Effect.logInfo(`📊 [DEVICE-SYNC] Found ${activeDevices.length} active devices, marked ${staleDevices.length} as stale`);

      // Return devices with proper schema format
      return activeDevices.map(device => ({
        deviceId: device.deviceId,
        userId: device.userId,
        deviceInfo: {
          deviceType: device.deviceType,
          platform: device.platform,
          appVersion: device.appVersion,
          userAgent: device.userAgent,
          lastSeen: device.lastHeartbeat,
          capabilities: device.capabilities,
        },
        status: device.status,
        sessionToken: device.sessionToken,
        roomToken: device.roomToken,
        connectedAt: device.connectedAt,
        lastHeartbeat: device.lastHeartbeat,
      }));
    }),
});

// Disconnect device and remove from presence room
export const disconnectDevice = mutation({
  args: DisconnectDeviceArgs,
  returns: DisconnectDeviceResult,
  handler: ({ sessionToken }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`🔌 [DEVICE-SYNC] disconnectDevice called with session: ${sessionToken}`);

      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) =>
          Effect.gen(function* () {
            // Mark device as offline
            yield* db.update(conn._id, {
              status: "offline" as const,
              lastHeartbeat: Date.now(),
            });

            // Remove from presence room
            const room = yield* db
              .query("devicePresenceRooms")
              .withIndex("by_room_id", (q) => q.eq("roomId", conn.roomToken))
              .first();

            yield* Option.match(room, {
              onSome: (r) => {
                const activeDevices = (r.activeDevices || []).filter(id => id !== conn.deviceId);
                return db.update(r._id, {
                  updatedAt: Date.now(),
                  activeDevices,
                });
              },
              onNone: () => Effect.void,
            });

            yield* Effect.logInfo(`✅ [DEVICE-SYNC] Device disconnected: ${conn.deviceId}`);
          }),
        onNone: () => 
          Effect.logInfo(`⚠️ [DEVICE-SYNC] No connection found for session: ${sessionToken}`),
      });

      return null;
    }),
});

// Update device status (online, offline, idle)
export const updateDeviceStatus = mutation({
  args: UpdateDeviceStatusArgs,
  returns: UpdateDeviceStatusResult,
  handler: ({ deviceId, sessionToken, status }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`🔄 [DEVICE-SYNC] updateDeviceStatus:`, {
        deviceId,
        status,
      });

      const connection = yield* db
        .query("deviceConnections")
        .withIndex("by_session_token", (q) => q.eq("sessionToken", sessionToken))
        .filter((q) => q.eq(q.field("deviceId"), deviceId))
        .first();

      yield* Option.match(connection, {
        onSome: (conn) => {
          return db.update(conn._id, {
            status,
            lastHeartbeat: Date.now(),
          });
        },
        onNone: () => 
          Effect.fail(new DeviceNotFoundError({ deviceId })),
      });

      return null;
    }),
});

// Create presence room for real-time sync
export const createPresenceRoom = mutation({
  args: CreatePresenceRoomArgs,
  returns: CreatePresenceRoomResult,
  handler: ({ roomId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectMutationCtx;

      yield* Effect.logInfo(`🏠 [DEVICE-SYNC] createPresenceRoom: ${roomId}`);

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

            yield* Effect.logInfo(`✅ [DEVICE-SYNC] Created presence room: ${roomId}`);

            return { roomId, createdAt: now };
          }),
      });
    }),
});

// Get all devices in a presence room
export const getRoomDevices = query({
  args: GetRoomDevicesArgs,
  returns: GetRoomDevicesResult,
  handler: ({ roomId }) =>
    Effect.gen(function* () {
      const { db } = yield* ConfectQueryCtx;

      yield* Effect.logInfo(`🏠 [DEVICE-SYNC] getRoomDevices: ${roomId}`);

      const devices = yield* db
        .query("deviceConnections")
        .withIndex("by_room_token", (q) => q.eq("roomToken", roomId))
        .filter((q) => q.neq(q.field("status"), "offline"))
        .order("desc")
        .collect();

      // Check for stale connections
      const now = Date.now();
      const HEARTBEAT_TIMEOUT = 30000; // 30 seconds
      
      const activeDevices = devices.filter(device => 
        now - device.lastHeartbeat < HEARTBEAT_TIMEOUT
      );

      yield* Effect.logInfo(`📊 [DEVICE-SYNC] Room ${roomId} has ${activeDevices.length} active devices`);

      // Return devices with proper schema format
      return activeDevices.map(device => ({
        deviceId: device.deviceId,
        userId: device.userId,
        deviceInfo: {
          deviceType: device.deviceType,
          platform: device.platform,
          appVersion: device.appVersion,
          userAgent: device.userAgent,
          lastSeen: device.lastHeartbeat,
          capabilities: device.capabilities,
        },
        status: device.status,
        sessionToken: device.sessionToken,
        roomToken: device.roomToken,
        connectedAt: device.connectedAt,
        lastHeartbeat: device.lastHeartbeat,
      }));
    }),
});