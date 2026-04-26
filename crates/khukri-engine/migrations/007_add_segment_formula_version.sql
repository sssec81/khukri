-- Tracks which segmentation formula version was used to create the stored segments.
-- NULL means the download pre-dates this column; those segments are not safe to resume
-- if the formula has changed (resolved_thread_count logic).
ALTER TABLE downloads ADD COLUMN segment_formula_version INTEGER;
