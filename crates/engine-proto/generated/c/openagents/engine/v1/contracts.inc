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
static const WireMessageDef wire_messages[] = {
  {"openagents.engine.v1.ToolActivity", &openagents_engine_v1_ToolActivity_msg, sizeof(openagents_engine_v1_ToolActivity), fields_openagents_engine_v1_ToolActivity, 8},
  {"openagents.engine.v1.AgentEvent", &openagents_engine_v1_AgentEvent_msg, sizeof(openagents_engine_v1_AgentEvent), fields_openagents_engine_v1_AgentEvent, 6},
  {"openagents.engine.v1.AgentTask", &openagents_engine_v1_AgentTask_msg, sizeof(openagents_engine_v1_AgentTask), fields_openagents_engine_v1_AgentTask, 14},
  {"openagents.engine.v1.AgentOperation", &openagents_engine_v1_AgentOperation_msg, sizeof(openagents_engine_v1_AgentOperation), fields_openagents_engine_v1_AgentOperation, 13},
  {"openagents.engine.v1.AgentReply", &openagents_engine_v1_AgentReply_msg, sizeof(openagents_engine_v1_AgentReply), fields_openagents_engine_v1_AgentReply, 3},
  {"openagents.engine.v1.AgentDispatch", &openagents_engine_v1_AgentDispatch_msg, sizeof(openagents_engine_v1_AgentDispatch), fields_openagents_engine_v1_AgentDispatch, 7},
  {"openagents.engine.v1.AgentProgress", &openagents_engine_v1_AgentProgress_msg, sizeof(openagents_engine_v1_AgentProgress), fields_openagents_engine_v1_AgentProgress, 13},
};
