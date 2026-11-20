-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0002_state: the state a person can put people and conversations into, other than
-- "read": archived, trashed, starred, snoozed, hidden, muted, very important.

-- A hidden person disappears from lists, search and notifications. `hidden_until` brings
-- them back on its own at that time.
ALTER TABLE people ADD COLUMN hidden_at INTEGER;
ALTER TABLE people ADD COLUMN hidden_until INTEGER;
ALTER TABLE people ADD COLUMN muted INTEGER NOT NULL DEFAULT 0 CHECK (muted IN (0, 1));
ALTER TABLE people ADD COLUMN vip INTEGER NOT NULL DEFAULT 0 CHECK (vip IN (0, 1));

-- in_inbox, trashed and starred are derived from message labels by recompute_conversation,
-- so changes that arrive from Gmail and changes made here end up in the same place.
ALTER TABLE conversations ADD COLUMN in_inbox INTEGER NOT NULL DEFAULT 0 CHECK (in_inbox IN (0, 1));
ALTER TABLE conversations ADD COLUMN trashed INTEGER NOT NULL DEFAULT 0 CHECK (trashed IN (0, 1));
ALTER TABLE conversations ADD COLUMN starred INTEGER NOT NULL DEFAULT 0 CHECK (starred IN (0, 1));
ALTER TABLE conversations ADD COLUMN snoozed_until INTEGER;
-- Why a conversation is hidden: 'manual', 'person:<id>', 'domain:<name>' or 'rule:<id>'.
ALTER TABLE conversations ADD COLUMN hidden_reason TEXT;
ALTER TABLE conversations ADD COLUMN hidden_until INTEGER;

CREATE INDEX conversations_inbox
  ON conversations (account_id, last_message_at DESC)
  WHERE in_inbox = 1 AND trashed = 0 AND hidden_at IS NULL;
