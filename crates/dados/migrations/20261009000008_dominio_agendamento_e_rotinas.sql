-- Domínio próprio pedido pelo painel, publicação agendada e o catálogo de
-- rotinas.

-- Quem pediu o domínio, para o histórico de quem mexeu no endereço do site.
alter table dominio
    add column pedido_por text,
    add column pedido_em timestamptz not null default now();

-- A publicação marcada para depois. A permissão é conferida ao agendar; a
-- validação roda de novo na hora.
alter table documento
    add column agendado_para timestamptz,
    add column agendado_por text,
    add constraint documento_agendado_tem_rascunho
        check (agendado_para is null or versao_rascunho is not null);

create index documento_agendado on documento (agendado_para) where agendado_para is not null;

-- O que roda sozinho. A linha é a trava: com várias instâncias, só a que
-- consegue atualizar `ultima_execucao` executa a rodada.
create table rotina (
    nome text primary key,
    ultima_execucao timestamptz not null default 'epoch'
);

insert into rotina (nome) values
    ('publicacao.agendada'),
    ('dominios.conferir'),
    ('historico.limpar');
