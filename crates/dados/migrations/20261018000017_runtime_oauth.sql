-- OAuth de clientes nativos do runtime. Estas credenciais não compartilham
-- tabelas, hashes ou escopos com o conector MCP.

create table runtime_oauth_client (
    id uuid primary key default gen_random_uuid(),
    app_id uuid not null references app (id) on delete cascade,
    name text not null check (length(trim(name)) > 0),
    redirect_uris text[] not null check (cardinality(redirect_uris) between 1 and 5),
    created_at timestamptz not null default now(),
    unique (id, app_id)
);

create index runtime_oauth_client_app on runtime_oauth_client (app_id);

create table runtime_oauth_code (
    hash text primary key,
    client_id uuid not null,
    app_id uuid not null,
    account_sub text not null,
    account_email text not null,
    account_name text not null,
    account_team boolean not null,
    scopes text[] not null check (
        cardinality(scopes) > 0
        and scopes <@ array['runtime:read']::text[]
    ),
    redirect_uri text not null,
    challenge text not null,
    expires_at timestamptz not null,
    used_at timestamptz,
    foreign key (client_id, app_id)
        references runtime_oauth_client (id, app_id) on delete cascade
);

create table runtime_oauth_session (
    id uuid primary key default gen_random_uuid(),
    client_id uuid not null,
    app_id uuid not null,
    account_sub text not null,
    account_email text not null,
    account_name text not null,
    account_team boolean not null,
    scopes text[] not null check (
        cardinality(scopes) > 0
        and scopes <@ array['runtime:read']::text[]
    ),
    access_hash text not null unique,
    access_expires_at timestamptz not null,
    refresh_hash text not null unique,
    refresh_expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    revoked_at timestamptz,
    foreign key (client_id, app_id)
        references runtime_oauth_client (id, app_id) on delete cascade
);

create index runtime_oauth_session_account
    on runtime_oauth_session (account_sub, app_id);
