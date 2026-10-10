-- Conector para assistentes de IA (MCP).
--
-- O servidor de autorização é o próprio CMS: registro dinâmico de cliente,
-- código com PKCE S256 e tokens que só existem aqui como SHA-256. A conexão é
-- da conta, e pode o que a pessoa marcou ao autorizar, limitado ao papel dela
-- em cada site.

create table cliente_mcp (
    id uuid primary key default gen_random_uuid(),
    nome text not null,
    redirect_uris text[] not null check (cardinality(redirect_uris) > 0),
    criado_em timestamptz not null default now()
);

-- O código de autorização: vale cinco minutos e uma troca só.
create table codigo_mcp (
    hash text primary key,
    cliente_id uuid not null references cliente_mcp (id) on delete cascade,
    conta_sub text not null,
    conta_email text not null,
    conta_nome text not null,
    conta_equipe boolean not null,
    escopos text[] not null,
    redirect_uri text not null,
    -- O desafio do PKCE (S256).
    desafio text not null,
    expira_em timestamptz not null,
    usado_em timestamptz
);

create table conexao_mcp (
    id uuid primary key default gen_random_uuid(),
    cliente_id uuid not null references cliente_mcp (id) on delete cascade,
    conta_sub text not null,
    conta_email text not null,
    conta_nome text not null,
    conta_equipe boolean not null,
    escopos text[] not null,
    acesso_hash text not null unique,
    acesso_expira_em timestamptz not null,
    renovacao_hash text not null unique,
    renovacao_expira_em timestamptz not null,
    criado_em timestamptz not null default now(),
    revogada_em timestamptz
);

create index conexao_mcp_da_conta on conexao_mcp (conta_sub);

-- O que o assistente fez: a ferramenta e os identificadores. Nunca os
-- argumentos nem o resultado.
create table chamada_mcp (
    id bigint generated always as identity primary key,
    conexao_id uuid not null references conexao_mcp (id) on delete cascade,
    ferramenta text not null,
    site_id uuid,
    documento_id uuid,
    deu_certo boolean not null,
    criado_em timestamptz not null default now()
);

create index chamada_mcp_da_conexao on chamada_mcp (conexao_id, criado_em desc);
