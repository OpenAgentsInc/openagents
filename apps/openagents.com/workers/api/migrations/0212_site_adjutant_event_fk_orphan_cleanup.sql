-- Prod FK drift cleanup (#5506).
--
-- D1 batch migrations historically ran with foreign key enforcement disabled,
-- which allowed a small number of event rows to retain references whose parent
-- rows no longer exist. Clear that drift by FK predicate only, not hard-coded
-- prod row ids.

DELETE FROM site_events
WHERE NOT EXISTS (
        SELECT 1
          FROM site_projects
         WHERE site_projects.id = site_events.site_id
      )
   OR (
        version_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM site_versions
           WHERE site_versions.id = site_events.version_id
        )
      )
   OR (
        deployment_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM site_deployments
           WHERE site_deployments.id = site_events.deployment_id
        )
      )
   OR (
        actor_user_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM users
           WHERE users.id = site_events.actor_user_id
        )
      )
   OR (
        actor_run_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM agent_runs
           WHERE agent_runs.id = site_events.actor_run_id
        )
      );

DELETE FROM adjutant_assignment_events
WHERE NOT EXISTS (
        SELECT 1
          FROM adjutant_assignments
         WHERE adjutant_assignments.id = adjutant_assignment_events.assignment_id
      )
   OR (
        software_order_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM software_orders
           WHERE software_orders.id = adjutant_assignment_events.software_order_id
        )
      )
   OR (
        site_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM site_projects
           WHERE site_projects.id = adjutant_assignment_events.site_id
        )
      )
   OR (
        goal_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM agent_goals
           WHERE agent_goals.id = adjutant_assignment_events.goal_id
        )
      )
   OR (
        run_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM agent_runs
           WHERE agent_runs.id = adjutant_assignment_events.run_id
        )
      )
   OR (
        actor_user_id IS NOT NULL
        AND NOT EXISTS (
          SELECT 1
            FROM users
           WHERE users.id = adjutant_assignment_events.actor_user_id
        )
      );
