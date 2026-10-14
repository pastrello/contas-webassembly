-- Run with a dedicated migration/admin connection. The web process never uses it.
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'contas_app') THEN
    CREATE ROLE contas_app NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOBYPASSRLS;
  END IF;
END $$;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;

CREATE TABLE users (
  id uuid PRIMARY KEY,
  email text NOT NULL UNIQUE CHECK (email = lower(email)),
  display_name text NOT NULL CHECK (char_length(display_name) BETWEEN 1 AND 100),
  password_hash text NOT NULL,
  active boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE tenants (
  id uuid PRIMARY KEY,
  name text NOT NULL CHECK (char_length(name) BETWEEN 1 AND 100),
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE memberships (
  tenant_id uuid NOT NULL REFERENCES tenants(id),
  user_id uuid NOT NULL REFERENCES users(id),
  role text NOT NULL CHECK (role IN ('owner','viewer')),
  PRIMARY KEY (tenant_id, user_id)
);
CREATE TABLE tenant_settings (
  tenant_id uuid PRIMARY KEY REFERENCES tenants(id),
  currency text NOT NULL DEFAULT 'BRL' CHECK (currency = 'BRL'),
  updated_by uuid NOT NULL,
  FOREIGN KEY (tenant_id, updated_by) REFERENCES memberships(tenant_id, user_id)
);
CREATE TABLE sessions (
  token_hash text PRIMARY KEY CHECK (length(token_hash) = 64),
  tenant_id uuid NOT NULL,
  user_id uuid NOT NULL,
  expires_at timestamptz NOT NULL DEFAULT now() + interval '12 hours',
  FOREIGN KEY (tenant_id,user_id) REFERENCES memberships(tenant_id,user_id) ON DELETE CASCADE
);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE INDEX memberships_user ON memberships(user_id);
CREATE TABLE auth_attempts (
  bucket text PRIMARY KEY,
  attempts integer NOT NULL,
  expires_at timestamptz NOT NULL
);

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tenants
  USING (id = nullif(current_setting('app.tenant_id', true), '')::uuid)
  WITH CHECK (id = nullif(current_setting('app.tenant_id', true), '')::uuid);
ALTER TABLE memberships ENABLE ROW LEVEL SECURITY;
ALTER TABLE memberships FORCE ROW LEVEL SECURITY;
-- An identity can discover its own memberships at login, before tenant selection.
CREATE POLICY membership_read ON memberships FOR SELECT
  USING (user_id = nullif(current_setting('app.user_id', true), '')::uuid
      OR tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
CREATE POLICY membership_insert ON memberships FOR INSERT
  WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);
ALTER TABLE tenant_settings ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenant_settings FORCE ROW LEVEL SECURITY;
CREATE POLICY settings_isolation ON tenant_settings
  USING (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_id = nullif(current_setting('app.tenant_id', true), '')::uuid);

GRANT USAGE ON SCHEMA public TO contas_app;
GRANT SELECT, INSERT ON users TO contas_app;
GRANT SELECT, INSERT, UPDATE ON tenants, tenant_settings TO contas_app;
GRANT SELECT, INSERT ON memberships TO contas_app;
GRANT SELECT, INSERT, DELETE ON sessions TO contas_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON auth_attempts TO contas_app;
