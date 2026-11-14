-- Connection routing is account-owned; HOST still checks its current local
-- grant, epoch, workspace and caller for every request. No provider key here.
CREATE TABLE chat.environment_connections_v1 (
  account text NOT NULL REFERENCES auth.accounts(id) ON DELETE CASCADE,
  host_pubkey text NOT NULL CHECK (host_pubkey ~ '^[0-9a-f]{64}$'),
  display_name text NOT NULL CHECK (octet_length(display_name) BETWEEN 1 AND 80),
  grant_id text NOT NULL CHECK (grant_id ~ '^[0-9a-f]{64}$'),
  epoch bigint NOT NULL CHECK (epoch >= 0),
  relay text NOT NULL,
  PRIMARY KEY(account,host_pubkey)
);
INSERT INTO auth.life_columns (tbl,col,holds,on_merge,on_purge,export,omit,why) VALUES
 ('chat.environment_connections_v1','account','account','end','delete',true,'{}','routing delegation ends on account merge; local HOST authority remains independent');
