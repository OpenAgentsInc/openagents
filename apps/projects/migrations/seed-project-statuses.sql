-- Insert default project statuses if they don't exist
INSERT OR IGNORE INTO project_status (id, name, description, color, type, position, indefinite)
VALUES 
  ('status-backlog', 'Backlog', 'Projects in planning stage', '#95A5A6', 'backlog', 0, 1),
  ('status-planned', 'Planned', 'Projects that are planned to start', '#3498DB', 'planned', 1, 1),
  ('status-started', 'In Progress', 'Projects that are currently in progress', '#F1C40F', 'started', 2, 0),
  ('status-paused', 'Paused', 'Projects that are temporarily paused', '#E67E22', 'paused', 3, 0),
  ('status-completed', 'Completed', 'Projects that are successfully completed', '#2ECC71', 'completed', 4, 0),
  ('status-canceled', 'Canceled', 'Projects that are canceled', '#E74C3C', 'canceled', 5, 0);