create table meetings (
    id            uuid        primary key default gen_random_uuid(),
    title         text        not null,
    description   text        not null default '',
    emoji         text        not null default '✨',
    held_on       date        not null,
    cover_mime    text,
    cover_bytes   bytea,
    cover_version integer     not null default 0,
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now()
);

create table participants (
    id          uuid        primary key default gen_random_uuid(),
    meeting_id  uuid        not null references meetings (id) on delete cascade,
    name        text        not null,
    emoji       text        not null,
    color_index smallint    not null,
    position    integer     not null,
    created_at  timestamptz not null default now(),
    -- Не косметика: position участвует в тай-брейке при раздаче остатка рублей.
    -- Два одинаковых position в одной встрече сделали бы результат зависимым от
    -- порядка строк, который Postgres при равных ключах не гарантирует.
    unique (meeting_id, position)
);

create table entries (
    id            uuid        primary key default gen_random_uuid(),
    meeting_id    uuid        not null references meetings (id) on delete cascade,
    kind          text        not null check (kind in ('expense', 'transfer')),
    payer_id      uuid        not null references participants (id) on delete cascade,
    recipient_id  uuid                 references participants (id) on delete cascade,
    amount_rubles bigint      not null check (amount_rubles > 0),
    description   text        not null default '',
    occurred_at   timestamptz not null default now(),
    created_at    timestamptz not null default now(),
    -- Получатель есть тогда и только тогда, когда это перевод.
    check ((kind = 'transfer') = (recipient_id is not null)),
    check (recipient_id is null or recipient_id <> payer_id)
);

create index entries_meeting_occurred_idx on entries (meeting_id, occurred_at desc);

create table entry_shares (
    entry_id        uuid     not null references entries (id) on delete cascade,
    participant_id  uuid     not null references participants (id) on delete cascade,
    -- Полная доля (4/4) не хранится: её отсутствие и есть полная доля.
    weight_quarters smallint not null check (weight_quarters between 0 and 3),
    primary key (entry_id, participant_id)
);

create table meeting_log (
    id         bigserial   primary key,
    meeting_id uuid        not null references meetings (id) on delete cascade,
    text       text        not null,
    created_at timestamptz not null default now()
);

create index meeting_log_meeting_created_idx on meeting_log (meeting_id, created_at desc);
