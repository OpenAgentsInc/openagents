import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

const migrationSql = readFileSync(
  join(
    __dirname,
    '..',
    'migrations',
    '0212_site_adjutant_event_fk_orphan_cleanup.sql',
  ),
  'utf8',
)

const rows = (db: DatabaseSync, query: string) =>
  db.prepare(query).all() as Array<Record<string, unknown>>

const createSchema = (db: DatabaseSync) => {
  db.exec(`
    PRAGMA foreign_keys = ON;

    CREATE TABLE users (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE agent_runs (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE software_orders (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE agent_goals (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE site_projects (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE site_versions (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE site_deployments (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE adjutant_assignments (
      id TEXT PRIMARY KEY NOT NULL
    );

    CREATE TABLE site_events (
      id TEXT PRIMARY KEY NOT NULL,
      site_id TEXT NOT NULL REFERENCES site_projects(id) ON DELETE CASCADE,
      version_id TEXT REFERENCES site_versions(id) ON DELETE SET NULL,
      deployment_id TEXT REFERENCES site_deployments(id) ON DELETE SET NULL,
      actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
      actor_run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL
    );

    CREATE TABLE adjutant_assignment_events (
      id TEXT PRIMARY KEY NOT NULL,
      assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
      software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
      site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
      goal_id TEXT REFERENCES agent_goals(id) ON DELETE SET NULL,
      run_id TEXT REFERENCES agent_runs(id) ON DELETE SET NULL,
      actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL
    );
  `)
}

const seedParents = (db: DatabaseSync) => {
  db.exec(`
    INSERT INTO users (id) VALUES ('user-ok');
    INSERT INTO agent_runs (id) VALUES ('run-ok');
    INSERT INTO software_orders (id) VALUES ('order-ok');
    INSERT INTO agent_goals (id) VALUES ('goal-ok');
    INSERT INTO site_projects (id) VALUES ('site-ok');
    INSERT INTO site_versions (id) VALUES ('version-ok');
    INSERT INTO site_deployments (id) VALUES ('deployment-ok');
    INSERT INTO adjutant_assignments (id) VALUES ('assignment-ok');
  `)
}

const seedForeignKeyDrift = (db: DatabaseSync) => {
  db.exec(`
    PRAGMA foreign_keys = OFF;

    INSERT INTO site_events (
      id,
      site_id,
      version_id,
      deployment_id,
      actor_user_id,
      actor_run_id
    ) VALUES
      ('site-event-valid', 'site-ok', 'version-ok', 'deployment-ok', 'user-ok', 'run-ok'),
      ('site-event-missing-required-site', 'site-missing', NULL, NULL, NULL, NULL),
      ('site-event-missing-optional-refs', 'site-ok', 'version-missing', 'deployment-missing', 'user-missing', 'run-missing');

    INSERT INTO adjutant_assignment_events (
      id,
      assignment_id,
      software_order_id,
      site_id,
      goal_id,
      run_id,
      actor_user_id
    ) VALUES
      ('assignment-event-valid', 'assignment-ok', 'order-ok', 'site-ok', 'goal-ok', 'run-ok', 'user-ok'),
      ('assignment-event-missing-required-assignment', 'assignment-missing', NULL, NULL, NULL, NULL, NULL),
      ('assignment-event-missing-optional-refs', 'assignment-ok', 'order-missing', 'site-missing', 'goal-missing', 'run-missing', 'user-missing');

    PRAGMA foreign_keys = ON;
  `)
}

describe('site/adjutant event FK cleanup migration', () => {
  test('clears child-table FK drift and remains rerunnable', () => {
    const db = new DatabaseSync(':memory:')
    createSchema(db)
    seedParents(db)
    seedForeignKeyDrift(db)

    expect(rows(db, 'PRAGMA foreign_key_check')).toHaveLength(11)

    db.exec(migrationSql)

    expect(rows(db, 'PRAGMA foreign_key_check')).toEqual([])
    expect(
      rows(
        db,
        'SELECT id FROM site_events ORDER BY id',
      ).map(row => row.id),
    ).toEqual(['site-event-valid'])
    expect(
      rows(
        db,
        'SELECT id FROM adjutant_assignment_events ORDER BY id',
      ).map(row => row.id),
    ).toEqual(['assignment-event-valid'])

    db.exec(migrationSql)

    expect(rows(db, 'PRAGMA foreign_key_check')).toEqual([])
    expect(rows(db, 'SELECT id FROM site_events')).toHaveLength(1)
    expect(rows(db, 'SELECT id FROM adjutant_assignment_events')).toHaveLength(
      1,
    )
    db.close()
  })
})
