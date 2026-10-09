-- Autores e categorias do blog de cada site.
--
-- O post guarda o autor e a categoria inteiros, no contrato do motor: o que
-- foi ao ar não muda quando o cadastro muda. Estas tabelas são de onde o
-- painel os tira na hora de escrever.

create table autor (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    slug text not null check (slug ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'),
    nome text not null check (btrim(nome) <> ''),
    cargo text not null default '',
    bio text not null default '',
    -- A foto é da biblioteca do site. Enquanto um autor a usa, não é apagada.
    foto_id uuid references midia (id),
    perfis text[] not null default '{}',
    credenciais text[] not null default '{}',
    unique (site_id, slug)
);

create table categoria (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    slug text not null check (slug ~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$'),
    nome text not null check (btrim(nome) <> ''),
    unique (site_id, slug)
);
