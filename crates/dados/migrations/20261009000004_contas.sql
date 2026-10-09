-- Quem participa de cada site, e os convites.
--
-- O login é do Auth. O CMS guarda só a participação: conta, site e papel.

-- Quem criou o site. Serve ao limite de criações por dia.
alter table site add column criado_por text;

create index site_criado_por on site (criado_por, criado_em desc) where criado_por is not null;

create table participacao (
    site_id uuid not null references site (id) on delete cascade,
    -- O `sub` da conta no Auth.
    conta text not null,
    email text not null check (email = lower(email)),
    papel text not null check (papel in ('autor', 'editor', 'dono')),
    criado_em timestamptz not null default now(),
    primary key (site_id, conta)
);

create index participacao_da_conta on participacao (conta);

create table convite (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    email text not null check (email = lower(email)),
    papel text not null check (papel in ('autor', 'editor', 'dono')),
    -- O token vai no link e só existe aqui como SHA-256.
    token_hash text not null unique,
    expira_em timestamptz not null,
    aceito_em timestamptz,
    criado_por text not null,
    criado_em timestamptz not null default now()
);
