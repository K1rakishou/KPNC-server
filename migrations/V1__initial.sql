CREATE TABLE IF NOT EXISTS accounts(
    account_id VARCHAR(128),
    firebase_token VARCHAR(1024),
    valid_until TIMESTAMP WITH TIME ZONE DEFAULT NULL
);

CREATE TABLE IF NOT EXISTS migrations(
    version INTEGER,
    name VARCHAR(256),
    applied_on TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    checksum VARCHAR(512)
);
