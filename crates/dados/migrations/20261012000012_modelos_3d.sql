-- Modelos 3D por site.
--
-- O GLB é o que o visualizador do site abre; o USDZ, quando existe, é o que o
-- iPhone pede para pôr o modelo no espaço da pessoa. Os arquivos ficam no
-- disco, em <mídia>/<site>/modelos/<id>.glb e .usdz, e não mudam depois do
-- envio: modelo novo é linha nova.

create table modelo_3d (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    -- O nome do arquivo enviado.
    nome text not null,
    -- O que o modelo mostra, para quem não o enxerga.
    descricao text not null check (btrim(descricao) <> ''),
    -- SHA-256 do GLB: o mesmo modelo não entra duas vezes no site.
    hash text not null,
    glb_bytes bigint not null check (glb_bytes > 0),
    usdz_bytes bigint check (usdz_bytes > 0),
    enviado_por text not null,
    criado_em timestamptz not null default now(),
    unique (site_id, hash)
);

create index modelo_3d_do_site on modelo_3d (site_id, criado_em desc);
