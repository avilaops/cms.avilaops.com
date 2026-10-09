-- Sites, domínios e documentos com versões.
--
-- Toda tabela de conteúdo tem `site_id`, e toda consulta filtra por ele.

create table site (
    id uuid primary key default gen_random_uuid(),
    -- É o rótulo do endereço provisório: <slug>.<domínio-base>.
    slug text not null unique
        check (slug ~ '^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$'),
    situacao text not null default 'em-montagem'
        check (situacao in ('em-montagem', 'ativo', 'suspenso')),
    -- O dono assumiu o endereço provisório como definitivo. Sem isso, e sem
    -- domínio próprio, o site fica fora da busca.
    provisorio_definitivo boolean not null default false,
    -- Nome, descrição, idioma, logo, organização e diretrizes para IA.
    perfil jsonb not null,
    criado_em timestamptz not null default now(),
    atualizado_em timestamptz not null default now()
);

create table dominio (
    host text primary key check (host = lower(host)),
    site_id uuid not null references site (id) on delete cascade,
    situacao text not null default 'pendente'
        check (situacao in ('pendente', 'ativo')),
    conferido_em timestamptz
);

-- A canônica de um site aponta para um endereço só.
create unique index dominio_um_ativo_por_site on dominio (site_id) where situacao = 'ativo';

create table documento (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    especie text not null check (especie in ('pagina', 'post')),
    -- Vazio só na home.
    slug text not null,
    -- Começa com barra. A home é '/'.
    caminho text not null check (caminho like '/%'),
    situacao text not null default 'rascunho'
        check (situacao in ('rascunho', 'em-revisao', 'publicado', 'despublicado')),
    versao_publicada uuid,
    publicado_em timestamptz,
    atualizado_em timestamptz,
    criado_em timestamptz not null default now(),
    unique (site_id, especie, slug),
    unique (site_id, caminho),
    -- Publicado sempre aponta para a versão que está no ar.
    check (situacao <> 'publicado' or versao_publicada is not null)
);

-- O conteúdo é o tipo do motor-web, inteiro, em JSON. Editar um documento
-- publicado cria nova versão; o site continua servindo a publicada.
create table versao (
    id uuid primary key default gen_random_uuid(),
    documento_id uuid not null references documento (id) on delete cascade,
    numero integer not null check (numero > 0),
    conteudo jsonb not null,
    criado_em timestamptz not null default now(),
    unique (documento_id, numero)
);

alter table documento
    add constraint documento_versao_publicada_fkey
    foreign key (versao_publicada) references versao (id);

create index documento_publicados on documento (site_id) where situacao = 'publicado';
