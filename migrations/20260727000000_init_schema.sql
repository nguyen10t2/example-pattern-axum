-- Custom ENUM types
DO $$ BEGIN
    CREATE TYPE currency_code AS ENUM('USD', 'VND');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE group_role AS ENUM('OWNER', 'ADMIN', 'MEMBER');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE split_type AS ENUM('EQUAL', 'EXACT', 'PERCENTAGE');
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

-- 1. Users Table
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    full_name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    password_hash TEXT,
    google_id VARCHAR(255) UNIQUE,
    avatar_url TEXT,
    phone VARCHAR(16),
    phone_verified BOOLEAN NOT NULL DEFAULT false,
    preferred_currency currency_code NOT NULL DEFAULT 'VND',
    is_active BOOLEAN NOT NULL DEFAULT true,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_users_active ON users (is_active) WHERE is_active = true;
CREATE INDEX IF NOT EXISTS idx_users_deleted ON users (deleted_at) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_users_google_id ON users (google_id);

-- 2. Groups Table
CREATE TABLE IF NOT EXISTS groups (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    invite_code VARCHAR(10) UNIQUE,
    default_currency currency_code NOT NULL DEFAULT 'VND',
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_groups_deleted ON groups (deleted_at) WHERE deleted_at IS NULL;

-- 3. Group Members Table
CREATE TABLE IF NOT EXISTS group_members (
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE RESTRICT,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role group_role NOT NULL DEFAULT 'MEMBER',
    joined_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (group_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_group_members_user ON group_members (user_id);

-- 4. Expenses Table
CREATE TABLE IF NOT EXISTS expenses (
    id UUID PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE RESTRICT,
    created_by_id UUID NOT NULL REFERENCES users(id),
    payer_id UUID NOT NULL REFERENCES users(id),
    amount BIGINT NOT NULL,
    currency currency_code NOT NULL,
    description VARCHAR(255) NOT NULL,
    split_type split_type NOT NULL DEFAULT 'EQUAL',
    expense_date TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT amount_positive CHECK (amount > 0)
);

CREATE INDEX IF NOT EXISTS idx_expenses_group_created ON expenses (group_id, created_at);
CREATE INDEX IF NOT EXISTS idx_expenses_payer ON expenses (payer_id);
CREATE INDEX IF NOT EXISTS idx_expenses_active ON expenses (group_id) WHERE deleted_at IS NULL;

-- 5. Expense Shares Table
CREATE TABLE IF NOT EXISTS expense_shares (
    id UUID PRIMARY KEY,
    expense_id UUID NOT NULL REFERENCES expenses(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),
    share_amount BIGINT NOT NULL,
    share_percentage INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT share_amount_non_negative CHECK (share_amount >= 0),
    CONSTRAINT share_percentage_valid CHECK (share_percentage >= 0 AND share_percentage <= 10000)
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_expense_user ON expense_shares (expense_id, user_id);
CREATE INDEX IF NOT EXISTS idx_expense_shares_user ON expense_shares (user_id);

-- 6. Settlements Table
CREATE TABLE IF NOT EXISTS settlements (
    id UUID PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE RESTRICT,
    sender_id UUID NOT NULL REFERENCES users(id),
    receiver_id UUID NOT NULL REFERENCES users(id),
    amount BIGINT NOT NULL,
    currency currency_code NOT NULL,
    settled_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT settlement_amount_positive CHECK (amount > 0)
);

CREATE INDEX IF NOT EXISTS idx_settlements_group_settled ON settlements (group_id, settled_at);
CREATE INDEX IF NOT EXISTS idx_settlements_sender ON settlements (sender_id);
CREATE INDEX IF NOT EXISTS idx_settlements_receiver ON settlements (receiver_id);
CREATE INDEX IF NOT EXISTS idx_settlements_active ON settlements (group_id) WHERE deleted_at IS NULL;
