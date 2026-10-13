-- Imagem que não escreve "Foto: ..." embaixo dela na página: logo, ícone,
-- ilustração da casa. O crédito e os direitos continuam no dado estruturado.
alter table midia add column ocultar_credito boolean not null default false;
