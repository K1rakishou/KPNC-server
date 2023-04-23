CREATE TABLE IF NOT EXISTS logs
(
    id BIGSERIAL NOT NULL,
    log_time TIMESTAMP WITH TIME ZONE NOT NULL,
    log_level VARCHAR(8),
    target VARCHAR,
    message VARCHAR NOT NULL
);

CREATE INDEX logs_log_time_idx ON logs (log_time);