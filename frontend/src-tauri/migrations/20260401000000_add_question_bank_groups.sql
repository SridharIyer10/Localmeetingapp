-- Named question banks ("headers"). Each bank groups reusable questions
-- (rows of `question_bank`), so several banks can coexist and be picked from
-- when starting a co-pilot meeting.

CREATE TABLE IF NOT EXISTS question_bank_groups (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
-- Header names are unique, ignoring case.
CREATE UNIQUE INDEX IF NOT EXISTS idx_question_bank_groups_name
    ON question_bank_groups(lower(name));

ALTER TABLE question_bank ADD COLUMN bank_id TEXT REFERENCES question_bank_groups(id);
CREATE INDEX IF NOT EXISTS idx_question_bank_bank ON question_bank(bank_id);

-- Questions saved before banks existed move into a default "General" bank.
INSERT INTO question_bank_groups (id, name, created_at, updated_at)
SELECT 'bank-default', 'General', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE EXISTS (SELECT 1 FROM question_bank);
UPDATE question_bank SET bank_id = 'bank-default' WHERE bank_id IS NULL;
