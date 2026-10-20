-- Reusable question bank for co-pilot meetings.
--
-- Questions entered once can be selected for future meetings. Multi-value fields
-- are JSON arrays (never comma-joined).

CREATE TABLE IF NOT EXISTS question_bank (
    id                    TEXT PRIMARY KEY,
    question              TEXT NOT NULL,
    sample_questions_json TEXT NOT NULL DEFAULT '[]',
    sample_answers_json   TEXT NOT NULL DEFAULT '[]',
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_question_bank_created ON question_bank(created_at DESC);
