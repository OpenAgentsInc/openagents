-- Lightning Address rows in 0149_auth_life's account registry (#13208): what deleting, merging
-- and exporting an account do with them. A merge leaves the address with the merged account: its
-- held name can't move to another account.
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('pay.addresses', 'account', 'account', 'keep', 'delete', true, '{}', 'the Lightning Address; a paid name stays held in pay.name_holds'),
  ('pay.address_changes', 'account', 'account', 'keep', 'delete', true, '{}', 'Lightning Address destination changes'),
  ('pay.notices', 'account', 'account', 'keep', 'delete', true, '{}', 'Lightning Address notices'),
  ('pay.name_holds', 'account', 'account', 'keep', 'keep', true, '{}', 'a paid name is held for good');
