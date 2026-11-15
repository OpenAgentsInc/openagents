
„

9openagents/environment/execution/v1alpha1/execution.proto)openagents.environment.execution.v1alpha1"…
Binding
schema (	Rschema
task_id (	Rtask_id(
conversation_id (	Rconversation_id
turn_id (	Rturn_id
route_id (	Rroute_id 
host_pubkey (	Rhost_pubkey"
workspace_id (	Rworkspace_id

session_id (	R
session_id

command_id	 (	R
command_id
	engine_id
 (	R	engine_id

account_id (	R
account_id
model_id (	Rmodel_id
	effort_id (	R	effort_id"
input_sha256 (	Rinput_sha256 
input_bytes (Rinput_bytes$
report_sha256 (	Rreport_sha2564
report_observed_at_ms (Rreport_observed_at_ms"
issued_at_ms (Rissued_at_ms$
expires_at_ms (Rexpires_at_ms"6
InputMessage
role (	Rrole
text (	Rtext"¶
Input
schema (	Rschema"
instructions (	Rinstructions
	reference (	R	referenceS
messages (27.openagents.environment.execution.v1alpha1.InputMessageRmessages"š

InputChunk
schema (	Rschema
task_id (	Rtask_id

command_id (	R
command_id"
input_sha256 (	Rinput_sha256 
input_bytes (Rinput_bytes
offset (Roffset
text (	Rtext"
issued_at_ms (Rissued_at_ms$
expires_at_ms	 (Rexpires_at_msbproto3
ä
;openagents/environment/execution/v1alpha1/preparation.proto)openagents.environment.execution.v1alpha1"F
PreparationRequest
schema (	Rschema
task_id (	Rtask_id"Í
PreparationView
schema (	Rschema 
host_pubkey (	Rhost_pubkey
task_id (	Rtask_id

command_id (	R
command_id"
input_sha256 (	Rinput_sha256 
input_bytes (Rinput_bytes&
retained_bytes (Rretained_bytes
complete (Rcomplete
replay	 (Rreplay$
expires_at_ms
 (Rexpires_at_ms"Ù
InputCommandReceipt
schema (	Rschema
task_id (	Rtask_id

command_id (	R
command_id&
request_sha256 (	Rrequest_sha256"
issued_at_ms (Rissued_at_ms$
expires_at_ms (Rexpires_at_msbproto3