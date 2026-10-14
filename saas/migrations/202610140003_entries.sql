CREATE TABLE entries (
 id uuid PRIMARY KEY,
 sequence bigserial UNIQUE NOT NULL,
 tenant_id uuid NOT NULL REFERENCES tenants(id),
 account_id uuid NOT NULL,
 history_id uuid NOT NULL,
 entry_date date NOT NULL CHECK(entry_date BETWEEN DATE '0001-01-01' AND DATE '9999-12-31'),
 document text NOT NULL CHECK(char_length(document)<=60),
 description text NOT NULL CHECK(char_length(description)<=500),
 amount_cents bigint NOT NULL CHECK(amount_cents BETWEEN 1 AND 9000000000000),
 direction smallint NOT NULL CHECK(direction IN (-1,1)),
 cleared boolean NOT NULL DEFAULT false,
 archived boolean NOT NULL DEFAULT false,
 cancelled boolean NOT NULL DEFAULT false,
 version bigint NOT NULL DEFAULT 1 CHECK(version>0),
 request_id uuid NOT NULL,
 request_payload jsonb NOT NULL,
 updated_by uuid NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now(),
 updated_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(tenant_id,request_id),
 FOREIGN KEY(tenant_id,account_id) REFERENCES accounts(tenant_id,id),
 FOREIGN KEY(tenant_id,history_id) REFERENCES histories(tenant_id,id),
 FOREIGN KEY(tenant_id,updated_by) REFERENCES memberships(tenant_id,user_id)
);
CREATE INDEX entries_account_date ON entries(tenant_id,account_id,entry_date,sequence);
CREATE INDEX entries_history ON entries(tenant_id,history_id);
ALTER TABLE entries ENABLE ROW LEVEL SECURITY;
ALTER TABLE entries FORCE ROW LEVEL SECURITY;
CREATE POLICY entries_isolation ON entries
 USING(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid)
 WITH CHECK(tenant_id=nullif(current_setting('app.tenant_id',true),'')::uuid);
GRANT SELECT,INSERT,UPDATE ON entries TO contas_app;
GRANT USAGE ON SEQUENCE entries_sequence_seq TO contas_app;
