-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0005_automation: rules that sort new mail, watches that wait for particular mail, the log
-- of notifications, and saved searches.

CREATE TABLE rules (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  enabled     INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  position    INTEGER NOT NULL DEFAULT 0,
  query       TEXT NOT NULL,
  -- A JSON array of actions, see store::rules::RuleAction.
  actions     TEXT NOT NULL,
  -- Rules after this one do not run for a message that matched it.
  stop        INTEGER NOT NULL DEFAULT 0 CHECK (stop IN (0, 1)),
  hit_count   INTEGER NOT NULL DEFAULT 0,
  last_hit_at INTEGER,
  created_at  INTEGER NOT NULL
) STRICT;

CREATE TABLE watches (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  query       TEXT NOT NULL,
  -- once: done at the first match. keep: keeps watching.
  mode        TEXT NOT NULL DEFAULT 'once' CHECK (mode IN ('once', 'keep')),
  priority    TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('normal', 'urgent')),
  status      TEXT NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'arrived', 'expired', 'paused')),
  created_at  INTEGER NOT NULL,
  expires_at  INTEGER,
  arrived_at  INTEGER,
  hit_count   INTEGER NOT NULL DEFAULT 0,
  last_hit_at INTEGER
) STRICT;

CREATE TABLE watch_hits (
  id              INTEGER PRIMARY KEY,
  watch_id        INTEGER NOT NULL REFERENCES watches (id) ON DELETE CASCADE,
  message_id      INTEGER NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
  conversation_id INTEGER NOT NULL,
  matched_at      INTEGER NOT NULL,
  -- Order and tracking numbers found in the message: a JSON array of {kind, value}.
  extracted       TEXT NOT NULL DEFAULT '[]',
  seen_at         INTEGER,
  UNIQUE (watch_id, message_id)
) STRICT;

CREATE INDEX watch_hits_by_watch ON watch_hits (watch_id, matched_at DESC);

-- Every notification that fired, and every one that was held back, with the reason in words.
CREATE TABLE notifications (
  id              INTEGER PRIMARY KEY,
  at              INTEGER NOT NULL,
  kind            TEXT NOT NULL CHECK (kind IN ('watch', 'favourite', 'important', 'rule', 'routine', 'system')),
  title           TEXT NOT NULL,
  body            TEXT NOT NULL DEFAULT '',
  reason          TEXT NOT NULL,
  person_id       INTEGER,
  conversation_id INTEGER,
  watch_id        INTEGER,
  urgent          INTEGER NOT NULL DEFAULT 0 CHECK (urgent IN (0, 1)),
  -- 0 means it was held back; the reason says why.
  shown           INTEGER NOT NULL DEFAULT 1 CHECK (shown IN (0, 1)),
  opened_at       INTEGER
) STRICT;

CREATE INDEX notifications_by_time ON notifications (at DESC);

CREATE TABLE saved_views (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL,
  query      TEXT NOT NULL,
  position   INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL
) STRICT;
