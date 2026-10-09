-- Fluxo de publicação: rascunho, revisão, redirecionamento e histórico.
--
-- `documento.situacao` diz o que o visitante vê: 'rascunho' nunca foi ao ar,
-- 'publicado' está no ar, 'despublicado' saiu. O andamento do texto em edição
-- fica em `versao_rascunho` e `revisao_pedida_em`, porque um documento no ar
-- pode ter, ao mesmo tempo, um rascunho novo esperando revisão. O valor
-- 'em-revisao' da primeira migração deixa de ser gravado.

alter table documento
    -- Documento que nunca foi ao ar ainda não tem endereço: o slug do rascunho
    -- só passa a valer, e a disputar unicidade, na publicação. Soltar o
    -- `not null` não quebra o código anterior, que sempre informa os dois.
    alter column slug drop not null,
    alter column caminho drop not null,
    -- A versão em edição. É regravada a cada salvamento e congela ao publicar.
    add column versao_rascunho uuid references versao (id),
    add column revisao_pedida_em timestamptz,
    -- Conta do Auth (`sub`) de quem criou. É o que define "rascunho próprio".
    add column criado_por text,
    add constraint documento_fora_do_rascunho_tem_endereco
        check (situacao = 'rascunho' or (slug is not null and caminho is not null)),
    add constraint documento_em_revisao_tem_rascunho
        check (revisao_pedida_em is null or versao_rascunho is not null);

-- Conta do Auth de quem gravou a versão por último.
alter table versao add column gravado_por text;

-- Endereço que já esteve no ar e mudou. Quem tem o link antigo chega ao novo.
create table redirecionamento (
    site_id uuid not null references site (id) on delete cascade,
    de text not null check (de like '/%'),
    para text not null check (para like '/%'),
    criado_em timestamptz not null default now(),
    primary key (site_id, de),
    check (de <> para)
);

-- Quem fez o quê em qual documento. Não guarda o conteúdo.
create table historico (
    id bigint generated always as identity primary key,
    site_id uuid not null references site (id) on delete cascade,
    documento_id uuid references documento (id) on delete set null,
    conta text not null,
    -- A ação foi da equipe da Ávila Ops, com poder de Dono.
    equipe boolean not null default false,
    acao text not null,
    criado_em timestamptz not null default now()
);

create index historico_do_site on historico (site_id, criado_em desc);
