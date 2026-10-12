-- Produto de catálogo: a terceira espécie de documento.
--
-- É o que a empresa oferece sem vender pelo site: ficha técnica, fotos,
-- modelo 3D e um botão para pedir orçamento. Sai em /produtos/<slug>. Preço,
-- estoque e carrinho continuam sendo do Lojas.

alter table documento drop constraint documento_especie_check;
alter table documento
    add constraint documento_especie_check check (especie in ('pagina', 'post', 'item'));
