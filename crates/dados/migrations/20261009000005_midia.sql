-- Biblioteca de mídia por site.
--
-- O arquivo chega, o original vai para o disco e a linha nasce com dimensões e
-- `alt`. As variantes (AVIF e WebP nas larguras do motor) saem depois, por uma
-- rotina: a mídia fica 'pendente' até lá, e não pode ir ao ar sem elas.

create table midia (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    -- O nome do arquivo enviado. Dele sai o nome das variantes.
    nome text not null,
    -- SHA-256 do original: o mesmo arquivo não entra duas vezes no site.
    hash text not null,
    largura integer not null check (largura > 0),
    altura integer not null check (altura > 0),
    alt text not null check (btrim(alt) <> ''),
    legenda text,
    credito text,
    bytes bigint not null check (bytes > 0),
    situacao text not null default 'pendente'
        check (situacao in ('pendente', 'pronta', 'falhou')),
    tentativas integer not null default 0,
    proxima_tentativa_em timestamptz not null default now(),
    enviado_por text not null,
    criado_em timestamptz not null default now(),
    unique (site_id, hash)
);

create index midia_a_processar on midia (proxima_tentativa_em) where situacao = 'pendente';

create table variante (
    midia_id uuid not null references midia (id) on delete cascade,
    formato text not null check (formato in ('avif', 'webp')),
    largura integer not null check (largura > 0),
    -- O nome do arquivo na pasta do site, servido em /midia/<arquivo>.
    arquivo text not null,
    bytes bigint not null,
    primary key (midia_id, formato, largura)
);

-- De onde cada imagem é usada, no rascunho ou no que está no ar. Mídia em uso
-- não é apagada: a referência sem `on delete cascade` é a trava.
create table uso_de_midia (
    midia_id uuid not null references midia (id),
    documento_id uuid not null references documento (id) on delete cascade,
    primary key (midia_id, documento_id)
);

create index uso_de_midia_do_documento on uso_de_midia (documento_id);
