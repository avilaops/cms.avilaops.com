-- Mensagens recebidas pelos formulários dos sites.
--
-- É dado pessoal de visitante: guarda só o que o formulário pediu, com o
-- rótulo de cada campo na hora do envio, e some sozinho depois de 180 dias
-- (a rotina de limpeza do histórico cuida disso). O endereço de rede de quem
-- enviou não é guardado.

create table mensagem (
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    -- A página pode sair do ar depois; a mensagem fica.
    documento_id uuid references documento (id) on delete set null,
    caminho text not null,
    -- [{"rotulo": "...", "valor": "..."}], na ordem do formulário.
    campos jsonb not null,
    criado_em timestamptz not null default now()
);

create index mensagem_do_site on mensagem (site_id, criado_em desc);
