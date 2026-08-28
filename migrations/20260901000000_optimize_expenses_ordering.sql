-- Align expenses ordering with the query plan (ORDER BY expense_date DESC, filtered by group_id AND deleted_at IS NULL).
-- The previous idx_expenses_group_created was on (group_id, created_at) which did not match the ORDER BY,
-- forcing an in-memory sort; idx_expenses_active (group_id) is subsumed by the new composite index.

DROP INDEX IF EXISTS idx_expenses_active;
DROP INDEX IF EXISTS idx_expenses_group_created;

CREATE INDEX IF NOT EXISTS idx_expenses_group_expense_date
    ON expenses (group_id, expense_date)
    WHERE deleted_at IS NULL;
