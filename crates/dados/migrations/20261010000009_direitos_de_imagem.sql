-- De quem é cada imagem e em que termos pode ser usada.
--
-- Com `credito`, é o que sai no dado estruturado da imagem (`ImageObject`), de
-- onde a busca de imagens tira o crédito e o selo de imagem licenciável.
-- Licença e página de aquisição são endereços: `https://` ou caminho do site.

alter table midia
    add column autoria text,
    add column aviso_de_direitos text,
    add column licenca text
        check (licenca is null or licenca ~ '^(https://|/)[^[:space:]]+$'),
    add column aquisicao_de_licenca text
        check (aquisicao_de_licenca is null or aquisicao_de_licenca ~ '^(https://|/)[^[:space:]]+$');
