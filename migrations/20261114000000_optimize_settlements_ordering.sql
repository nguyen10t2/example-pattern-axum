-- Align settlements pagination with the query plan (ORDER BY settled_at DESC, filtered by group_id AND deleted_at IS NULL).
-- idx_settlements_active (group_id) only filters and forces an in-memory sort; mirror the expenses fix with a
-- partial composite index on (group_id, settled_at).

DROP INDEX IF EXISTS idx_settlements_active;

CREATE INDEX IF NOT EXISTS idx_settlements_group_settled_active
    ON settlements (group_id, settled_at)
    WHERE deleted_at IS NULL;
