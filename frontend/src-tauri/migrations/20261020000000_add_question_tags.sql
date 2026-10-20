-- Co-pilot: manual question tags recorded by the interviewer during the call.
--
-- `user_tag` is user input only (never computed, never a verdict). It is stored on
-- the meeting's own question rows, so it is saved with the session from the first
-- tap (draft meeting) and follows the questions when the recording is attached.
-- NULL = not tagged. Editable until the question summary has been generated.
--
-- `tag_note` is written by the post-call analysis: "Not explicitly tagged." for
-- untagged questions, or the discrepancy when the transcript contradicts the tag.
--
-- Additive only: existing migrations are checksummed in users' databases.

ALTER TABLE meeting_questions ADD COLUMN user_tag TEXT DEFAULT NULL
    CHECK (user_tag IS NULL OR user_tag IN ('asked', 'not_asked', 'answered'));
ALTER TABLE meeting_questions ADD COLUMN tag_note TEXT DEFAULT NULL;
