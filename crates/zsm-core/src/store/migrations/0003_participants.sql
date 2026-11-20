-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0003_participants: keep the text of the people on a message next to it, as the search index
-- saw it. The full-text index cannot be read back, and rules and watches must see exactly what
-- search sees.
ALTER TABLE messages ADD COLUMN participants TEXT NOT NULL DEFAULT '';
