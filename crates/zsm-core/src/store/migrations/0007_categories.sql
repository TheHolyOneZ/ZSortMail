-- SPDX-License-Identifier: GPL-3.0-or-later
-- 0007_categories: groups become categories people are sorted into (Doctor, Friends, ...), each
-- with an icon, a picture, a description, and a name of its own for each member.

ALTER TABLE groups ADD COLUMN icon TEXT NOT NULL DEFAULT 'users';
ALTER TABLE groups ADD COLUMN description TEXT NOT NULL DEFAULT '';
-- A small picture as a data URL, resized by the interface before it is stored.
ALTER TABLE groups ADD COLUMN image TEXT;
-- Shown in the sidebar under People.
ALTER TABLE groups ADD COLUMN sidebar INTEGER NOT NULL DEFAULT 0 CHECK (sidebar IN (0, 1));
ALTER TABLE groups ADD COLUMN position INTEGER NOT NULL DEFAULT 0;

-- The name this category uses for this person, when it differs ("Dr. Fischer" for the dentist).
ALTER TABLE person_groups ADD COLUMN alias TEXT;
