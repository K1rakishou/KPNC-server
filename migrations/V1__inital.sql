drop table if exists migrations;
drop table if exists logs;
drop table if exists accounts;
drop table if exists account_tokens;
drop table if exists threads;
drop table if exists posts;
drop table if exists post_descriptors;
drop table if exists post_replies;
drop table if exists post_watches;

create table migrations
(
    version    integer not null primary key,
    name       varchar(256),
    applied_on timestamp with time zone default (now() AT TIME ZONE 'utc'::text) not null,
    checksum   varchar(512)
);

create table logs
(
    id        bigserial,
    log_time  timestamp with time zone not null,
    log_level varchar(8),
    target    varchar,
    message   varchar not null
);

create index logs_log_time_idx
    on logs (log_time);
create index logs_log_level_idx
    on logs (log_level);

create table accounts
(
    id bigserial,
    account_id   varchar(128) not null primary key,
    valid_until  timestamp with time zone default null,
    created_on   timestamp with time zone default (now() AT TIME ZONE 'utc'::text) not null,
    deleted_on   timestamp with time zone default null
);

create unique index accounts_id
    on accounts (id);

create index accounts_created_on_idx
    on accounts (created_on);

create index accounts_deleted_on_idx
    on accounts (deleted_on);

create table account_tokens
(
    id               bigserial primary key,
    owner_account_id bigint not null
        constraint fk_owner_account_id
            references accounts (id)
            on update cascade on delete cascade,
    token            varchar(1024) default NULL::character varying,
    application_type bigint not null,
    token_type       bigint not null
);

create unique index owner_account_id_idx
    on account_tokens (owner_account_id);

create index token_idx
    on account_tokens (token);

create unique index unique_token_idx
    on account_tokens (token, application_type, token_type);

create table threads
(
    id                         bigserial primary key,
    site_name                  varchar(128) not null,
    board_code                 varchar(64) not null,
    thread_no                  bigint not null,
    is_dead                    boolean default false,
    last_processed_post_no     bigint default 0,
    last_processed_post_sub_no bigint default 0,
    created_on                 timestamp with time zone default (now() AT TIME ZONE 'utc'::text) not null,
    deleted_on                 timestamp with time zone default null,
    last_modified              timestamp with time zone default null
);

create unique index threads_unique_id_idx
    on threads (site_name, board_code, thread_no);

create table post_descriptors
(
    id              bigserial primary key,
    owner_thread_id bigint not null
        constraint fk_owner_thread_id
            references threads (id)
            on update cascade on delete cascade,
    post_no         bigint not null,
    post_sub_no     bigint not null default 0
);

create unique index post_descriptors_unique_id_idx
    on post_descriptors (owner_thread_id, post_no, post_sub_no);

create table post_replies
(
    id                            bigserial primary key,
    owner_account_id              bigint not null
        constraint fk_owner_account_id
            references accounts (id)
            on update cascade on delete cascade,
    owner_post_descriptor_id      bigint not null
        constraint fk_owner_post_descriptor_id
            references post_descriptors (id)
            on update cascade on delete cascade,
    reply_to_post_descriptor_id      bigint not null
        constraint fk_reply_to_post_descriptor_id
            references post_descriptors (id)
            on update cascade on delete cascade,
    notification_delivery_attempt smallint default 0,
    notification_delivered_on     timestamp with time zone default null,
    created_on                    timestamp with time zone default (now() AT TIME ZONE 'utc'::text) not null,
    deleted_on                    timestamp with time zone default null
);

create unique index post_replies_unique_id_idx
    on post_replies (owner_account_id, owner_post_descriptor_id, reply_to_post_descriptor_id);

create table post_watches
(
    id                       bigserial primary key,
    owner_account_id         bigint not null
        constraint fk_account_id
            references accounts (id)
            on update cascade on delete cascade,
    owner_post_descriptor_id bigint default '-1'::integer not null
        constraint fk_owner_post_descriptor_id
            references post_descriptors (id)
            on update cascade on delete cascade,
    application_type         bigint default '-1'::integer not null
);

create index post_watches_owner_account_id_idx
    on post_watches (owner_account_id);

create index post_watches_owner_post_descriptor_id_idx
    on post_watches (owner_post_descriptor_id);

create unique index post_watchers_unique_idx
    on post_watches (owner_account_id, owner_post_descriptor_id)