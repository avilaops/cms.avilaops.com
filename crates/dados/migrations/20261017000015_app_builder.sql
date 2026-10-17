-- App Registry separado do domínio de sites.
--
-- O rascunho mutável fica no registro do app. Cada `app_versao` é um
-- snapshot imutável; publicar ou reverter só troca o ponteiro do ambiente.

create table app (
    id uuid primary key default gen_random_uuid(),
    slug text not null unique
        check (slug ~ '^[a-z0-9]([a-z0-9-]{0,62}[a-z0-9])?$'),
    nome text not null check (btrim(nome) <> ''),
    bundle_id text not null unique
        check (bundle_id ~ '^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+$'),
    criado_por text not null check (btrim(criado_por) <> ''),
    rascunho jsonb,
    revisao_do_rascunho bigint not null default 0
        check (revisao_do_rascunho >= 0),
    rascunho_atualizado_por text,
    criado_em timestamptz not null default now(),
    atualizado_em timestamptz not null default now(),
    check (rascunho is not null or revisao_do_rascunho = 0),
    check (rascunho is null or rascunho_atualizado_por is not null)
);

create table app_participacao (
    app_id uuid not null references app (id) on delete cascade,
    -- O `sub` da conta no Ávila Auth.
    conta text not null check (btrim(conta) <> ''),
    papel text not null check (papel in ('owner', 'editor', 'viewer')),
    criado_em timestamptz not null default now(),
    primary key (app_id, conta)
);

create index app_participacao_da_conta on app_participacao (conta, app_id);

create table app_versao (
    id uuid primary key default gen_random_uuid(),
    app_id uuid not null references app (id) on delete cascade,
    numero integer not null check (numero > 0),
    schema_version text not null check (btrim(schema_version) <> ''),
    minimum_runtime_version text not null
        check (btrim(minimum_runtime_version) <> ''),
    definicao jsonb not null,
    criado_por text not null check (btrim(criado_por) <> ''),
    criado_em timestamptz not null default now(),
    unique (app_id, numero),
    unique (app_id, id)
);

create function impedir_mutacao_de_app_versao()
returns trigger
language plpgsql
as $$
begin
    raise exception 'app_versao é imutável' using errcode = '55000';
end;
$$;

create trigger app_versao_nao_atualiza
before update on app_versao
for each row execute function impedir_mutacao_de_app_versao();

create trigger app_versao_nao_remove
before delete on app_versao
for each row execute function impedir_mutacao_de_app_versao();

create table app_ambiente (
    app_id uuid not null references app (id) on delete cascade,
    nome text not null check (nome in ('development', 'staging', 'production')),
    versao_id uuid,
    atualizado_por text not null check (btrim(atualizado_por) <> ''),
    atualizado_em timestamptz not null default now(),
    primary key (app_id, nome),
    foreign key (app_id, versao_id)
        references app_versao (app_id, id)
);

create index app_ambiente_da_versao
    on app_ambiente (app_id, versao_id)
    where versao_id is not null;
