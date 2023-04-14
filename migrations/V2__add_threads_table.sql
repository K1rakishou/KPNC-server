CREATE TABLE IF NOT EXISTS threads(
    id_generated                    BIGSERIAL NOT NULL,
    site_name                       VARCHAR(128) NOT NULL,
    board_code                      VARCHAR(64) NOT NULL,
    thread_no                       INT8 NOT NULL,
    last_processed_post_no          INT8 DEFAULT NULL,
    last_processed_post_sub_no      INT8 DEFAULT NULL,
    created_on                      TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    deleted_on                      TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    PRIMARY KEY (id_generated)
);

CREATE UNIQUE INDEX threads_unique_id_idx ON threads (site_name, board_code, thread_no);
