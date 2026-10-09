-- Eventos: o que aconteceu e vira automação no n8n.
--
-- O evento é gravado na mesma transação do fato. Uma rotina entrega por
-- webhook, pelo menos uma vez, e o n8n encerra o evento por uma rota de volta,
-- com o resultado. Com o n8n fora do ar, o fato acontece igual e o evento
-- espera na fila.

create table evento (
    -- Vai no corpo da entrega: o mesmo fato tem sempre o mesmo identificador,
    -- e quem recebe descarta o repetido.
    id uuid primary key default gen_random_uuid(),
    site_id uuid not null references site (id) on delete cascade,
    tipo text not null,
    -- A chave do fato. Um fato gera um evento só.
    chave text not null unique,
    -- Projeção explícita por tipo, nunca a linha do banco.
    dados jsonb not null,
    situacao text not null default 'pendente'
        check (situacao in ('pendente', 'entregue', 'encerrado')),
    tentativas integer not null default 0,
    proxima_tentativa_em timestamptz not null default now(),
    ocorrido_em timestamptz not null default now(),
    entregue_em timestamptz,
    encerrado_em timestamptz,
    -- O que o n8n devolveu ao encerrar.
    resultado jsonb
);

create index evento_a_entregar on evento (proxima_tentativa_em) where situacao = 'pendente';
