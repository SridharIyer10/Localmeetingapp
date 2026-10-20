-- Intake Co-Pilot schema (Chunk 1)
--
-- Coverage (meeting_questions.status) and quality (meeting_questions.rating) are
-- orthogonal. `rating` is NULL unless `status = 'answered'`. Multi-value fields
-- are JSON arrays; they are never comma-joined strings.

ALTER TABLE meetings ADD COLUMN meeting_mode TEXT NOT NULL DEFAULT 'plain';
ALTER TABLE meetings ADD COLUMN consent_recorded_at TEXT;
ALTER TABLE meetings ADD COLUMN recording_destroyed_at TEXT;
ALTER TABLE meetings ADD COLUMN transcription_completed_at TEXT;
ALTER TABLE meetings ADD COLUMN processing_stage TEXT;
ALTER TABLE meetings ADD COLUMN analysis_model TEXT;
ALTER TABLE meetings ADD COLUMN analysis_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE meetings ADD COLUMN action_points_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE meetings ADD COLUMN next_steps_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE meetings ADD COLUMN actions_discussed_json TEXT NOT NULL DEFAULT '[]';

CREATE TABLE IF NOT EXISTS transcript_segments (
    id            TEXT PRIMARY KEY,
    meeting_id    TEXT NOT NULL,
    seq           INTEGER NOT NULL,
    start_ms      INTEGER NOT NULL,
    end_ms        INTEGER NOT NULL,
    channel       INTEGER NOT NULL,
    speaker_label TEXT,
    text          TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_segments_meeting ON transcript_segments(meeting_id, seq);

CREATE TABLE IF NOT EXISTS meeting_questions (
    id                    TEXT PRIMARY KEY,
    meeting_id            TEXT NOT NULL,
    sort_order            INTEGER NOT NULL,
    main_question         TEXT NOT NULL,
    sample_questions_json TEXT NOT NULL DEFAULT '[]',
    sample_answers_json   TEXT NOT NULL DEFAULT '[]',
    status                TEXT NOT NULL DEFAULT 'not_asked',
    rating                TEXT DEFAULT NULL,
    extracted_answer      TEXT DEFAULT NULL,
    missing_details       TEXT DEFAULT NULL,
    confidence            REAL DEFAULT NULL,
    evidence_segment_ids  TEXT NOT NULL DEFAULT '[]',
    asked_timestamp_ms    INTEGER DEFAULT NULL,
    user_ticked           INTEGER NOT NULL DEFAULT 0,
    user_notes            TEXT DEFAULT NULL,
    manually_overridden   INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    CHECK (status IN ('not_asked', 'asked_not_answered', 'answered')),
    CHECK (rating IS NULL OR rating IN ('Poor', 'Partial', 'Strong')),
    CHECK (rating IS NULL OR status = 'answered')
);
CREATE INDEX IF NOT EXISTS idx_questions_meeting ON meeting_questions(meeting_id, sort_order);

-- No api key columns. Keys live in the OS secret store (Keychain / DPAPI).
CREATE TABLE IF NOT EXISTS ai_settings (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    provider_mode      TEXT NOT NULL DEFAULT 'embedded_local',
    embedded_llm_model TEXT DEFAULT 'qwen2.5-3b-instruct-q4_k_m.gguf',
    embedded_stt_model TEXT DEFAULT 'ggml-small.en-q5_1.bin',
    local_base_url     TEXT DEFAULT 'http://localhost:11434/v1',
    local_model_name   TEXT DEFAULT '',
    cloud_provider     TEXT DEFAULT 'openai',
    cloud_model_name   TEXT DEFAULT 'gpt-4o-mini',
    CHECK (provider_mode IN ('embedded_local', 'local_server', 'cloud'))
);

INSERT OR IGNORE INTO ai_settings (id) VALUES (1);
