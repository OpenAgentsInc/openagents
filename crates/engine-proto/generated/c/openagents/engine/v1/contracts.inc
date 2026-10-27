// Generated from proto/. Do not edit.
#include "openagents/engine/v1/agent.pb.h"
static const WireFieldDef fields_openagents_engine_v1_ToolActivity[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "kind", 9, false, 64, 100, ""},
  {3, "name", 9, false, 80, 100, ""},
  {4, "state", 9, false, 32, 100, ""},
  {5, "input", 9, false, 4096, 100, ""},
  {6, "output", 9, false, 12288, 100, ""},
  {7, "exit", 17, false, 65536, 100, ""},
  {8, "truncated", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_AgentEvent[] = {
  {1, "seq", 4, false, 65536, 100, ""},
  {2, "kind", 9, false, 64, 100, ""},
  {3, "text", 9, false, 24000, 100, ""},
  {4, "tool", 11, false, 65536, 100, "openagents.engine.v1.ToolActivity"},
  {5, "id", 9, false, 128, 100, ""},
  {6, "speaker", 9, false, 128, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_AgentTask[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "id", 9, false, 128, 100, ""},
  {3, "target", 9, false, 128, 100, ""},
  {4, "objective", 9, false, 4000, 100, ""},
  {5, "phase", 9, false, 32, 100, ""},
  {6, "revision", 4, false, 65536, 100, ""},
  {7, "seq", 4, false, 65536, 100, ""},
  {8, "write", 8, false, 65536, 100, ""},
  {9, "admitted", 8, false, 65536, 100, ""},
  {10, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent"},
  {11, "history_truncated", 8, false, 65536, 100, ""},
  {12, "connection", 9, false, 32, 100, ""},
  {13, "result", 9, false, 16000, 100, ""},
  {14, "exit", 17, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_AgentOperation[] = {
  {1, "op", 9, false, 32, 100, ""},
  {2, "id", 9, false, 128, 100, ""},
  {3, "target", 9, false, 128, 100, ""},
  {4, "objective", 9, false, 4000, 100, ""},
  {5, "revision", 4, false, 65536, 100, ""},
  {6, "write", 8, false, 65536, 100, ""},
  {7, "action", 9, false, 32, 100, ""},
  {8, "run", 9, false, 128, 100, ""},
  {9, "seq", 4, false, 65536, 100, ""},
  {10, "state", 9, false, 32, 100, ""},
  {11, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent"},
  {12, "result", 9, false, 16000, 100, ""},
  {13, "exit", 17, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_AgentReply[] = {
  {1, "ok", 8, false, 65536, 100, ""},
  {2, "error", 9, false, 2000, 100, ""},
  {3, "task", 11, false, 65536, 100, "openagents.engine.v1.AgentTask"},
};
static const WireFieldDef fields_openagents_engine_v1_AgentDispatch[] = {
  {1, "v", 13, false, 65536, 100, ""},
  {2, "op", 9, false, 32, 100, ""},
  {3, "id", 9, false, 128, 100, ""},
  {4, "target", 9, false, 128, 100, ""},
  {5, "objective", 9, false, 4000, 100, ""},
  {6, "write", 8, false, 65536, 100, ""},
  {7, "after", 4, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_AgentProgress[] = {
  {1, "v", 13, false, 65536, 100, ""},
  {2, "run", 9, false, 128, 100, ""},
  {3, "state", 9, false, 32, 100, ""},
  {4, "seq", 4, false, 65536, 100, ""},
  {5, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent"},
  {6, "result", 9, false, 16000, 100, ""},
  {7, "exit", 17, false, 65536, 100, ""},
  {8, "base", 9, false, 64, 100, ""},
  {9, "head", 9, false, 64, 100, ""},
  {10, "files", 9, true, 512, 40, ""},
  {11, "more", 8, false, 65536, 100, ""},
  {12, "result_truncated", 8, false, 65536, 100, ""},
  {13, "error", 9, false, 2000, 100, ""},
};
#include "openagents/engine/v1/control.pb.h"
static const WireFieldDef fields_openagents_engine_v1_CameraControl[] = {
  {1, "action", 9, false, 32, 100, ""},
  {2, "delta", 1, false, 65536, 100, ""},
  {3, "view", 17, false, 65536, 100, ""},
  {4, "allow_toggle", 8, false, 65536, 100, ""},
  {5, "on", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_EntityEdit[] = {
  {1, "action", 9, false, 32, 100, ""},
  {2, "id", 13, false, 65536, 100, ""},
  {3, "mesh", 17, false, 65536, 100, ""},
  {4, "x", 1, false, 65536, 100, ""},
  {5, "y", 1, false, 65536, 100, ""},
  {6, "z", 1, false, 65536, 100, ""},
  {7, "yaw", 1, false, 65536, 100, ""},
  {8, "scale", 1, false, 65536, 100, ""},
  {9, "tint", 1, true, 65536, 3, ""},
  {10, "glow", 1, false, 65536, 100, ""},
  {11, "on", 8, false, 65536, 100, ""},
  {12, "pickable", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_InputIntent[] = {
  {1, "key_x", 1, false, 65536, 100, ""},
  {2, "key_y", 1, false, 65536, 100, ""},
  {3, "pad_x", 1, false, 65536, 100, ""},
  {4, "pad_y", 1, false, 65536, 100, ""},
  {5, "run", 8, false, 65536, 100, ""},
  {6, "jump", 8, false, 65536, 100, ""},
  {7, "dyaw", 1, false, 65536, 100, ""},
  {8, "dpitch", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_WorldLoad[] = {
  {1, "name", 9, false, 128, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_EngineCapabilities[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "world", 9, false, 128, 100, ""},
  {3, "revision", 4, false, 65536, 100, ""},
  {4, "operations", 9, true, 32, 16, ""},
  {5, "camera_actions", 9, true, 32, 16, ""},
  {6, "entity_actions", 9, true, 32, 16, ""},
  {7, "world_load", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_EngineRequest[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "request_id", 9, false, 128, 100, ""},
  {3, "world", 9, false, 128, 100, ""},
  {4, "expected_revision", 4, false, 65536, 100, ""},
  {5, "op", 9, false, 32, 100, ""},
  {6, "camera", 11, false, 65536, 100, "openagents.engine.v1.CameraControl"},
  {7, "entity", 11, false, 65536, 100, "openagents.engine.v1.EntityEdit"},
  {8, "input", 11, false, 65536, 100, "openagents.engine.v1.InputIntent"},
  {9, "load", 11, false, 65536, 100, "openagents.engine.v1.WorldLoad"},
};
static const WireFieldDef fields_openagents_engine_v1_EngineEvent[] = {
  {1, "request_id", 9, false, 128, 100, ""},
  {2, "world", 9, false, 128, 100, ""},
  {3, "revision", 4, false, 65536, 100, ""},
  {4, "kind", 9, false, 64, 100, ""},
  {5, "entity_id", 13, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_EngineReply[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "request_id", 9, false, 128, 100, ""},
  {3, "revision", 4, false, 65536, 100, ""},
  {4, "ok", 8, false, 65536, 100, ""},
  {5, "code", 9, false, 64, 100, ""},
  {6, "error", 9, false, 2000, 100, ""},
  {7, "event", 11, false, 65536, 100, "openagents.engine.v1.EngineEvent"},
  {8, "capabilities", 11, false, 65536, 100, "openagents.engine.v1.EngineCapabilities"},
  {9, "entity_id", 13, false, 65536, 100, ""},
  {10, "view", 17, false, 65536, 100, ""},
};
#include "openagents/engine/v1/hud.pb.h"
static const WireFieldDef fields_openagents_engine_v1_ArtifactReference[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "path", 9, false, 65536, 100, ""},
  {3, "uri", 9, false, 65536, 100, ""},
  {4, "media_type", 9, false, 65536, 100, ""},
  {5, "sha256", 9, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_InteractionRequest[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "tool_id", 9, false, 65536, 100, ""},
  {3, "kind", 9, false, 64, 100, ""},
  {4, "prompt", 9, false, 4000, 100, ""},
  {5, "options", 9, true, 256, 32, ""},
};
static const WireFieldDef fields_openagents_engine_v1_ConversationSnapshot[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "id", 9, false, 128, 100, ""},
  {3, "revision", 4, false, 65536, 100, ""},
  {4, "phase", 9, false, 32, 100, ""},
  {5, "objective", 9, false, 4000, 100, ""},
  {6, "events", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent"},
  {7, "history_truncated", 8, false, 65536, 100, ""},
  {8, "result", 9, false, 16000, 100, ""},
  {9, "interactions", 11, true, 65536, 32, "openagents.engine.v1.InteractionRequest"},
  {10, "artifacts", 11, true, 65536, 100, "openagents.engine.v1.ArtifactReference"},
};
static const WireFieldDef fields_openagents_engine_v1_ConversationDelta[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "id", 9, false, 128, 100, ""},
  {3, "base_revision", 4, false, 65536, 100, ""},
  {4, "revision", 4, false, 65536, 100, ""},
  {5, "upsert", 11, true, 65536, 100, "openagents.engine.v1.AgentEvent"},
  {6, "remove", 9, true, 128, 100, ""},
  {7, "phase", 9, false, 32, 100, ""},
  {8, "history_truncated", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudBar[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "label", 9, false, 256, 100, ""},
  {3, "kind", 9, false, 64, 100, ""},
  {4, "text", 9, false, 24000, 100, ""},
  {5, "value", 1, false, 65536, 100, ""},
  {6, "max", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudCounter[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "label", 9, false, 256, 100, ""},
  {3, "value", 9, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudPrompt[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "verb", 9, false, 128, 100, ""},
  {3, "key", 9, false, 32, 100, ""},
  {4, "hold", 8, false, 65536, 100, ""},
  {5, "progress", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudAction[] = {
  {1, "id", 9, false, 128, 100, ""},
  {2, "label", 9, false, 256, 100, ""},
  {3, "enabled", 8, false, 65536, 100, ""},
  {4, "pressed", 8, false, 65536, 100, ""},
  {5, "shown", 8, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudInvocation[] = {
  {1, "schema", 13, false, 65536, 100, ""},
  {2, "world", 9, false, 128, 100, ""},
  {3, "action_id", 9, false, 65536, 100, ""},
  {4, "request_id", 9, false, 65536, 100, ""},
  {5, "revision", 4, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudSpeech[] = {
  {1, "speaker", 9, false, 65536, 100, ""},
  {2, "text", 9, false, 24000, 100, ""},
  {3, "replies", 9, true, 4000, 32, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudScreen[] = {
  {1, "kind", 9, false, 64, 100, ""},
  {2, "title", 9, false, 65536, 100, ""},
  {3, "detail", 9, false, 65536, 100, ""},
  {4, "actions", 9, true, 65536, 128, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudGuide[] = {
  {1, "target", 9, false, 65536, 100, ""},
  {2, "text", 9, false, 24000, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudRect[] = {
  {1, "values", 1, true, 65536, 4, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudPoint[] = {
  {1, "x", 1, false, 65536, 100, ""},
  {2, "z", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudMark[] = {
  {1, "x", 1, false, 65536, 100, ""},
  {2, "z", 1, false, 65536, 100, ""},
  {3, "kind", 9, false, 64, 100, ""},
  {4, "label", 9, false, 256, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudMap[] = {
  {1, "bounds", 1, true, 65536, 4, ""},
  {2, "grid", 1, false, 65536, 100, ""},
  {3, "rects", 11, true, 65536, 4096, "openagents.engine.v1.HudRect"},
  {4, "marks", 11, true, 65536, 4096, "openagents.engine.v1.HudMark"},
  {5, "self", 1, true, 65536, 3, ""},
  {6, "caption", 9, false, 65536, 100, ""},
  {7, "plan", 13, false, 65536, 100, ""},
  {8, "route", 11, true, 65536, 4096, "openagents.engine.v1.HudPoint"},
};
static const WireFieldDef fields_openagents_engine_v1_HudToast[] = {
  {1, "id", 13, false, 128, 100, ""},
  {2, "text", 9, false, 24000, 100, ""},
  {3, "seconds", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudHit[] = {
  {1, "id", 13, false, 128, 100, ""},
  {2, "angle", 1, false, 65536, 100, ""},
  {3, "amount", 1, false, 65536, 100, ""},
};
static const WireFieldDef fields_openagents_engine_v1_HudFrame[] = {
  {1, "world", 9, false, 128, 100, ""},
  {2, "bars", 11, true, 65536, 128, "openagents.engine.v1.HudBar"},
  {3, "counters", 11, true, 65536, 128, "openagents.engine.v1.HudCounter"},
  {4, "objective", 9, false, 4000, 100, ""},
  {5, "quest", 9, false, 4000, 100, ""},
  {6, "prompt", 11, false, 4000, 100, "openagents.engine.v1.HudPrompt"},
  {7, "speech", 11, false, 65536, 100, "openagents.engine.v1.HudSpeech"},
  {8, "screen", 11, false, 65536, 100, "openagents.engine.v1.HudScreen"},
  {9, "guide", 11, false, 65536, 100, "openagents.engine.v1.HudGuide"},
  {10, "actions", 11, true, 65536, 128, "openagents.engine.v1.HudAction"},
  {11, "map", 11, false, 65536, 100, "openagents.engine.v1.HudMap"},
  {12, "toasts", 11, true, 65536, 6, "openagents.engine.v1.HudToast"},
  {13, "hits", 11, true, 65536, 8, "openagents.engine.v1.HudHit"},
};
static const WireMessageDef wire_messages[] = {
  {"openagents.engine.v1.ToolActivity", &openagents_engine_v1_ToolActivity_msg, sizeof(openagents_engine_v1_ToolActivity), fields_openagents_engine_v1_ToolActivity, 8},
  {"openagents.engine.v1.AgentEvent", &openagents_engine_v1_AgentEvent_msg, sizeof(openagents_engine_v1_AgentEvent), fields_openagents_engine_v1_AgentEvent, 6},
  {"openagents.engine.v1.AgentTask", &openagents_engine_v1_AgentTask_msg, sizeof(openagents_engine_v1_AgentTask), fields_openagents_engine_v1_AgentTask, 14},
  {"openagents.engine.v1.AgentOperation", &openagents_engine_v1_AgentOperation_msg, sizeof(openagents_engine_v1_AgentOperation), fields_openagents_engine_v1_AgentOperation, 13},
  {"openagents.engine.v1.AgentReply", &openagents_engine_v1_AgentReply_msg, sizeof(openagents_engine_v1_AgentReply), fields_openagents_engine_v1_AgentReply, 3},
  {"openagents.engine.v1.AgentDispatch", &openagents_engine_v1_AgentDispatch_msg, sizeof(openagents_engine_v1_AgentDispatch), fields_openagents_engine_v1_AgentDispatch, 7},
  {"openagents.engine.v1.AgentProgress", &openagents_engine_v1_AgentProgress_msg, sizeof(openagents_engine_v1_AgentProgress), fields_openagents_engine_v1_AgentProgress, 13},
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
};
