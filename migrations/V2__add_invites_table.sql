create table invites(
    invite_id varchar(256) primary key not null,
    accepted_on timestamp with time zone default null,
    expires_on timestamp with time zone default null
)