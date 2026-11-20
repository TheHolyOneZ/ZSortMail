-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0006_writing: drafts and their files, and routines (emails that come round on a schedule).

CREATE TABLE drafts (
  id                     INTEGER PRIMARY KEY,
  account_id             INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  -- What this draft answers or forwards.
  conversation_id        INTEGER REFERENCES conversations (id) ON DELETE SET NULL,
  in_reply_to_message_id INTEGER,
  mode                   TEXT NOT NULL DEFAULT 'new' CHECK (mode IN ('new', 'reply', 'reply_all', 'forward')),
  to_addresses           TEXT NOT NULL DEFAULT '[]',
  cc_addresses           TEXT NOT NULL DEFAULT '[]',
  bcc_addresses          TEXT NOT NULL DEFAULT '[]',
  subject                TEXT NOT NULL DEFAULT '',
  body_html              TEXT NOT NULL DEFAULT '',
  body_text              TEXT NOT NULL DEFAULT '',
  routine_id             INTEGER,
  -- When to send it by itself (unix milliseconds); null while it is only a draft.
  send_at                INTEGER,
  -- Remind me if nobody has replied after this many days.
  nudge_days             INTEGER CHECK (nudge_days IS NULL OR nudge_days > 0),
  -- 0 until someone writes in it: an untouched draft is not worth keeping.
  touched                INTEGER NOT NULL DEFAULT 0 CHECK (touched IN (0, 1)),
  created_at             INTEGER NOT NULL,
  updated_at             INTEGER NOT NULL
) STRICT;

CREATE INDEX drafts_by_update ON drafts (updated_at DESC);
CREATE INDEX drafts_by_send ON drafts (send_at) WHERE send_at IS NOT NULL;

CREATE TABLE draft_attachments (
  id         INTEGER PRIMARY KEY,
  draft_id   INTEGER NOT NULL REFERENCES drafts (id) ON DELETE CASCADE,
  filename   TEXT NOT NULL,
  mime_type  TEXT NOT NULL,
  size_bytes INTEGER NOT NULL,
  path       TEXT NOT NULL
) STRICT;

CREATE INDEX draft_attachments_by_draft ON draft_attachments (draft_id);

-- An email that comes round again: every Monday, the first of the month, or whenever you ask.
CREATE TABLE routines (
  id           INTEGER PRIMARY KEY,
  name         TEXT NOT NULL,
  enabled      INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  account_id   INTEGER REFERENCES accounts (id) ON DELETE CASCADE,
  to_addresses TEXT NOT NULL DEFAULT '[]',
  cc_addresses TEXT NOT NULL DEFAULT '[]',
  subject      TEXT NOT NULL,
  -- Plain text with {{date}}, {{weekday}} and similar variables.
  body         TEXT NOT NULL,
  -- JSON: {"kind": "daily|weekdays|weekly|monthly|manual", "time": "09:00", "days": [1], "day_of_month": 1}
  recurrence   TEXT NOT NULL,
  -- draft: prepare it and tell me. send: send it by itself.
  mode         TEXT NOT NULL DEFAULT 'draft' CHECK (mode IN ('draft', 'send')),
  next_run_at  INTEGER,
  last_run_at  INTEGER,
  streak       INTEGER NOT NULL DEFAULT 0,
  skip_next    INTEGER NOT NULL DEFAULT 0 CHECK (skip_next IN (0, 1)),
  created_at   INTEGER NOT NULL
) STRICT;

CREATE TABLE routine_runs (
  id         INTEGER PRIMARY KEY,
  routine_id INTEGER NOT NULL REFERENCES routines (id) ON DELETE CASCADE,
  due_at     INTEGER NOT NULL,
  ran_at     INTEGER NOT NULL,
  result     TEXT NOT NULL CHECK (result IN ('prepared', 'sent', 'skipped')),
  -- It ran well after its time (the computer was off or asleep), once, instead of several times.
  late       INTEGER NOT NULL DEFAULT 0 CHECK (late IN (0, 1)),
  draft_id   INTEGER
) STRICT;

CREATE INDEX routine_runs_by_routine ON routine_runs (routine_id, due_at DESC);

-- "Remind me if there is no reply": one row per message sent with a reminder.
CREATE TABLE nudges (
  id              INTEGER PRIMARY KEY,
  message_id      INTEGER NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
  conversation_id INTEGER NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
  due_at          INTEGER NOT NULL,
  state           TEXT NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'replied', 'fired', 'cancelled')),
  created_at      INTEGER NOT NULL
) STRICT;

CREATE INDEX nudges_due ON nudges (due_at) WHERE state = 'waiting';
CREATE INDEX nudges_by_conversation ON nudges (conversation_id);

-- A notification can lead to a draft (a routine that is ready) instead of a conversation.
ALTER TABLE notifications ADD COLUMN draft_id INTEGER;
