-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0008_bcc: remember who a message was blind-copied to, so the details of a sent message can show it.

ALTER TABLE messages ADD COLUMN bcc_addresses TEXT NOT NULL DEFAULT '[]';
