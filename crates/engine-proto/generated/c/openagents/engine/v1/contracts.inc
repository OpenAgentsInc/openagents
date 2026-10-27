// Generated from proto/. Do not edit.
#define OPENAGENTS_CONTRACT_TABLE_VERSION 2
#include "openagents/common/v1/refs.pb.h"
static const WireFieldDef fields_openagents_common_v1_EventRef[] = {
  {1, "id", 9, false, 64, 64, "", false},
  {2, "pubkey", 9, false, 64, 64, "", false},
  {3, "kind", 13, false, 4096, 64, "", false},
  {4, "coordinate", 9, false, 1024, 64, "", true},
};
static const WireFieldDef fields_openagents_common_v1_SourceHint[] = {
  {1, "url", 9, false, 4096, 64, "", true},
  {2, "event", 11, false, 4096, 64, "openagents.common.v1.EventRef", true},
};
static const WireFieldDef fields_openagents_common_v1_ArtifactRef[] = {
  {1, "digest", 9, false, 71, 64, "", false},
  {2, "size", 4, false, 4096, 64, "", false},
  {3, "media_type", 9, false, 128, 64, "", false},
  {4, "schema", 9, false, 256, 64, "", true},
  {5, "event", 11, false, 4096, 64, "openagents.common.v1.EventRef", true},
  {6, "sources", 11, true, 4096, 16, "openagents.common.v1.SourceHint", true},
};
static const WireFieldDef fields_openagents_common_v1_SignedEvent[] = {
  {1, "json", 9, false, 524288, 64, "", false},
};
static const WireFieldDef fields_openagents_common_v1_Refusal[] = {
  {1, "domain", 9, false, 32, 64, "", false},
  {2, "code", 9, false, 64, 64, "", false},
  {3, "message", 9, false, 512, 64, "", false},
  {4, "missing_right", 9, false, 4096, 64, "", true},
  {5, "retryable", 8, false, 4096, 64, "", false},
};
#include "openagents/engine/v1/agent.pb.h"
static const WireFieldDef fields_openagents_engine_v1_ToolActivity[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "kind", 9, false, 64, 100, "", false},
  {3, "name", 9, false, 80, 100, "", false},
  {4, "state", 9, false, 32, 100, "", false},
  {5, "input", 9, false, 4096, 100, "", false},
  {6, "output", 9, false, 12288, 100, "", false},
  {7, "exit", 17, false, 65536, 100, "", true},
  {8, "truncated", 8, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_AgentEvent[] = {
  {1, "seq", 4, false, 65536, 100, "", false},
  {2, "kind", 9, false, 64, 100, "", false},
  {3, "text", 9, false, 24000, 100, "", false},
  {4, "tool", 11, false, 65536, 100, "openagents.engine.v1.ToolActivity", true},
  {5, "id", 9, false, 128, 100, "", false},
  {6, "speaker", 9, false, 128, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_AgentTask[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "id", 9, false, 128, 100, "", false},
  {3, "target", 9, false, 128, 100, "", false},
  {4, "objective", 9, false, 4000, 100, "", false},
  {5, "phase", 9, false, 32, 100, "", false},
  {6, "revision", 4, false, 65536, 100, "", false},
  {7, "seq", 4, false, 65536, 100, "", false},
  {8, "write", 8, false, 65536, 100, "", false},
  {9, "admitted", 8, false, 65536, 100, "", false},
  {10, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent", true},
  {11, "history_truncated", 8, false, 65536, 100, "", false},
  {12, "connection", 9, false, 32, 100, "", false},
  {13, "result", 9, false, 16000, 100, "", false},
  {14, "exit", 17, false, 65536, 100, "", true},
  {15, "transport", 9, false, 32, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_AgentOperation[] = {
  {1, "op", 9, false, 32, 100, "", false},
  {2, "id", 9, false, 128, 100, "", false},
  {3, "target", 9, false, 128, 100, "", false},
  {4, "objective", 9, false, 4000, 100, "", false},
  {5, "revision", 4, false, 65536, 100, "", false},
  {6, "write", 8, false, 65536, 100, "", false},
  {7, "action", 9, false, 32, 100, "", false},
  {8, "run", 9, false, 128, 100, "", false},
  {9, "seq", 4, false, 65536, 100, "", false},
  {10, "state", 9, false, 32, 100, "", false},
  {11, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent", true},
  {12, "result", 9, false, 16000, 100, "", false},
  {13, "exit", 17, false, 65536, 100, "", true},
};
static const WireFieldDef fields_openagents_engine_v1_AgentReply[] = {
  {1, "ok", 8, false, 65536, 100, "", false},
  {2, "error", 9, false, 2000, 100, "", false},
  {3, "task", 11, false, 65536, 100, "openagents.engine.v1.AgentTask", true},
};
static const WireFieldDef fields_openagents_engine_v1_AgentDispatch[] = {
  {1, "v", 13, false, 65536, 100, "", false},
  {2, "op", 9, false, 32, 100, "", false},
  {3, "id", 9, false, 128, 100, "", false},
  {4, "target", 9, false, 128, 100, "", false},
  {5, "objective", 9, false, 4000, 100, "", false},
  {6, "write", 8, false, 65536, 100, "", false},
  {7, "after", 4, false, 65536, 100, "", false},
  {8, "transport", 9, false, 32, 100, "", false},
  {9, "admitted", 8, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_AgentProgress[] = {
  {1, "v", 13, false, 65536, 100, "", false},
  {2, "run", 9, false, 128, 100, "", false},
  {3, "state", 9, false, 32, 100, "", false},
  {4, "seq", 4, false, 65536, 100, "", false},
  {5, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent", true},
  {6, "result", 9, false, 16000, 100, "", false},
  {7, "exit", 17, false, 65536, 100, "", true},
  {8, "base", 9, false, 64, 100, "", false},
  {9, "head", 9, false, 64, 100, "", false},
  {10, "files", 9, true, 512, 40, "", false},
  {11, "more", 8, false, 65536, 100, "", false},
  {12, "result_truncated", 8, false, 65536, 100, "", false},
  {13, "error", 9, false, 2000, 100, "", false},
};
#include "openagents/engine/v1/assets.pb.h"
static const WireFieldDef fields_openagents_engine_v1_RequiredAssetBuffer[] = {
  {1, "semantic", 9, false, 48, 100, "", false},
  {2, "byte_stride", 13, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_RequiredAssetFile[] = {
  {1, "id", 9, false, 64, 100, "", false},
  {2, "path", 9, false, 192, 100, "", false},
  {3, "format", 9, false, 64, 100, "", false},
  {4, "dependencies", 9, true, 48, 8, "", false},
  {5, "buffers", 11, true, 65536, 8, "openagents.engine.v1.RequiredAssetBuffer", true},
};
static const WireFieldDef fields_openagents_engine_v1_ConsumerAssetProfile[] = {
  {1, "id", 9, false, 64, 100, "", false},
  {2, "scene_id", 9, false, 48, 100, "", false},
  {3, "files", 11, true, 65536, 32, "openagents.engine.v1.RequiredAssetFile", true},
};
static const WireFieldDef fields_openagents_engine_v1_AssetProfileCatalog[] = {
  {1, "schema_version", 13, false, 65536, 100, "", false},
  {2, "profiles", 11, true, 65536, 32, "openagents.engine.v1.ConsumerAssetProfile", true},
};
#include "openagents/engine/v1/control.pb.h"
static const WireFieldDef fields_openagents_engine_v1_CameraControl[] = {
  {1, "action", 9, false, 32, 100, "", false},
  {2, "delta", 1, false, 65536, 100, "", true},
  {3, "view", 17, false, 65536, 100, "", true},
  {4, "allow_toggle", 8, false, 65536, 100, "", true},
  {5, "on", 8, false, 65536, 100, "", true},
};
static const WireFieldDef fields_openagents_engine_v1_EntityEdit[] = {
  {1, "action", 9, false, 32, 100, "", false},
  {2, "id", 13, false, 65536, 100, "", true},
  {3, "mesh", 17, false, 65536, 100, "", true},
  {4, "x", 1, false, 65536, 100, "", true},
  {5, "y", 1, false, 65536, 100, "", true},
  {6, "z", 1, false, 65536, 100, "", true},
  {7, "yaw", 1, false, 65536, 100, "", true},
  {8, "scale", 1, false, 65536, 100, "", true},
  {9, "tint", 1, true, 65536, 3, "", false},
  {10, "glow", 1, false, 65536, 100, "", true},
  {11, "on", 8, false, 65536, 100, "", true},
  {12, "pickable", 8, false, 65536, 100, "", true},
};
static const WireFieldDef fields_openagents_engine_v1_InputIntent[] = {
  {1, "key_x", 1, false, 65536, 100, "", false},
  {2, "key_y", 1, false, 65536, 100, "", false},
  {3, "pad_x", 1, false, 65536, 100, "", false},
  {4, "pad_y", 1, false, 65536, 100, "", false},
  {5, "run", 8, false, 65536, 100, "", false},
  {6, "jump", 8, false, 65536, 100, "", false},
  {7, "dyaw", 1, false, 65536, 100, "", false},
  {8, "dpitch", 1, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_WorldLoad[] = {
  {1, "name", 9, false, 128, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_EngineCapabilities[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "world", 9, false, 128, 100, "", false},
  {3, "revision", 4, false, 65536, 100, "", false},
  {4, "operations", 9, true, 32, 16, "", false},
  {5, "camera_actions", 9, true, 32, 16, "", false},
  {6, "entity_actions", 9, true, 32, 16, "", false},
  {7, "world_load", 8, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_EngineRequest[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "request_id", 9, false, 128, 100, "", false},
  {3, "world", 9, false, 128, 100, "", false},
  {4, "expected_revision", 4, false, 65536, 100, "", false},
  {5, "op", 9, false, 32, 100, "", false},
  {6, "camera", 11, false, 65536, 100, "openagents.engine.v1.CameraControl", true},
  {7, "entity", 11, false, 65536, 100, "openagents.engine.v1.EntityEdit", true},
  {8, "input", 11, false, 65536, 100, "openagents.engine.v1.InputIntent", true},
  {9, "load", 11, false, 65536, 100, "openagents.engine.v1.WorldLoad", true},
};
static const WireFieldDef fields_openagents_engine_v1_EngineEvent[] = {
  {1, "request_id", 9, false, 128, 100, "", false},
  {2, "world", 9, false, 128, 100, "", false},
  {3, "revision", 4, false, 65536, 100, "", false},
  {4, "kind", 9, false, 64, 100, "", false},
  {5, "entity_id", 13, false, 65536, 100, "", true},
};
static const WireFieldDef fields_openagents_engine_v1_EngineReply[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "request_id", 9, false, 128, 100, "", false},
  {3, "revision", 4, false, 65536, 100, "", false},
  {4, "ok", 8, false, 65536, 100, "", false},
  {5, "code", 9, false, 64, 100, "", false},
  {6, "error", 9, false, 2000, 100, "", false},
  {7, "event", 11, false, 65536, 100, "openagents.engine.v1.EngineEvent", true},
  {8, "capabilities", 11, false, 65536, 100, "openagents.engine.v1.EngineCapabilities", true},
  {9, "entity_id", 13, false, 65536, 100, "", true},
  {10, "view", 17, false, 65536, 100, "", true},
};
#include "openagents/engine/v1/hud.pb.h"
static const WireFieldDef fields_openagents_engine_v1_ArtifactReference[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "path", 9, false, 65536, 100, "", false},
  {3, "uri", 9, false, 65536, 100, "", false},
  {4, "media_type", 9, false, 65536, 100, "", false},
  {5, "sha256", 9, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_InteractionRequest[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "tool_id", 9, false, 65536, 100, "", false},
  {3, "kind", 9, false, 64, 100, "", false},
  {4, "prompt", 9, false, 4000, 100, "", false},
  {5, "options", 9, true, 256, 32, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_ConversationSnapshot[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "id", 9, false, 128, 100, "", false},
  {3, "revision", 4, false, 65536, 100, "", false},
  {4, "phase", 9, false, 32, 100, "", false},
  {5, "objective", 9, false, 4000, 100, "", false},
  {6, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent", true},
  {7, "history_truncated", 8, false, 65536, 100, "", false},
  {8, "result", 9, false, 16000, 100, "", false},
  {9, "interactions", 11, true, 65536, 32, "openagents.engine.v1.InteractionRequest", true},
  {10, "artifacts", 11, true, 65536, 100, "openagents.engine.v1.ArtifactReference", true},
};
static const WireFieldDef fields_openagents_engine_v1_ConversationDelta[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "id", 9, false, 128, 100, "", false},
  {3, "base_revision", 4, false, 65536, 100, "", false},
  {4, "revision", 4, false, 65536, 100, "", false},
  {5, "upsert", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent", true},
  {6, "remove", 9, true, 128, 100, "", false},
  {7, "phase", 9, false, 32, 100, "", false},
  {8, "history_truncated", 8, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudBar[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "label", 9, false, 256, 100, "", false},
  {3, "kind", 9, false, 64, 100, "", false},
  {4, "text", 9, false, 24000, 100, "", false},
  {5, "value", 1, false, 65536, 100, "", false},
  {6, "max", 1, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudCounter[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "label", 9, false, 256, 100, "", false},
  {3, "value", 9, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudPrompt[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "verb", 9, false, 128, 100, "", false},
  {3, "key", 9, false, 32, 100, "", false},
  {4, "hold", 8, false, 65536, 100, "", false},
  {5, "progress", 1, false, 65536, 100, "", true},
};
static const WireFieldDef fields_openagents_engine_v1_HudAction[] = {
  {1, "id", 9, false, 128, 100, "", false},
  {2, "label", 9, false, 256, 100, "", false},
  {3, "enabled", 8, false, 65536, 100, "", false},
  {4, "pressed", 8, false, 65536, 100, "", false},
  {5, "shown", 8, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudInvocation[] = {
  {1, "schema", 13, false, 65536, 100, "", false},
  {2, "world", 9, false, 128, 100, "", false},
  {3, "action_id", 9, false, 65536, 100, "", false},
  {4, "request_id", 9, false, 65536, 100, "", false},
  {5, "revision", 4, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudSpeech[] = {
  {1, "speaker", 9, false, 65536, 100, "", false},
  {2, "text", 9, false, 24000, 100, "", false},
  {3, "replies", 9, true, 4000, 32, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudScreen[] = {
  {1, "kind", 9, false, 64, 100, "", false},
  {2, "title", 9, false, 65536, 100, "", false},
  {3, "detail", 9, false, 65536, 100, "", false},
  {4, "actions", 9, true, 65536, 128, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudGuide[] = {
  {1, "target", 9, false, 65536, 100, "", false},
  {2, "text", 9, false, 24000, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudRect[] = {
  {1, "values", 1, true, 65536, 4, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudPoint[] = {
  {1, "x", 1, false, 65536, 100, "", false},
  {2, "z", 1, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudMark[] = {
  {1, "x", 1, false, 65536, 100, "", false},
  {2, "z", 1, false, 65536, 100, "", false},
  {3, "kind", 9, false, 64, 100, "", false},
  {4, "label", 9, false, 256, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudMap[] = {
  {1, "bounds", 1, true, 65536, 4, "", false},
  {2, "grid", 1, false, 65536, 100, "", true},
  {3, "rects", 11, true, 65536, 4096, "openagents.engine.v1.HudRect", true},
  {4, "marks", 11, true, 65536, 4096, "openagents.engine.v1.HudMark", true},
  {5, "self", 1, true, 65536, 3, "", false},
  {6, "caption", 9, false, 65536, 100, "", false},
  {7, "plan", 13, false, 65536, 100, "", true},
  {8, "route", 11, true, 65536, 4096, "openagents.engine.v1.HudPoint", true},
};
static const WireFieldDef fields_openagents_engine_v1_HudToast[] = {
  {1, "id", 13, false, 128, 100, "", false},
  {2, "text", 9, false, 24000, 100, "", false},
  {3, "seconds", 1, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudHit[] = {
  {1, "id", 13, false, 128, 100, "", false},
  {2, "angle", 1, false, 65536, 100, "", false},
  {3, "amount", 1, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_HudFrame[] = {
  {1, "world", 9, false, 128, 100, "", false},
  {2, "bars", 11, true, 65536, 128, "openagents.engine.v1.HudBar", true},
  {3, "counters", 11, true, 65536, 128, "openagents.engine.v1.HudCounter", true},
  {4, "objective", 9, false, 4000, 100, "", false},
  {5, "quest", 9, false, 4000, 100, "", false},
  {6, "prompt", 11, false, 4000, 100, "openagents.engine.v1.HudPrompt", true},
  {7, "speech", 11, false, 65536, 100, "openagents.engine.v1.HudSpeech", true},
  {8, "screen", 11, false, 65536, 100, "openagents.engine.v1.HudScreen", true},
  {9, "guide", 11, false, 65536, 100, "openagents.engine.v1.HudGuide", true},
  {10, "actions", 11, true, 65536, 128, "openagents.engine.v1.HudAction", true},
  {11, "map", 11, false, 65536, 100, "openagents.engine.v1.HudMap", true},
  {12, "toasts", 11, true, 65536, 6, "openagents.engine.v1.HudToast", true},
  {13, "hits", 11, true, 65536, 8, "openagents.engine.v1.HudHit", true},
};
#include "openagents/engine/v1/state.pb.h"
static const WireFieldDef fields_openagents_engine_v1_RegisteredFieldSchema[] = {
  {1, "name", 9, false, 128, 100, "", false},
  {2, "kind", 14, false, 65536, 100, "openagents.engine.v1.RegisteredFieldKind", false},
  {3, "bits", 13, false, 65536, 100, "", false},
  {4, "source_contract", 9, false, 256, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_ComponentSchema[] = {
  {1, "name", 9, false, 128, 100, "", false},
  {2, "version", 13, false, 65536, 100, "", false},
  {3, "resource", 8, false, 65536, 100, "", false},
  {4, "fields", 11, true, 65536, 256, "openagents.engine.v1.RegisteredFieldSchema", true},
};
static const WireFieldDef fields_openagents_engine_v1_AuthoredSource[] = {
  {1, "contract", 9, false, 256, 100, "", false},
  {2, "json", 9, false, 1048576, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_RegisteredFieldValue[] = {
  {1, "name", 9, false, 128, 100, "", false},
  {2, "kind", 14, false, 65536, 100, "openagents.engine.v1.RegisteredFieldKind", false},
  {3, "boolean", 8, false, 65536, 100, "", true},
  {4, "integer", 18, false, 65536, 100, "", true},
  {5, "unsigned_value", 4, false, 65536, 100, "", true},
  {6, "number", 1, false, 65536, 100, "", true},
  {7, "text", 9, false, 65536, 100, "", true},
  {8, "vector", 1, true, 65536, 3, "", false},
  {9, "entity", 13, false, 65536, 100, "", true},
  {10, "source", 11, false, 65536, 100, "openagents.engine.v1.AuthoredSource", true},
};
static const WireFieldDef fields_openagents_engine_v1_ComponentState[] = {
  {1, "name", 9, false, 128, 100, "", false},
  {2, "version", 13, false, 65536, 100, "", false},
  {3, "fields", 11, true, 65536, 256, "openagents.engine.v1.RegisteredFieldValue", true},
};
static const WireFieldDef fields_openagents_engine_v1_EntityState[] = {
  {1, "id", 13, false, 65536, 100, "", false},
  {2, "components", 11, true, 65536, 256, "openagents.engine.v1.ComponentState", true},
};
static const WireFieldDef fields_openagents_engine_v1_ComponentVersion[] = {
  {1, "name", 9, false, 128, 100, "", false},
  {2, "version", 13, false, 65536, 100, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_EntityChange[] = {
  {1, "id", 13, false, 65536, 100, "", false},
  {2, "set", 11, true, 65536, 256, "openagents.engine.v1.ComponentState", true},
  {3, "put", 11, true, 65536, 256, "openagents.engine.v1.ComponentState", true},
  {4, "unset", 9, true, 128, 256, "", false},
};
static const WireFieldDef fields_openagents_engine_v1_WorldSnapshot[] = {
  {1, "contract_version", 13, false, 65536, 100, "", false},
  {2, "world", 9, false, 128, 100, "", false},
  {3, "schema", 13, false, 65536, 100, "", false},
  {4, "revision", 4, false, 65536, 100, "", false},
  {5, "definitions", 11, true, 65536, 1024, "openagents.engine.v1.ComponentSchema", true},
  {6, "entities", 11, true, 65536, 65536, "openagents.engine.v1.EntityState", true},
  {7, "resources", 11, true, 65536, 1024, "openagents.engine.v1.ComponentState", true},
};
static const WireFieldDef fields_openagents_engine_v1_WorldDelta[] = {
  {1, "contract_version", 13, false, 65536, 100, "", false},
  {2, "world", 9, false, 128, 100, "", false},
  {3, "schema", 13, false, 65536, 100, "", false},
  {4, "base_revision", 4, false, 65536, 100, "", false},
  {5, "revision", 4, false, 65536, 100, "", false},
  {6, "versions", 11, true, 65536, 1024, "openagents.engine.v1.ComponentVersion", true},
  {7, "added", 11, true, 65536, 65536, "openagents.engine.v1.EntityState", true},
  {8, "removed", 13, true, 65536, 65536, "", false},
  {9, "changed", 11, true, 65536, 65536, "openagents.engine.v1.EntityChange", true},
  {10, "resources", 11, true, 65536, 1024, "openagents.engine.v1.ComponentState", true},
  {11, "entity_order", 13, true, 65536, 65536, "", false},
};
#include "openagents/reach/v1/reach.pb.h"
static const WireFieldDef fields_openagents_reach_v1_VersionRange[] = {
  {1, "min", 13, false, 4096, 64, "", false},
  {2, "max", 13, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_reach_v1_Telemetry[] = {
  {1, "cpu_count", 13, false, 4096, 64, "", false},
  {2, "cpu_utilization_pct", 13, false, 4096, 64, "", false},
  {3, "memory_available_pct", 13, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_reach_v1_Presence[] = {
  {1, "host", 9, false, 4096, 64, "", false},
  {2, "owner", 9, false, 4096, 64, "", false},
  {3, "generation", 4, false, 4096, 64, "", false},
  {4, "protocol", 13, false, 4096, 64, "", false},
  {5, "compatibility", 11, false, 4096, 64, "openagents.reach.v1.VersionRange", true},
  {6, "capabilities", 9, true, 4096, 64, "", false},
  {7, "observed_at", 4, false, 4096, 64, "", false},
  {8, "telemetry", 11, false, 4096, 64, "openagents.reach.v1.Telemetry", true},
};
static const WireFieldDef fields_openagents_reach_v1_Hint[] = {
  {1, "class_name", 9, false, 4096, 64, "", false},
  {2, "transport", 9, false, 4096, 64, "", false},
  {3, "address", 9, false, 4096, 64, "", false},
  {4, "status", 9, false, 4096, 64, "", false},
  {5, "observed_at", 4, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_reach_v1_ReachState[] = {
  {1, "presence", 11, false, 4096, 64, "openagents.reach.v1.Presence", true},
  {2, "hints", 11, true, 4096, 64, "openagents.reach.v1.Hint", true},
  {3, "issued_at", 4, false, 4096, 64, "", false},
  {4, "expires_at", 4, false, 4096, 64, "", false},
  {5, "received_at", 4, false, 4096, 64, "", false},
};
#include "openagents/host/v1/host.pb.h"
static const WireFieldDef fields_openagents_host_v1_GrantOrigin[] = {
  {1, "kind", 9, false, 4096, 64, "", false},
  {2, "id", 9, false, 4096, 64, "", false},
  {3, "issuer", 9, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_Grant[] = {
  {1, "id", 9, false, 64, 64, "", false},
  {2, "host", 9, false, 64, 64, "", false},
  {3, "owner", 9, false, 64, 64, "", false},
  {4, "device", 9, false, 64, 64, "", false},
  {5, "relay", 9, false, 4096, 64, "", false},
  {6, "rights", 9, true, 16, 7, "", false},
  {7, "epoch", 4, false, 4096, 64, "", false},
  {8, "origin", 11, false, 4096, 64, "openagents.host.v1.GrantOrigin", true},
  {9, "issued_at", 4, false, 4096, 64, "", false},
  {10, "expires_at", 4, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_Access[] = {
  {1, "grant", 11, false, 4096, 64, "openagents.host.v1.Grant", true},
  {2, "authorization", 11, false, 4096, 64, "openagents.common.v1.SignedEvent", true},
};
static const WireFieldDef fields_openagents_host_v1_TaskScope[] = {
  {1, "workspace", 9, false, 4096, 64, "", false},
  {2, "task", 9, false, 4096, 64, "", false},
  {3, "revision", 4, false, 4096, 64, "", false},
  {4, "attempt", 4, false, 4096, 64, "", true},
  {5, "intent_digest", 9, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_TaskCursor[] = {
  {1, "scope", 11, false, 4096, 64, "openagents.host.v1.TaskScope", true},
  {2, "source", 9, false, 4096, 64, "", false},
  {3, "source_digest", 9, false, 4096, 64, "", false},
  {4, "source_bytes", 4, false, 4096, 64, "", false},
  {5, "next_step", 4, false, 4096, 64, "", false},
  {6, "prefix_digest", 9, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_Connection[] = {
  {1, "schema", 13, false, 4096, 64, "", false},
  {2, "access", 11, false, 4096, 64, "openagents.host.v1.Access", true},
  {3, "workspace", 9, false, 4096, 64, "", false},
  {4, "peer", 9, false, 4096, 64, "", false},
  {5, "label", 9, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_TaskBinding[] = {
  {1, "schema", 13, false, 4096, 64, "", false},
  {2, "phone_run", 9, false, 4096, 64, "", false},
  {3, "remote_task", 9, false, 4096, 64, "", false},
  {4, "revision", 4, false, 4096, 64, "", false},
  {5, "pending", 11, false, 4096, 64, "openagents.host.v1.PreparedCall", true},
  {6, "cursor", 11, false, 4096, 64, "openagents.host.v1.TaskCursor", true},
  {7, "cursor_after", 4, false, 4096, 64, "", false},
  {8, "cancel_sent", 8, false, 4096, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_PreparedCall[] = {
  {1, "pending_json", 9, false, 524288, 64, "", false},
  {2, "operation", 9, false, 4096, 64, "", false},
  {3, "phone_run", 9, false, 4096, 64, "", false},
  {4, "workspace", 9, false, 4096, 64, "", false},
  {5, "remote_task", 9, false, 4096, 64, "", false},
  {6, "after", 4, false, 4096, 64, "", false},
  {7, "cursor", 11, false, 4096, 64, "openagents.host.v1.TaskCursor", true},
};
static const WireFieldDef fields_openagents_host_v1_AgentRequest[] = {
  {1, "schema", 13, false, 4096, 64, "", false},
  {2, "operation", 9, false, 4096, 64, "", false},
  {3, "invitation", 9, false, 640, 64, "", false},
  {4, "access", 11, false, 4096, 64, "openagents.host.v1.Access", true},
  {5, "pending", 11, false, 4096, 64, "openagents.host.v1.PreparedCall", true},
  {6, "phone_run", 9, false, 128, 64, "", false},
  {7, "workspace", 9, false, 128, 64, "", false},
  {8, "objective", 9, false, 4000, 64, "", false},
  {9, "remote_task", 9, false, 64, 64, "", false},
  {10, "revision", 4, false, 4096, 64, "", false},
  {11, "after", 4, false, 4096, 64, "", false},
  {12, "allow_writes", 8, false, 4096, 64, "", false},
  {13, "cursor", 11, false, 4096, 64, "openagents.host.v1.TaskCursor", true},
  {14, "stream_address", 9, false, 512, 64, "", false},
};
static const WireFieldDef fields_openagents_host_v1_AgentReply[] = {
  {1, "schema", 13, false, 4096, 64, "", false},
  {2, "error", 11, false, 4096, 64, "openagents.common.v1.Refusal", true},
  {3, "access", 11, false, 4096, 64, "openagents.host.v1.Access", true},
  {4, "reach", 11, false, 4096, 64, "openagents.reach.v1.ReachState", true},
  {5, "pending", 11, false, 4096, 64, "openagents.host.v1.PreparedCall", true},
  {6, "progress", 11, false, 4096, 64, "openagents.engine.v1.AgentProgress", true},
  {7, "remote_task", 9, false, 4096, 64, "", false},
  {8, "revision", 4, false, 4096, 64, "", false},
  {9, "route", 9, false, 4096, 64, "", false},
  {10, "cursor", 11, false, 4096, 64, "openagents.host.v1.TaskCursor", true},
  {11, "host_phase", 9, false, 4096, 64, "", false},
};
static const WireMessageDef wire_messages[] = {
  {"openagents.common.v1.EventRef", &openagents_common_v1_EventRef_msg, sizeof(openagents_common_v1_EventRef), fields_openagents_common_v1_EventRef, 4},
  {"openagents.common.v1.SourceHint", &openagents_common_v1_SourceHint_msg, sizeof(openagents_common_v1_SourceHint), fields_openagents_common_v1_SourceHint, 2},
  {"openagents.common.v1.ArtifactRef", &openagents_common_v1_ArtifactRef_msg, sizeof(openagents_common_v1_ArtifactRef), fields_openagents_common_v1_ArtifactRef, 6},
  {"openagents.common.v1.SignedEvent", &openagents_common_v1_SignedEvent_msg, sizeof(openagents_common_v1_SignedEvent), fields_openagents_common_v1_SignedEvent, 1},
  {"openagents.common.v1.Refusal", &openagents_common_v1_Refusal_msg, sizeof(openagents_common_v1_Refusal), fields_openagents_common_v1_Refusal, 5},
  {"openagents.engine.v1.ToolActivity", &openagents_engine_v1_ToolActivity_msg, sizeof(openagents_engine_v1_ToolActivity), fields_openagents_engine_v1_ToolActivity, 8},
  {"openagents.engine.v1.AgentEvent", &openagents_engine_v1_AgentEvent_msg, sizeof(openagents_engine_v1_AgentEvent), fields_openagents_engine_v1_AgentEvent, 6},
  {"openagents.engine.v1.AgentTask", &openagents_engine_v1_AgentTask_msg, sizeof(openagents_engine_v1_AgentTask), fields_openagents_engine_v1_AgentTask, 15},
  {"openagents.engine.v1.AgentOperation", &openagents_engine_v1_AgentOperation_msg, sizeof(openagents_engine_v1_AgentOperation), fields_openagents_engine_v1_AgentOperation, 13},
  {"openagents.engine.v1.AgentReply", &openagents_engine_v1_AgentReply_msg, sizeof(openagents_engine_v1_AgentReply), fields_openagents_engine_v1_AgentReply, 3},
  {"openagents.engine.v1.AgentDispatch", &openagents_engine_v1_AgentDispatch_msg, sizeof(openagents_engine_v1_AgentDispatch), fields_openagents_engine_v1_AgentDispatch, 9},
  {"openagents.engine.v1.AgentProgress", &openagents_engine_v1_AgentProgress_msg, sizeof(openagents_engine_v1_AgentProgress), fields_openagents_engine_v1_AgentProgress, 13},
  {"openagents.engine.v1.RequiredAssetBuffer", &openagents_engine_v1_RequiredAssetBuffer_msg, sizeof(openagents_engine_v1_RequiredAssetBuffer), fields_openagents_engine_v1_RequiredAssetBuffer, 2},
  {"openagents.engine.v1.RequiredAssetFile", &openagents_engine_v1_RequiredAssetFile_msg, sizeof(openagents_engine_v1_RequiredAssetFile), fields_openagents_engine_v1_RequiredAssetFile, 5},
  {"openagents.engine.v1.ConsumerAssetProfile", &openagents_engine_v1_ConsumerAssetProfile_msg, sizeof(openagents_engine_v1_ConsumerAssetProfile), fields_openagents_engine_v1_ConsumerAssetProfile, 3},
  {"openagents.engine.v1.AssetProfileCatalog", &openagents_engine_v1_AssetProfileCatalog_msg, sizeof(openagents_engine_v1_AssetProfileCatalog), fields_openagents_engine_v1_AssetProfileCatalog, 2},
  {"openagents.engine.v1.CameraControl", &openagents_engine_v1_CameraControl_msg, sizeof(openagents_engine_v1_CameraControl), fields_openagents_engine_v1_CameraControl, 5},
  {"openagents.engine.v1.EntityEdit", &openagents_engine_v1_EntityEdit_msg, sizeof(openagents_engine_v1_EntityEdit), fields_openagents_engine_v1_EntityEdit, 12},
  {"openagents.engine.v1.InputIntent", &openagents_engine_v1_InputIntent_msg, sizeof(openagents_engine_v1_InputIntent), fields_openagents_engine_v1_InputIntent, 8},
  {"openagents.engine.v1.WorldLoad", &openagents_engine_v1_WorldLoad_msg, sizeof(openagents_engine_v1_WorldLoad), fields_openagents_engine_v1_WorldLoad, 1},
  {"openagents.engine.v1.EngineCapabilities", &openagents_engine_v1_EngineCapabilities_msg, sizeof(openagents_engine_v1_EngineCapabilities), fields_openagents_engine_v1_EngineCapabilities, 7},
  {"openagents.engine.v1.EngineRequest", &openagents_engine_v1_EngineRequest_msg, sizeof(openagents_engine_v1_EngineRequest), fields_openagents_engine_v1_EngineRequest, 9},
  {"openagents.engine.v1.EngineEvent", &openagents_engine_v1_EngineEvent_msg, sizeof(openagents_engine_v1_EngineEvent), fields_openagents_engine_v1_EngineEvent, 5},
  {"openagents.engine.v1.EngineReply", &openagents_engine_v1_EngineReply_msg, sizeof(openagents_engine_v1_EngineReply), fields_openagents_engine_v1_EngineReply, 10},
  {"openagents.engine.v1.ArtifactReference", &openagents_engine_v1_ArtifactReference_msg, sizeof(openagents_engine_v1_ArtifactReference), fields_openagents_engine_v1_ArtifactReference, 5},
  {"openagents.engine.v1.InteractionRequest", &openagents_engine_v1_InteractionRequest_msg, sizeof(openagents_engine_v1_InteractionRequest), fields_openagents_engine_v1_InteractionRequest, 5},
  {"openagents.engine.v1.ConversationSnapshot", &openagents_engine_v1_ConversationSnapshot_msg, sizeof(openagents_engine_v1_ConversationSnapshot), fields_openagents_engine_v1_ConversationSnapshot, 10},
  {"openagents.engine.v1.ConversationDelta", &openagents_engine_v1_ConversationDelta_msg, sizeof(openagents_engine_v1_ConversationDelta), fields_openagents_engine_v1_ConversationDelta, 8},
  {"openagents.engine.v1.HudBar", &openagents_engine_v1_HudBar_msg, sizeof(openagents_engine_v1_HudBar), fields_openagents_engine_v1_HudBar, 6},
  {"openagents.engine.v1.HudCounter", &openagents_engine_v1_HudCounter_msg, sizeof(openagents_engine_v1_HudCounter), fields_openagents_engine_v1_HudCounter, 3},
  {"openagents.engine.v1.HudPrompt", &openagents_engine_v1_HudPrompt_msg, sizeof(openagents_engine_v1_HudPrompt), fields_openagents_engine_v1_HudPrompt, 5},
  {"openagents.engine.v1.HudAction", &openagents_engine_v1_HudAction_msg, sizeof(openagents_engine_v1_HudAction), fields_openagents_engine_v1_HudAction, 5},
  {"openagents.engine.v1.HudInvocation", &openagents_engine_v1_HudInvocation_msg, sizeof(openagents_engine_v1_HudInvocation), fields_openagents_engine_v1_HudInvocation, 5},
  {"openagents.engine.v1.HudSpeech", &openagents_engine_v1_HudSpeech_msg, sizeof(openagents_engine_v1_HudSpeech), fields_openagents_engine_v1_HudSpeech, 3},
  {"openagents.engine.v1.HudScreen", &openagents_engine_v1_HudScreen_msg, sizeof(openagents_engine_v1_HudScreen), fields_openagents_engine_v1_HudScreen, 4},
  {"openagents.engine.v1.HudGuide", &openagents_engine_v1_HudGuide_msg, sizeof(openagents_engine_v1_HudGuide), fields_openagents_engine_v1_HudGuide, 2},
  {"openagents.engine.v1.HudRect", &openagents_engine_v1_HudRect_msg, sizeof(openagents_engine_v1_HudRect), fields_openagents_engine_v1_HudRect, 1},
  {"openagents.engine.v1.HudPoint", &openagents_engine_v1_HudPoint_msg, sizeof(openagents_engine_v1_HudPoint), fields_openagents_engine_v1_HudPoint, 2},
  {"openagents.engine.v1.HudMark", &openagents_engine_v1_HudMark_msg, sizeof(openagents_engine_v1_HudMark), fields_openagents_engine_v1_HudMark, 4},
  {"openagents.engine.v1.HudMap", &openagents_engine_v1_HudMap_msg, sizeof(openagents_engine_v1_HudMap), fields_openagents_engine_v1_HudMap, 8},
  {"openagents.engine.v1.HudToast", &openagents_engine_v1_HudToast_msg, sizeof(openagents_engine_v1_HudToast), fields_openagents_engine_v1_HudToast, 3},
  {"openagents.engine.v1.HudHit", &openagents_engine_v1_HudHit_msg, sizeof(openagents_engine_v1_HudHit), fields_openagents_engine_v1_HudHit, 3},
  {"openagents.engine.v1.HudFrame", &openagents_engine_v1_HudFrame_msg, sizeof(openagents_engine_v1_HudFrame), fields_openagents_engine_v1_HudFrame, 13},
  {"openagents.engine.v1.RegisteredFieldSchema", &openagents_engine_v1_RegisteredFieldSchema_msg, sizeof(openagents_engine_v1_RegisteredFieldSchema), fields_openagents_engine_v1_RegisteredFieldSchema, 4},
  {"openagents.engine.v1.ComponentSchema", &openagents_engine_v1_ComponentSchema_msg, sizeof(openagents_engine_v1_ComponentSchema), fields_openagents_engine_v1_ComponentSchema, 4},
  {"openagents.engine.v1.AuthoredSource", &openagents_engine_v1_AuthoredSource_msg, sizeof(openagents_engine_v1_AuthoredSource), fields_openagents_engine_v1_AuthoredSource, 2},
  {"openagents.engine.v1.RegisteredFieldValue", &openagents_engine_v1_RegisteredFieldValue_msg, sizeof(openagents_engine_v1_RegisteredFieldValue), fields_openagents_engine_v1_RegisteredFieldValue, 10},
  {"openagents.engine.v1.ComponentState", &openagents_engine_v1_ComponentState_msg, sizeof(openagents_engine_v1_ComponentState), fields_openagents_engine_v1_ComponentState, 3},
  {"openagents.engine.v1.EntityState", &openagents_engine_v1_EntityState_msg, sizeof(openagents_engine_v1_EntityState), fields_openagents_engine_v1_EntityState, 2},
  {"openagents.engine.v1.ComponentVersion", &openagents_engine_v1_ComponentVersion_msg, sizeof(openagents_engine_v1_ComponentVersion), fields_openagents_engine_v1_ComponentVersion, 2},
  {"openagents.engine.v1.EntityChange", &openagents_engine_v1_EntityChange_msg, sizeof(openagents_engine_v1_EntityChange), fields_openagents_engine_v1_EntityChange, 4},
  {"openagents.engine.v1.WorldSnapshot", &openagents_engine_v1_WorldSnapshot_msg, sizeof(openagents_engine_v1_WorldSnapshot), fields_openagents_engine_v1_WorldSnapshot, 7},
  {"openagents.engine.v1.WorldDelta", &openagents_engine_v1_WorldDelta_msg, sizeof(openagents_engine_v1_WorldDelta), fields_openagents_engine_v1_WorldDelta, 11},
  {"openagents.reach.v1.VersionRange", &openagents_reach_v1_VersionRange_msg, sizeof(openagents_reach_v1_VersionRange), fields_openagents_reach_v1_VersionRange, 2},
  {"openagents.reach.v1.Telemetry", &openagents_reach_v1_Telemetry_msg, sizeof(openagents_reach_v1_Telemetry), fields_openagents_reach_v1_Telemetry, 3},
  {"openagents.reach.v1.Presence", &openagents_reach_v1_Presence_msg, sizeof(openagents_reach_v1_Presence), fields_openagents_reach_v1_Presence, 8},
  {"openagents.reach.v1.Hint", &openagents_reach_v1_Hint_msg, sizeof(openagents_reach_v1_Hint), fields_openagents_reach_v1_Hint, 5},
  {"openagents.reach.v1.ReachState", &openagents_reach_v1_ReachState_msg, sizeof(openagents_reach_v1_ReachState), fields_openagents_reach_v1_ReachState, 5},
  {"openagents.host.v1.GrantOrigin", &openagents_host_v1_GrantOrigin_msg, sizeof(openagents_host_v1_GrantOrigin), fields_openagents_host_v1_GrantOrigin, 3},
  {"openagents.host.v1.Grant", &openagents_host_v1_Grant_msg, sizeof(openagents_host_v1_Grant), fields_openagents_host_v1_Grant, 10},
  {"openagents.host.v1.Access", &openagents_host_v1_Access_msg, sizeof(openagents_host_v1_Access), fields_openagents_host_v1_Access, 2},
  {"openagents.host.v1.TaskScope", &openagents_host_v1_TaskScope_msg, sizeof(openagents_host_v1_TaskScope), fields_openagents_host_v1_TaskScope, 5},
  {"openagents.host.v1.TaskCursor", &openagents_host_v1_TaskCursor_msg, sizeof(openagents_host_v1_TaskCursor), fields_openagents_host_v1_TaskCursor, 6},
  {"openagents.host.v1.Connection", &openagents_host_v1_Connection_msg, sizeof(openagents_host_v1_Connection), fields_openagents_host_v1_Connection, 5},
  {"openagents.host.v1.TaskBinding", &openagents_host_v1_TaskBinding_msg, sizeof(openagents_host_v1_TaskBinding), fields_openagents_host_v1_TaskBinding, 8},
  {"openagents.host.v1.PreparedCall", &openagents_host_v1_PreparedCall_msg, sizeof(openagents_host_v1_PreparedCall), fields_openagents_host_v1_PreparedCall, 7},
  {"openagents.host.v1.AgentRequest", &openagents_host_v1_AgentRequest_msg, sizeof(openagents_host_v1_AgentRequest), fields_openagents_host_v1_AgentRequest, 14},
  {"openagents.host.v1.AgentReply", &openagents_host_v1_AgentReply_msg, sizeof(openagents_host_v1_AgentReply), fields_openagents_host_v1_AgentReply, 11},
};
