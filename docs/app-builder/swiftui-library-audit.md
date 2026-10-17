# Auditoria de bibliotecas SwiftUI para AAD

Auditoria concluída em 17/10/2026 para o AVL-153. A decisão para AAD 1.0 é
manter `AvilaAADRuntime` como adaptador próprio, sem dependências externas.
O JSON validado pelo CMS continua sendo AAD; nenhum formato de terceiros entra
na API pública nem é enviado diretamente ao renderizador.

## Critérios

- licença compatível e obrigações identificadas;
- pacote Swift reproduzível em runner macOS;
- compatibilidade com iOS 16, alvo mínimo do piloto;
- validação fechada de tipos, ações e versões;
- ações resolvidas por uma allowlist compilada, sem código remoto;
- componentes nativos e metadados suficientes para acessibilidade;
- manutenção, testes e estabilidade de API verificáveis.

## Resultado

| Opção | Evidência em 17/10/2026 | Compatibilidade com AAD | Decisão |
| --- | --- | --- | --- |
| [DynamicUI 0.1.1](https://github.com/0xWDG/DynamicUI/releases/tag/0.1.1) | MIT; Swift 5.9; iOS 15+; sem dependências; CI para plataformas Apple; validação versionada e metadados de acessibilidade | O esquema, os nomes de componentes e as ações são diferentes. Tipos desconhecidos podem ser ignorados e a biblioteca inclui expressões, links e imagens remotas, uma superfície maior que AAD 1.0 | Melhor candidata para uma experiência isolada futura, sempre atrás de um tradutor AAD tipado. Não adicionar na v1 |
| [JSONDrivenUI 1.0.0](https://github.com/EnesKaraosman/JSONDrivenUI/releases/tag/1.0.0) | MIT; Swift 5.9; iOS 17+; 160 testes declarados; depende de Kingfisher 8 | Formato próprio, callbacks por texto, imagens remotas e alvo mínimo acima do piloto | Não selecionar |
| [swiftui-json-render 0.2.1](https://github.com/bipa-app/swiftui-json-render/releases/tag/0.2.1) | Apache-2.0; Swift 5.9; iOS 16+; ampla suíte; o próprio projeto se declara pré-release e possui API voltada também a streaming/IA | Formato e versionamento próprios; componentes, ações, imagens remotas e extensões excedem a allowlist AAD | Não selecionar enquanto estiver em v0.x; reavaliar somente com API estável e adaptador isolado |
| [FlutterBuilder](https://github.com/Sharifur/flutter-builder) | Laravel/Flutter, sem release ou tag e sem arquivo de licença identificado | Não usa SwiftUI e gera outro contrato/runtime | Somente referência visual; não usar nem copiar código |
| `AvilaAADRuntime` | Código próprio, zero dependências, iOS 16+/macOS 13+, workflow macOS e XCTest | Decodifica AAD 1.0 diretamente e só renderiza `text`, `button`, `spacer` e `navigate` | Selecionado para o piloto |

Os dados acima foram verificados nos repositórios e manifests dos projetos nos
commits `c4f4157` (DynamicUI), `074e0d2` (JSONDrivenUI), `ceeb06d`
(swiftui-json-render) e `a2debfd` (FlutterBuilder).

## Arquitetura aprovada para o piloto

```text
API do CMS -> bytes AAD -> limite de tamanho -> decodificação estrita
           -> validação de versão/ids/rotas/limites
           -> modelo AAD interno -> SwiftUI nativo
           -> ações compiladas e permitidas pelo aplicativo
```

1. O backend Rust e o JSON Schema continuam sendo a fonte do contrato.
2. O runtime mantém modelos AAD próprios e não expõe modelos de biblioteca.
3. `navigate` altera apenas a `NavigationStack` local. Novas ações exigem tipos
   Swift explícitos, autorização do backend e testes; nomes vindos do JSON não
   podem selecionar funções arbitrárias.
4. O pacote não baixa manifests, scripts, plugins ou código executável. Isso
   preserva a separação entre configuração e funcionalidade compilada exigida
   pela regra 2.5.2 da App Store.
5. Falha de versão, componente ou rota interrompe a definição inteira. O
   runtime não deve ignorar silenciosamente conteúdo que o backend recusaria.
6. Dependências futuras só podem receber um modelo intermediário já validado;
   nunca o AAD bruto.

## Acessibilidade

- `Text` e `Button` usam controles SwiftUI nativos e preservam seus nomes
  visíveis como descrições acessíveis.
- O piloto precisa manter ordem de leitura igual à ordem dos componentes e
  suportar Dynamic Type sem alturas fixas.
- Antes de distribuição, executar Accessibility Inspector e a jornada completa
  em dispositivo físico com VoiceOver, Voice Control, Switch Control e
  Assistive Access. XCTest não substitui essa verificação manual.
- Metadados configuráveis de acessibilidade só entram numa futura versão AAD
  com campos tipados e limites próprios.

## Segurança e conformidade

- Tamanho máximo, campos desconhecidos, versões, ids, duplicidades, limites e
  destinos devem permanecer em paridade com a validação Rust.
- URLs, HTML, WebView, expressões, reflection e carregamento dinâmico ficam fora
  de AAD 1.0.
- Credenciais e tokens não fazem parte da definição; quando existir networking,
  ele viverá em serviços compilados e autorizado no backend.
- A regra 2.5.2 da App Store proíbe baixar ou executar código que altere as
  funcionalidades do app. Por isso, AAD descreve somente composições e ações
  previamente incluídas no binário.

## Gate para reavaliar DynamicUI

Uma experiência futura só pode avançar se todos os itens abaixo forem provados
num branch descartável:

1. versão fixada por tag e checksum no `Package.resolved`;
2. tradutor `AAD -> DynamicUI` sem aceitar o JSON da biblioteca pela rede;
3. tipos desconhecidos e ações fora da allowlist falhando de forma fechada;
4. imagens e links remotos desativados ou mediados por política do aplicativo;
5. paridade de snapshot e acessibilidade para todos os componentes AAD;
6. ganho mensurável de manutenção que compense a nova cadeia de suprimentos.

## Evidências

- [Workflow SwiftUI aprovado](https://github.com/avilaops/cms.avilaops.com/actions/runs/39026475261)
- [Workflow geral aprovado](https://github.com/avilaops/cms.avilaops.com/actions/runs/39026475232)
- [App Review Guidelines 2.5.2](https://developer.apple.com/app-store/review/guidelines/#software-requirements)
- [Testes de acessibilidade recomendados pela Apple](https://developer.apple.com/documentation/accessibility/performing-accessibility-testing-for-your-app)

