# Editor React e fronteira do servidor

## Decisão

O painel evolui seguindo a separação de responsabilidades do
[experiments](https://github.com/avilaops/experiments/blob/main/DEPLOYMENT.md):
interface React/TypeScript, credenciais e autorização no servidor, núcleo
independente da interface e evidências por revisão no GitHub CI.

O BFF do CMS é o adaptador HTTP Rust existente. Não há serviço Node adicional,
segredo no bundle, login paralelo ou duplicação do domínio editorial. Auth
continua sendo o provedor de sessão. SQLx, motor-web, MCP e Estúdio permanecem.

## Implementação desta revisão

`dashboard/` constrói os assets privados. O servidor incorpora os bundles e
os serve com nomes derivados do hash. Docker compila o painel em etapa própria;
Node não entra na imagem final. A CI compila o painel antes dos testes Rust.

O contrato de apresentação `editor-shell/1` está no HTML autenticado:
`form[data-editor-contract="1"]`, seções com identificador estável e
`data-editor-section`, seletores de mídia marcados e URL de prévia emitida pelo
servidor. React controla navegação, filtro de mídia e estado local. Campos,
ações disponíveis e submissão pertencem ao formulário Rust. Se o bundle não
carregar, o formulário permanece utilizável. Não há interpretação de HTML
fornecido por assistentes nem conteúdo editorial executável.

Salvar, mover e remover continuam enviando o formulário completo ao fluxo
editorial transacional. Esconder uma seção não desabilita nem remove seus
campos. O atalho Ctrl/Cmd+S usa a ação de salvar existente. A interface avisa
sobre alterações pendentes; não declara sucesso antes de resposta do servidor.

A prévia incorporada mostra a última versão salva, não alterações locais.
Somente essa resposta permite `frame-ancestors 'self'`; somente o editor recebe
`frame-src 'self'`. O iframe não executa scripts nem envia formulários. Assim,
interações como 3D devem ser conferidas na prévia em outra aba. Todas as
variantes de mídia do documento são adaptadas antes da renderização de srcset.
As referências persistidas não mudam.

## Verificação e limites

Os testes Rust verificam autorização, fluxo editorial, contrato de apresentação
e variante de imagem. Eles exportam HTML de editor/prévia obtido pelo roteador
autenticado com dados sintéticos. O teste Playwright usa esse HTML para conferir
campos preservados entre seções, envio completo, estado pendente, iframe,
largura de 390 px e funcionamento sem JavaScript. O servidor de replay não
substitui os testes Rust de gravação e autorização. Capturas ficam no artefato
`cms-editor-evidence` do commit testado.

Esta é uma migração progressiva do editor, não uma reescrita de todas as telas.
Revisão otimista entre editores, comparação/restauração de versões editoriais,
contrato JSON de edição e unificação com o canvas do Estúdio continuam como
incrementos separados. Não se atribuem ao CMS as garantias de If-Match do
experiments apenas por adotar React. Não há deploy nem alteração de conteúdo
de clientes decorrentes desta revisão.
