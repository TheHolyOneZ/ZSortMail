-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0009_notification_mail: the log also records mail that did not notify, so it needs a kind of its own.

CREATE TABLE notifications_new (
  id              INTEGER PRIMARY KEY,
  at              INTEGER NOT NULL,
  kind            TEXT NOT NULL CHECK (kind IN ('watch', 'favourite', 'important', 'rule', 'routine', 'mail', 'system')),
  title           TEXT NOT NULL,
  body            TEXT NOT NULL DEFAULT '',
  reason          TEXT NOT NULL,
  person_id       INTEGER,
  conversation_id INTEGER,
  watch_id        INTEGER,
  urgent          INTEGER NOT NULL DEFAULT 0 CHECK (urgent IN (0, 1)),
  shown           INTEGER NOT NULL DEFAULT 1 CHECK (shown IN (0, 1)),
  opened_at       INTEGER,
  draft_id        INTEGER
) STRICT;

INSERT INTO notifications_new (id, at, kind, title, body, reason, person_id, conversation_id, watch_id, urgent, shown, opened_at, draft_id)
  SELECT id, at, kind, title, body, reason, person_id, conversation_id, watch_id, urgent, shown, opened_at, draft_id FROM notifications;

DROP TABLE notifications;
ALTER TABLE notifications_new RENAME TO notifications;
CREATE INDEX notifications_by_time ON notifications (at DESC);
