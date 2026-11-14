
ß
4openagents/environment/access/v1alpha1/runtime.proto&openagents.environment.access.v1alpha1"Ž
RuntimeRequest
method (	Rmethod"
workspace_id (	Rworkspace_id

session_id (	R
session_id

command_id (	R
command_id

generation (R
generation,
expected_revision (Rexpected_revision
text (	Rtext&
interaction_id (	Rinteraction_id
allow	 (Rallow
before
 (Rbefore$
expires_at_ms (Rexpires_at_ms"
issued_at_ms (Rissued_at_ms"‰
RuntimeItem
item_id (	Ritem_id
role (	Rrole
text (	Rtext
	tool_name (	R	tool_name
complete (Rcomplete"º
RuntimeSession

session_id (	R
session_id"
workspace_id (	Rworkspace_id
title (	Rtitle
origin (	Rorigin
control (	Rcontrol

generation (R
generation
revision (Rrevision
busy (Rbusy&
interaction_id	 (	Rinteraction_id$
updated_at_ms
 (Rupdated_at_ms"â
RuntimeView
schema (	Rschema 
host_pubkey (	Rhost_pubkey"
display_name (	Rdisplay_name
method (	RmethodR
sessions (26.openagents.environment.access.v1alpha1.RuntimeSessionRsessionsI
items (23.openagents.environment.access.v1alpha1.RuntimeItemRitems
error (	Rerror
complete (Rcomplete 
next_cursor	 (	Rnext_cursor

command_id
 (	R
command_id&
command_status (	Rcommand_status"
source_epoch (	Rsource_epoch"Ü
RuntimeConfiguration
schema (	Rschema
device (	Rdevice
grant (	Rgrant
	workspace (	R	workspace"
display_name (	Rdisplay_name
enabled (Renabled"
history_root (	Rhistory_root"T
CatalogEntry 
host_pubkey (	Rhost_pubkey"
display_name (	Rdisplay_name"‚
RuntimeCatalog
schema (	RschemaX
environments (24.openagents.environment.access.v1alpha1.CatalogEntryRenvironmentsbproto3