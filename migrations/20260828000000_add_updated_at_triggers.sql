-- Auto-bump `updated_at` on every UPDATE via a single shared trigger function.
-- Centralizes the "timestamp on write" pattern so repositories don't have to
-- remember to set `updated_at = now()` (and so soft-deletes, which only touch
-- `deleted_at`, also refresh `updated_at`).

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_users_updated_at BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_groups_updated_at BEFORE UPDATE ON groups
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_expenses_updated_at BEFORE UPDATE ON expenses
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_expense_shares_updated_at BEFORE UPDATE ON expense_shares
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER trg_settlements_updated_at BEFORE UPDATE ON settlements
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
