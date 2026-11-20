-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0001_init: accounts, people, conversations, messages, attachments, labels, outbox,
-- settings and full-text search.
--
-- Times are unix epoch milliseconds (INTEGER). JSON columns hold UTF-8 JSON text.
-- Body columns hold zstd-compressed bytes. Every row that belongs to a mailbox carries
-- account_id, so more than one account is a UI change later, not a schema change.

CREATE TABLE accounts (
  id           INTEGER PRIMARY KEY,
  provider     TEXT NOT NULL CHECK (provider IN ('gmail', 'demo')),
  email        TEXT NOT NULL COLLATE NOCASE UNIQUE,
  display_name TEXT,
  created_at   INTEGER NOT NULL
) STRICT;

CREATE TABLE sync_state (
  account_id      INTEGER PRIMARY KEY REFERENCES accounts (id) ON DELETE CASCADE,
  history_id      TEXT,
  backfill_cursor TEXT,
  backfill_done   INTEGER NOT NULL DEFAULT 0 CHECK (backfill_done IN (0, 1)),
  last_sync_at    INTEGER
) STRICT;

CREATE TABLE people (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  kind            TEXT NOT NULL DEFAULT 'person'
                    CHECK (kind IN ('person', 'service', 'list', 'self')),
  status          TEXT NOT NULL DEFAULT 'known'
                    CHECK (status IN ('known', 'new_sender', 'blocked')),
  display_name    TEXT NOT NULL,
  title           TEXT,
  description     TEXT,
  organization    TEXT,
  ink             INTEGER NOT NULL DEFAULT 0 CHECK (ink BETWEEN 0 AND 9),
  pinned          INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0, 1)),
  whitelisted     INTEGER NOT NULL DEFAULT 0 CHECK (whitelisted IN (0, 1)),
  cadence_days    INTEGER CHECK (cadence_days IS NULL OR cadence_days > 0),
  photo_path      TEXT,
  notes           TEXT,
  created_at      INTEGER NOT NULL,
  -- Denormalized from conversations by Store::refresh_person.
  first_message_at  INTEGER,
  last_message_at   INTEGER,
  last_subject      TEXT NOT NULL DEFAULT '',
  unread_count      INTEGER NOT NULL DEFAULT 0,
  needs_reply       INTEGER NOT NULL DEFAULT 0 CHECK (needs_reply IN (0, 1)),
  waiting_on_them   INTEGER NOT NULL DEFAULT 0 CHECK (waiting_on_them IN (0, 1)),
  has_files         INTEGER NOT NULL DEFAULT 0 CHECK (has_files IN (0, 1)),
  conversation_count INTEGER NOT NULL DEFAULT 0,
  file_count        INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX people_by_activity ON people (account_id, status, last_message_at DESC);

CREATE TABLE person_addresses (
  account_id INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  address    TEXT NOT NULL COLLATE NOCASE,
  person_id  INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
  PRIMARY KEY (account_id, address)
) STRICT, WITHOUT ROWID;

CREATE INDEX person_addresses_by_person ON person_addresses (person_id);

CREATE TABLE conversations (
  id               INTEGER PRIMARY KEY,
  account_id       INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  provider_id      TEXT NOT NULL,
  subject          TEXT NOT NULL,
  snippet          TEXT NOT NULL DEFAULT '',
  first_message_at INTEGER NOT NULL,
  last_message_at  INTEGER NOT NULL,
  message_count    INTEGER NOT NULL DEFAULT 0,
  unread_count     INTEGER NOT NULL DEFAULT 0,
  has_attachments  INTEGER NOT NULL DEFAULT 0 CHECK (has_attachments IN (0, 1)),
  last_from_me     INTEGER NOT NULL DEFAULT 0 CHECK (last_from_me IN (0, 1)),
  category         TEXT NOT NULL DEFAULT 'person'
                     CHECK (category IN ('person', 'newsletter', 'receipt', 'automated')),
  hidden_at        INTEGER,
  UNIQUE (account_id, provider_id)
) STRICT;

CREATE INDEX conversations_by_activity
  ON conversations (account_id, last_message_at DESC)
  WHERE hidden_at IS NULL;

-- A group thread belongs to every participant, so conversation to person is many to many.
CREATE TABLE conversation_people (
  conversation_id INTEGER NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
  person_id       INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
  PRIMARY KEY (conversation_id, person_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX conversation_people_by_person ON conversation_people (person_id, conversation_id);

CREATE TABLE messages (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  conversation_id INTEGER NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
  provider_id     TEXT NOT NULL,
  rfc_message_id  TEXT,
  in_reply_to     TEXT,
  from_person_id  INTEGER REFERENCES people (id) ON DELETE SET NULL,
  from_address    TEXT NOT NULL COLLATE NOCASE,
  to_addresses    TEXT NOT NULL DEFAULT '[]',
  cc_addresses    TEXT NOT NULL DEFAULT '[]',
  subject         TEXT NOT NULL,
  snippet         TEXT NOT NULL DEFAULT '',
  sent_at         INTEGER NOT NULL,
  is_from_me      INTEGER NOT NULL DEFAULT 0 CHECK (is_from_me IN (0, 1)),
  unread          INTEGER NOT NULL DEFAULT 0 CHECK (unread IN (0, 1)),
  starred         INTEGER NOT NULL DEFAULT 0 CHECK (starred IN (0, 1)),
  size_bytes      INTEGER,
  body_state      TEXT NOT NULL DEFAULT 'none'
                    CHECK (body_state IN ('none', 'metadata', 'full')),
  body_text       BLOB,
  body_quoted     BLOB,
  body_html       BLOB,
  remote_images   INTEGER NOT NULL DEFAULT 0,
  trackers_blocked INTEGER NOT NULL DEFAULT 0,
  list_id         TEXT,
  unsubscribe     TEXT,
  category        TEXT NOT NULL DEFAULT 'person'
                    CHECK (category IN ('person', 'newsletter', 'receipt', 'automated')),
  hidden_at       INTEGER,
  UNIQUE (account_id, provider_id)
) STRICT;

CREATE INDEX messages_by_conversation ON messages (conversation_id, sent_at);
CREATE INDEX messages_by_sender ON messages (from_person_id, sent_at DESC);

CREATE TABLE attachments (
  id          INTEGER PRIMARY KEY,
  message_id  INTEGER NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
  provider_id TEXT,
  filename    TEXT NOT NULL,
  mime_type   TEXT NOT NULL,
  size_bytes  INTEGER NOT NULL,
  content_id  TEXT,
  inline      INTEGER NOT NULL DEFAULT 0 CHECK (inline IN (0, 1)),
  cache_path  TEXT,
  sha256      TEXT
) STRICT;

CREATE INDEX attachments_by_message ON attachments (message_id);

CREATE TABLE labels (
  id          INTEGER PRIMARY KEY,
  account_id  INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL,
  name        TEXT NOT NULL,
  kind        TEXT NOT NULL DEFAULT 'user' CHECK (kind IN ('system', 'user')),
  UNIQUE (account_id, provider_id)
) STRICT;

CREATE TABLE message_labels (
  message_id INTEGER NOT NULL REFERENCES messages (id) ON DELETE CASCADE,
  label_id   INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
  PRIMARY KEY (message_id, label_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX message_labels_by_label ON message_labels (label_id, message_id);

-- Writes the UI has made that Gmail has not seen yet. Drained by the sync worker.
CREATE TABLE outbox (
  id              INTEGER PRIMARY KEY,
  account_id      INTEGER NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
  op              TEXT NOT NULL,
  payload         TEXT NOT NULL,
  attempts        INTEGER NOT NULL DEFAULT 0,
  next_attempt_at INTEGER NOT NULL,
  last_error      TEXT,
  created_at      INTEGER NOT NULL
) STRICT;

CREATE INDEX outbox_due ON outbox (next_attempt_at);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
) STRICT, WITHOUT ROWID;

-- Full-text search. Contentless: the index only, so bodies are not stored twice.
-- The rowid equals messages.id / people.id. contentless_delete lets rows be removed
-- without supplying the original text.
CREATE VIRTUAL TABLE messages_fts USING fts5 (
  subject,
  body,
  participants,
  content = '',
  contentless_delete = 1,
  tokenize = 'unicode61 remove_diacritics 2',
  prefix = '2 3 4'
);

CREATE VIRTUAL TABLE people_fts USING fts5 (
  name,
  addresses,
  organization,
  title,
  subject,
  content = '',
  contentless_delete = 1,
  tokenize = 'unicode61 remove_diacritics 2',
  prefix = '1 2 3'
);
