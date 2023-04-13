CREATE TABLE IF NOT EXISTS migrations
(
    version                 INTEGER,
    name                    VARCHAR(256),
    applied_on              TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    checksum                VARCHAR(512),
    PRIMARY KEY (version)
);

CREATE TABLE IF NOT EXISTS accounts
(
    id_generated            BIGSERIAL NOT NULL,
    account_id              VARCHAR(128),
    firebase_token          VARCHAR(1024) DEFAULT NULL,
    valid_until             TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    created_on              TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    deleted_on              TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    PRIMARY KEY (account_id)
);

CREATE UNIQUE INDEX accounts_id_generated ON accounts (id_generated);
CREATE INDEX accounts_created_on_idx ON accounts (created_on);
CREATE INDEX accounts_deleted_on_idx ON accounts (deleted_on);

CREATE TABLE IF NOT EXISTS post_descriptors
(
    id_generated            BIGSERIAL NOT NULL,
    site_name               VARCHAR(128) NOT NULL,
    board_code              VARCHAR(64) NOT NULL,
    thread_no               INT8 NOT NULL,
    post_no                 INT8 NOT NULL,
    post_sub_no             INT8 DEFAULT NULL,
    PRIMARY KEY (id_generated)
);

CREATE UNIQUE INDEX post_descriptors_id_generated_idx
    ON post_descriptors (site_name, board_code, thread_no, post_no, post_sub_no);

CREATE TABLE IF NOT EXISTS posts
(
    id_generated                BIGSERIAL NOT NULL,
    owner_post_descriptor_id    INT8 NOT NULL,
    is_dead                     BOOLEAN DEFAULT FALSE,
    created_on                  TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    deleted_on                  TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    CONSTRAINT fk_owner_post_descriptor_id FOREIGN KEY (owner_post_descriptor_id)
        REFERENCES post_descriptors (id_generated) ON UPDATE CASCADE ON DELETE CASCADE,
    PRIMARY KEY (id_generated)
);

CREATE UNIQUE INDEX owner_post_descriptor_id_idx ON posts (owner_post_descriptor_id);
CREATE INDEX posts_created_on_idx ON posts (created_on);
CREATE INDEX posts_deleted_on_idx ON posts (deleted_on);

CREATE TABLE IF NOT EXISTS watches
(
    id_generated        BIGSERIAL NOT NULL,
    owner_post_id       INT8 NOT NULL,
    owner_account_id    INT8 NOT NULL,
    PRIMARY KEY (id_generated),
    CONSTRAINT fk_post_id FOREIGN KEY (owner_post_id)
        REFERENCES posts (id_generated) ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_account_id FOREIGN KEY (owner_account_id)
        REFERENCES accounts (id_generated) ON UPDATE CASCADE ON DELETE CASCADE
);

CREATE UNIQUE INDEX watches_owner_post_id_owner_post_id_idx
    ON watches (owner_post_id, owner_account_id);

CREATE TABLE IF NOT EXISTS post_replies
(
    id_generated                BIGSERIAL NOT NULL,
    owner_account_id            INT8 NOT NULL,
    owner_post_descriptor_id    INT8 NOT NULL,
    created_on                  TIMESTAMP WITH TIME ZONE DEFAULT (now() AT TIME ZONE 'utc') NOT NULL,
    notification_sent_on        TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    deleted_on                  TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    CONSTRAINT fk_owner_account_id FOREIGN KEY (owner_account_id)
        REFERENCES accounts (id_generated) ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_owner_post_descriptor_id FOREIGN KEY (owner_post_descriptor_id)
        REFERENCES post_descriptors (id_generated) ON UPDATE CASCADE ON DELETE CASCADE,
    PRIMARY KEY (id_generated)
);

CREATE UNIQUE INDEX post_replies_unique_id_idx ON post_replies (owner_account_id, owner_post_descriptor_id);
