-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0004_sorting: groups of people, and rules about whole domains.

CREATE TABLE groups (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL COLLATE NOCASE UNIQUE,
  ink        INTEGER NOT NULL DEFAULT 0 CHECK (ink BETWEEN 0 AND 9),
  created_at INTEGER NOT NULL
) STRICT;

CREATE TABLE person_groups (
  person_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  group_id  INTEGER NOT NULL REFERENCES groups (id) ON DELETE CASCADE,
  PRIMARY KEY (person_id, group_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX person_groups_by_group ON person_groups (group_id);

-- What to do with a sender nobody has decided about yet, by the domain they write from.
CREATE TABLE domain_rules (
  domain     TEXT PRIMARY KEY COLLATE NOCASE,
  action     TEXT NOT NULL CHECK (action IN ('allow', 'hide', 'block')),
  created_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;
