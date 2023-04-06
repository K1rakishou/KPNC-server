CREATE TABLE IF NOT EXISTS users(
    user_id VARCHAR(128),
    firebase_token VARCHAR(1024),
    valid_until TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc')
);

CREATE TABLE IF NOT EXISTS migrations(
    version INTEGER,
    name VARCHAR(256),
    applied_on TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc'),
    checksum VARCHAR(512)
);
