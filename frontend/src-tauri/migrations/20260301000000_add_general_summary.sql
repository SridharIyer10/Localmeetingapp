-- General meeting summary produced by the analysis pass, independent of any
-- question set (always present, even when no question was asked).

ALTER TABLE meetings ADD COLUMN general_summary TEXT;
