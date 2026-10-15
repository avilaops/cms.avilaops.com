# Tema Cardápio

O tema `cardapio` transforma os blocos de planos e preços das páginas
selecionadas em uma seleção de itens para WhatsApp. A fonte é o conteúdo
publicado do próprio site, lido pelo servidor; não existe arquivo separado de
produtos para atualizar.

Na identidade, escolha **Cardápio**, confira o telefone com DDD e informe as
categorias, uma por linha, na ordem desejada:

```text
pizza /pizzas
borda /bordas
produto /bebidas
```

O mesmo campo no conector é `paginasDoCardapio`. O tipo `pizza` permite
escolher uma borda; `borda` fornece os adicionais; `produto` não aceita borda.
Nenhuma regra é deduzida do nome da página. São permitidas até 12 categorias,
sem repetir endereços. O telefone aceita o formato brasileiro com DDD ou
com DDI 55; o tema não verifica se o número possui conta no WhatsApp.

Cada item vem de uma seção `planos` da página. Use um nome único por página e
preço exato em reais, como `R$ 35,90`, sem período. Valores sob consulta,
intervalos e cobranças por mês mantêm a versão estática, com o texto e os
links originais; não viram um subtotal. Páginas despublicadas e conteúdo de
outros sites não entram no cardápio.

O visitante pode buscar por nome ou ingrediente, trocar de categoria, ordenar,
escolher borda e quantidade, escrever observações e revisar a mensagem no
WhatsApp. Os valores são somados em centavos inteiros. O carrinho fica no
navegador, limitado a 100 linhas e 99 unidades por linha. Nome e endereço
digitados não são persistidos. Ao abrir a página novamente, itens removidos
são descartados e os preços vêm do catálogo atual. Renomear um produto limpa
esse item do carrinho; reordenar os blocos mantém sua identidade.

O resumo não envia mensagens, cobra ou confirma pedidos. Frete, tamanho,
disponibilidade, valor final e pagamento são confirmados no atendimento.
Não há cálculo de meia pizza porque essa regra depende do estabelecimento.

Sem JavaScript, ou se ele falhar, os blocos do CMS continuam legíveis. O
script é servido pelo próprio CMS em endereço com resumo do conteúdo e
cache imutável. A política de conteúdo libera script próprio somente nas
páginas que exibem o cardápio. Os outros temas preservam o comportamento.

## Verificação

Além de `cargo fmt --check`, Clippy e testes Rust, execute:

```sh
node --test crates/web/recursos/pedido.test.mjs
```

Os testes de integração usam banco isolado e conferem isolamento por site,
conteúdo publicado, atualização de preço, recurso JavaScript e política de
conteúdo. A prévia autenticada usa o rascunho da página aberta e o conteúdo
publicado das outras categorias.
