-- O redirecionamento passa a aceitar destino em outro endereço.
--
-- Até aqui ele só nascia da troca de slug, sempre de um caminho do site para
-- outro. Agora o Dono também o cadastra, para trazer um site que já existia:
-- o endereço antigo pode levar a outro caminho do site ou a outro site (as
-- rotas de um sistema que saíram para um subdomínio, por exemplo). `de` que
-- termina em `/*` vale para todo caminho que começa com o que vem antes.

alter table redirecionamento
    drop constraint redirecionamento_para_check,
    add constraint redirecionamento_para_check
        check (para like '/%' or para like 'https://%');
