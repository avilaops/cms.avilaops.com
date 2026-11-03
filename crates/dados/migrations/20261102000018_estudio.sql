-- Estúdio: arquivos de desenho de um site, páginas, nós, biblioteca,
-- versões imutáveis e a prévia publicada.
--
-- Toda linha de conteúdo leva `site_id`. A chave composta impede um nó de
-- apontar para a página de outro arquivo ou de outro site.

create table estudio_arquivo (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    nome text not null check (char_length(btrim(nome)) between 1 and 120),
    pedido text not null default '' check (char_length(pedido) <= 4000),
    criado_por text not null check (btrim(criado_por) <> ''),
    favorito boolean not null default false,
    sistema text not null check (char_length(sistema) between 1 and 40),
    criado_em timestamptz not null default now(),
    atualizado_em timestamptz not null default now(),
    unique (site_id, id)
);

create index estudio_arquivo_do_site
    on estudio_arquivo (site_id, atualizado_em desc, id);

create table estudio_pagina (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null,
    arquivo_id uuid not null,
    nome text not null check (char_length(btrim(nome)) between 1 and 80),
    ordem integer not null check (ordem >= 0),
    largura integer not null check (largura between 320 and 1920),
    altura integer not null check (altura between 320 and 2400),
    unique (arquivo_id, ordem),
    unique (site_id, arquivo_id, id),
    foreign key (site_id, arquivo_id)
        references estudio_arquivo (site_id, id) on delete cascade
);

create table estudio_no (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null,
    arquivo_id uuid not null,
    pagina_id uuid not null,
    tipo text not null check (tipo in ('frame', 'retangulo', 'texto', 'componente')),
    x integer not null check (x >= 0 and x <= 1920),
    y integer not null check (y >= 0 and y <= 2400),
    largura integer not null check (largura between 8 and 1920),
    altura integer not null check (altura between 8 and 2400),
    texto text not null default '' check (char_length(texto) <= 2000),
    preenchimento text check (
        preenchimento is null or preenchimento ~ '^#[0-9a-f]{6}$'
    ),
    biblioteca text,
    variante text,
    catalogo_versao text,
    props jsonb not null default '{}'::jsonb,
    ordem integer not null check (ordem >= 0),
    foreign key (site_id, arquivo_id, pagina_id)
        references estudio_pagina (site_id, arquivo_id, id) on delete cascade,
    check (
        (
            tipo = 'componente'
            and biblioteca = 'docs'
            and variante in (
                'chamada', 'cartao', 'abas', 'navegacao', 'sumario', 'lateral', 'busca'
            )
            and catalogo_versao is not null
            and char_length(catalogo_versao) between 1 and 40
        )
        or (
            tipo <> 'componente'
            and biblioteca is null
            and variante is null
            and catalogo_versao is null
        )
    )
);

create index estudio_no_da_pagina on estudio_no (pagina_id, ordem, id);

create function estudio_no_cabe_na_pagina()
returns trigger
language plpgsql
as $$
declare
    largura_pagina integer;
    altura_pagina integer;
begin
    select largura, altura into largura_pagina, altura_pagina
    from estudio_pagina
    where id = new.pagina_id
      and site_id = new.site_id
      and arquivo_id = new.arquivo_id;
    if largura_pagina is null then
        raise exception 'página do estúdio não encontrada' using errcode = '23503';
    end if;
    if new.x + new.largura > largura_pagina or new.y + new.altura > altura_pagina then
        raise exception 'nó fora da página' using errcode = '23514';
    end if;
    return new;
end;
$$;

create trigger estudio_no_dentro_da_pagina
before insert or update on estudio_no
for each row execute function estudio_no_cabe_na_pagina();

create table estudio_biblioteca (
    site_id uuid not null references site (id) on delete cascade,
    slug text not null check (slug = 'docs'),
    versao text not null check (char_length(btrim(versao)) between 1 and 40),
    ativa boolean not null,
    ativada_por text not null check (btrim(ativada_por) <> ''),
    ativada_em timestamptz not null default now(),
    primary key (site_id, slug)
);

create table estudio_versao (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null,
    arquivo_id uuid not null,
    numero integer not null check (numero > 0),
    definicao jsonb not null,
    criado_por text not null check (btrim(criado_por) <> ''),
    criado_em timestamptz not null default now(),
    unique (arquivo_id, numero),
    unique (site_id, id),
    unique (site_id, arquivo_id, id),
    foreign key (site_id, arquivo_id)
        references estudio_arquivo (site_id, id) on delete cascade
);

create function impedir_mutacao_de_estudio_versao()
returns trigger
language plpgsql
as $$
begin
    raise exception 'estudio_versao é imutável' using errcode = '55000';
end;
$$;

-- Só a atualização é barrada. Apagar o arquivo ou o site remove a versão
-- em cascata; o aplicativo não oferece outro caminho de exclusão.
create trigger estudio_versao_nao_atualiza
before update on estudio_versao
for each row execute function impedir_mutacao_de_estudio_versao();

-- O token é a capacidade da prévia. Fica gravado para o editor copiar o
-- link de novo. Não é credencial de sessão.
create table estudio_publicacao (
    arquivo_id uuid primary key,
    site_id uuid not null,
    versao_id uuid not null,
    token text not null unique check (token ~ '^[0-9a-f]{64}$'),
    publicado_por text not null check (btrim(publicado_por) <> ''),
    publicado_em timestamptz not null default now(),
    foreign key (site_id, arquivo_id)
        references estudio_arquivo (site_id, id) on delete cascade,
    foreign key (site_id, arquivo_id, versao_id)
        references estudio_versao (site_id, arquivo_id, id)
);
