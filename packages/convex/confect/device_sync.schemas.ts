import { Schema } from "effect";
import { Id } from "@rjdellecese/confect/server";

// Device information schema
export const DeviceInfo = Schema.Struct({
  deviceType: Schema.Literal("desktop", "mobile", "web"),
  platform: Schema.String, // "macos", "ios", "android", "windows", "linux", "browser"
  appVersion: Schema.String,
  userAgent: Schema.optional(Schema.String),
  lastSeen: Schema.Number,
  capabilities: Schema.Array(Schema.String), // ["claude-code", "file-sync", "push-notifications"]
});

// Device connection schema
export const DeviceConnection = Schema.Struct({
  deviceId: Schema.String,
  userId: Schema.String,
  deviceInfo: DeviceInfo,
  status: Schema.Literal("online", "offline", "idle"),
  sessionToken: Schema.String,
  roomToken: Schema.String,
  connectedAt: Schema.Number,
  lastHeartbeat: Schema.Number,
});

// Device update events
export const DeviceUpdate = Schema.Struct({
  type: Schema.Literal("connected", "disconnected", "heartbeat", "status_change"),
  deviceId: Schema.String,
  userId: Schema.String,
  timestamp: Schema.Number,
  data: Schema.optional(Schema.Record({ key: Schema.String, value: Schema.Unknown })),
});

// Connect device mutation args
export const ConnectDeviceArgs = Schema.Struct({
  deviceInfo: DeviceInfo,
});

export const ConnectDeviceResult = Schema.Struct({
  deviceId: Schema.String,
  sessionToken: Schema.String,
  roomToken: Schema.String,
});

// Device heartbeat args
export const DeviceHeartbeatArgs = Schema.Struct({
  deviceId: Schema.String,
  sessionToken: Schema.String,
});

export const DeviceHeartbeatResult = Schema.NullOr(Schema.Struct({}));

// Get user devices args
export const GetUserDevicesArgs = Schema.Struct({
  userId: Id.Id("users"),
});

export const GetUserDevicesResult = Schema.Array(DeviceConnection);

// Disconnect device args
export const DisconnectDeviceArgs = Schema.Struct({
  sessionToken: Schema.String,
});

export const DisconnectDeviceResult = Schema.NullOr(Schema.Struct({}));

// Update device status args
export const UpdateDeviceStatusArgs = Schema.Struct({
  deviceId: Schema.String,
  sessionToken: Schema.String,
  status: Schema.Literal("online", "offline", "idle"),
});

export const UpdateDeviceStatusResult = Schema.NullOr(Schema.Struct({}));

// Device presence room args
export const CreatePresenceRoomArgs = Schema.Struct({
  roomId: Schema.String,
});

export const CreatePresenceRoomResult = Schema.Struct({
  roomId: Schema.String,
  createdAt: Schema.Number,
});

// Get devices in room args
export const GetRoomDevicesArgs = Schema.Struct({
  roomId: Schema.String,
});

export const GetRoomDevicesResult = Schema.Array(DeviceConnection);

// Type exports for TypeScript usage
export type DeviceInfo = typeof DeviceInfo.Type;
export type DeviceConnection = typeof DeviceConnection.Type;
export type DeviceUpdate = typeof DeviceUpdate.Type;
export type ConnectDeviceArgs = typeof ConnectDeviceArgs.Type;
export type ConnectDeviceResult = typeof ConnectDeviceResult.Type;
export type DeviceHeartbeatArgs = typeof DeviceHeartbeatArgs.Type;
export type DeviceHeartbeatResult = typeof DeviceHeartbeatResult.Type;
export type GetUserDevicesArgs = typeof GetUserDevicesArgs.Type;
export type GetUserDevicesResult = typeof GetUserDevicesResult.Type;
export type DisconnectDeviceArgs = typeof DisconnectDeviceArgs.Type;
export type DisconnectDeviceResult = typeof DisconnectDeviceResult.Type;
export type UpdateDeviceStatusArgs = typeof UpdateDeviceStatusArgs.Type;
export type UpdateDeviceStatusResult = typeof UpdateDeviceStatusResult.Type;
export type CreatePresenceRoomArgs = typeof CreatePresenceRoomArgs.Type;
export type CreatePresenceRoomResult = typeof CreatePresenceRoomResult.Type;
export type GetRoomDevicesArgs = typeof GetRoomDevicesArgs.Type;
export type GetRoomDevicesResult = typeof GetRoomDevicesResult.Type;