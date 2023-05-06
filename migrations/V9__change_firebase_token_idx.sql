DROP INDEX IF EXISTS firebase_token_idx;

CREATE INDEX token_idx ON account_tokens (token);
CREATE UNIQUE INDEX unique_token_idx ON account_tokens (token, application_type, token_type);
