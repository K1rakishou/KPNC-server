CREATE TABLE IF NOT EXISTS account_tokens(
    id_generated            BIGSERIAL NOT NULL,
    owner_account_id        INT8 NOT NULL,
    firebase_token          VARCHAR(1024) DEFAULT NULL,
    application_type        INT2 NOT NULL,
    token_type              INT2 NOT NULL,
    PRIMARY KEY (id_generated)
);

CREATE UNIQUE INDEX owner_account_id_idx ON account_tokens (owner_account_id);
CREATE INDEX firebase_token_idx ON account_tokens (firebase_token);

ALTER TABLE public.accounts
    DROP COLUMN IF EXISTS firebase_token;