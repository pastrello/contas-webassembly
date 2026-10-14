CREATE TABLE accounts (
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL REFERENCES tenants(id),
  code text NOT NULL CHECK (code ~ '^[A-Z0-9_.-]{1,20}$'),
  number text NOT NULL DEFAULT '' CHECK (char_length(number) <= 40),
  name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100 AND btrim(name) <> ''),
  opening_balance_cents bigint NOT NULL CHECK (opening_balance_cents BETWEEN -9000000000000 AND 9000000000000),
  active boolean NOT NULL DEFAULT true,
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  updated_by uuid NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(tenant_id,code),
  UNIQUE(tenant_id,id),
  FOREIGN KEY(tenant_id,updated_by) REFERENCES memberships(tenant_id,user_id)
);
CREATE TABLE histories (
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL REFERENCES tenants(id),
  code text NOT NULL CHECK (code ~ '^[A-Z0-9_.-]{1,20}$'),
  description text NOT NULL CHECK (char_length(description) BETWEEN 1 AND 100 AND btrim(description) <> ''),
  direction smallint NOT NULL CHECK (direction IN (-1,1)),
  active boolean NOT NULL DEFAULT true,
  version bigint NOT NULL DEFAULT 1 CHECK (version > 0),
  updated_by uuid NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE(tenant_id,code),
  UNIQUE(tenant_id,id),
  FOREIGN KEY(tenant_id,updated_by) REFERENCES memberships(tenant_id,user_id)
);
ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounts FORCE ROW LEVEL SECURITY;
CREATE POLICY accounts_isolation ON accounts
  USING (tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid)
  WITH CHECK (tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
ALTER TABLE histories ENABLE ROW LEVEL SECURITY;
ALTER TABLE histories FORCE ROW LEVEL SECURITY;
CREATE POLICY histories_isolation ON histories
  USING (tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid)
  WITH CHECK (tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
GRANT SELECT,INSERT,UPDATE ON accounts,histories TO contas_app;
-- No DELETE grant: deactivate/reactivate while preserving identity for future entries.
