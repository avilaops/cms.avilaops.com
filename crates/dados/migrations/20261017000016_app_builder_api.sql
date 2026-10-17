-- Garantias persistentes usadas pelas APIs do App Builder.

alter table app_versao add column chave_idempotencia text;

create unique index app_versao_idempotente
    on app_versao (app_id, chave_idempotencia)
    where chave_idempotencia is not null;

create table app_auditoria (
    id bigint generated always as identity primary key,
    app_id uuid not null references app (id) on delete cascade,
    conta text not null check (btrim(conta) <> ''),
    acao text not null check (btrim(acao) <> ''),
    dados jsonb not null default '{}',
    correlation_id uuid not null,
    criado_em timestamptz not null default now()
);

create index app_auditoria_do_app
    on app_auditoria (app_id, criado_em desc, id desc);
