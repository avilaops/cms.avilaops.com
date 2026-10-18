# Runtime iOS SwiftUI piloto — design

## Intenção

Transformar a prova de conceito do `AvilaAADRuntime` em um pacote piloto que
consiga receber uma definição AAD 1.0 validada, apresentá-la em SwiftUI nativo
e falhar de forma segura. O piloto serve à equipe que integrará o primeiro app
iOS ao CMS; ainda não é um aplicativo distribuível nem uma implementação de
login real.

## Objetivos

- manter paridade com o limite de 512 KiB e com a validação fechada do backend;
- renderizar `text`, `button` e `spacer` com navegação local tipada;
- separar UI, estado do runtime, carregamento de dados e credenciais;
- oferecer armazenamento de credencial no Keychain sem registrar o segredo;
- representar carregamento, conteúdo e erro recuperável no app shell;
- compilar para iOS 16 e executar testes no runner macOS do GitHub;
- gerar capturas do SwiftUI real como artefatos do workflow.

## Não objetivos

- publicar no TestFlight ou App Store;
- criar OAuth/OIDC, porque o backend ainda não oferece um contrato móvel;
- consumir os endpoints privados do painel, que atualmente dependem de cookie;
- executar código, HTML, WebView, expressões ou componentes remotos;
- adicionar dependências externas ou regras de negócio do ERP/CRM/TMS.

## Arquitetura

```text
AADDefinitionLoader -> bytes (até 512 KiB) -> AADDefinitionDecoder
                    -> modelo validado -> AADRuntimeStore -> AADRuntimeRootView
                                                       -> AADAppView

AADCredentialStore -> Keychain genérico
```

`AADDefinitionLoader` é a fronteira assíncrona de dados. O piloto inclui um
loader em memória para integração e testes; um cliente HTTP de produção só
será criado quando existir endpoint móvel autenticável e documentado.
`AADRuntimeStore`, isolado no `MainActor`, controla estados e retentativa. A UI
recebe o store por injeção e nunca lê credenciais.

`AADCredentialStore` expõe apenas salvar, ler e apagar bytes de credencial. A
implementação Apple usa Keychain Services e acessibilidade
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`. Ela não define o protocolo
de autenticação nem presume Bearer token ou cookie.

## Contrato AAD

- versão do schema e runtime: exatamente `1.0.0`;
- payload máximo: 524.288 bytes;
- 1 a 100 telas e até 200 componentes por tela;
- ids e rotas: `^[a-z][a-z0-9-]*$`, com no máximo 64 bytes ASCII;
- ids de tela, rotas e ids de componente por tela não podem se repetir;
- `navigate` deve apontar para rota existente;
- campos desconhecidos ou chaves duplicadas falham na raiz, navegação, tela,
  componente e ação;
- nenhum fallback pode renderizar parcialmente uma definição inválida.

## Experiência do piloto

- Carregando: `ProgressView` e texto “Carregando aplicativo”.
- Pronto: `NavigationStack`, título nativo, conteúdo rolável e botão de ação.
- Erro: ícone de alerta, mensagem segura e botão “Tentar novamente”. O texto
  técnico do erro não aparece para a pessoa usuária.
- A ordem acessível acompanha a ordem declarada; controles usam SwiftUI nativo,
  Dynamic Type e não possuem altura fixa.

## Segurança e privacidade

- nenhum segredo em AAD, logs, fixtures ou screenshots;
- Keychain restringe o item ao aparelho e o atualiza de forma idempotente;
- falhas de Keychain são tipadas sem incluir o valor secreto;
- loaders remotos futuros deverão exigir HTTPS, política de autenticação
  definida pelo backend e `Cache-Control` compatível com dados privados;
- configuração remota só combina funcionalidades compiladas no binário.

## Evidência e aceite

1. XCTest cobre limites, campos desconhecidos, rotas, estado/retry e Keychain
   por uma interface testável, sem afirmar comportamento de mocks.
2. `swift test` passa em `macos-15`.
3. `xcodebuild` compila o pacote para um destino genérico de iOS Simulator.
4. O workflow publica capturas nativas dos estados pronto e erro.
5. A validação geral do repositório permanece verde.
6. Nenhum build ou teste é executado nos servidores da Ávila Ops.

## Pendências explícitas para produção

- contrato de autenticação móvel OAuth/OIDC e endpoint de entrega publicado;
- cache assinado, última versão válida e rollback no dispositivo;
- app host com bundle ID, assinatura, UI tests em Simulator e dispositivo;
- validação manual com VoiceOver, Voice Control, Switch Control e Assistive
  Access em hardware real.
