# Identidade visual Ávila

A marca do ecossistema Ávila Ops: CMS, Lojas e a casa (Ops). Uma marca para
tudo o que é da Ávila; os sites de clientes usam a estrutura do CMS e vestem
o tema e a cor do cliente.

## O Marco

O símbolo é o triângulo com o ponto central que a cartografia usa para o
vértice geodésico, o ponto de onde toda medição parte. Também se lê como o
"A" de Ávila. As assinaturas juntam o Marco, "Ávila" em Archivo e o produto
em caixa alta sob um traço curto de terracota (o traço de cota).

| Arquivo | Uso |
|---|---|
| `avila-simbolo.svg` | Marco em terracota, fundo claro. Mínimo 16px. |
| `avila-simbolo-negativo.svg` | Marco para fundo escuro. |
| `avila-simbolo-branco.svg` | Marco branco, só sobre terracota. |
| `avila-icone-app.svg` | Ícone de aplicativo e de perfil. |
| `avila-cms.svg`, `avila-lojas.svg`, `avila-ops.svg` | Assinaturas horizontais, mínimo 24px de altura. |
| `*-negativo.svg` | As mesmas assinaturas para fundo escuro. |

O texto das assinaturas está em curva: não depende de fonte instalada. Não
recolorir fora dessas versões, girar, contornar nem pôr sombra.

## Cores e tipo

| Token | Claro | Escuro | Papel |
|---|---|---|---|
| `terracota` | `#9a3412` | `#fb923c` | Marca: Marco, traço de cota, destaque dos sites da casa |
| `cota` | `#1a4fa0` | `#8fb4f0` | Ação nas aplicações: botão principal, link, foco |
| `grafite` | `#1c1917` | `#f5f5f4` | Texto |
| `suave` | `#57534e` | `#a8a29e` | Texto de apoio |
| `papel` | `#f6f5f2` | `#141210` | Fundo das aplicações |
| `superficie` | `#ffffff` | `#24211e` | Cartão e formulário |
| `borda-campo` | `#8c877d` | `#857d76` | Contorno de controle (3:1) |

Todos os pares de texto passam de 4,5:1 nos dois temas. A fonte da casa é a
Archivo, servida pelo próprio CMS em `/recursos/fontes/archivo.woff2`. O
painel (`crates/web/templates/painel_base.html`) segue estes tokens e o tema
escuro do aparelho; os sites de cliente seguem os temas em
`crates/web/src/tema.rs`.
