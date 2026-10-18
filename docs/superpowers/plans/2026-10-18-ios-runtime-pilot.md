# Runtime iOS SwiftUI Pilot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Entregar um pacote SwiftUI piloto que valide AAD 1.0 estritamente, gerencie carregamento, proteja credenciais no Keychain e gere build, testes e capturas no GitHub.

**Architecture:** O decoder recebe bytes e produz somente modelos AAD completamente válidos. Um store no `MainActor` coordena um loader injetado e alimenta um app shell SwiftUI; credenciais ficam numa fronteira independente baseada em Keychain.

**Tech Stack:** Swift 5.9, SwiftUI, Foundation, Security, XCTest, GitHub Actions `macos-15`.

**Spec:** `docs/superpowers/specs/2026-10-18-ios-runtime-pilot-design.md`

## Global Constraints

- iOS 16 e macOS 13 são os alvos mínimos.
- O pacote permanece sem dependências externas.
- AAD máximo de 524.288 bytes e schema/runtime exatamente `1.0.0`.
- Nenhum endpoint, método de autenticação ou publicação não documentado será inventado.
- Builds, testes e capturas executam somente no GitHub.
- Haverá um único commit/push quando o módulo estiver completo.

## Review Focus

- Payload exatamente no limite é aceito; um byte acima falha antes do decode.
- Campos desconhecidos em qualquer nível recusam a definição inteira.
- Uma resposta obsoleta de carregamento não substitui uma retentativa mais nova.
- Erros técnicos não vazam na mensagem apresentada pela UI.
- Salvar uma nova credencial substitui o item anterior sem duplicá-lo.

---

### Task 1: Decoder AAD estrito

**Files:**
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADDefinitionDecoder.swift`
- Modify: `platforms/ios/Sources/AvilaAADRuntime/AADModels.swift`
- Modify: `platforms/ios/Tests/AvilaAADRuntimeTests/AADTests.swift`

**Interfaces:**
- Produces: `AADDefinitionDecoder.decode(_ data: Data) throws -> AADApp`.
- Produces: erros públicos e equatáveis para tamanho, versão, identificador, limite, duplicidade e rota.

- [ ] Escrever testes que falham para payload acima de 524.288 bytes e campos desconhecidos na raiz, navegação, tela e ação.
- [ ] Implementar decodificação fechada em todos os níveis e validar tamanho antes de `JSONDecoder`.
- [ ] Adicionar casos-limite para ids ASCII, contagens, duplicidades e destinos.
- [ ] Revisar as mutações realistas: remover cada guarda deve quebrar ao menos um teste.

### Task 2: Estado e carregamento assíncrono

**Files:**
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADDefinitionLoader.swift`
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADRuntimeStore.swift`
- Create: `platforms/ios/Tests/AvilaAADRuntimeTests/AADRuntimeStoreTests.swift`

**Interfaces:**
- Produces: `AADDefinitionLoading.loadDefinition() async throws -> Data`.
- Produces: `AADRuntimeStore.State` com `idle`, `loading`, `ready(AADApp)` e `failed`.
- Produces: `AADRuntimeStore.load()` e `retry()` no `MainActor`.

- [ ] Escrever testes para sucesso, erro seguro, retentativa e concorrência obsoleta.
- [ ] Implementar loader em memória e store com cancelamento/geração de requisição.
- [ ] Garantir que a UI receba apenas uma mensagem segura para qualquer erro técnico.

### Task 3: Fronteira de credenciais no Keychain

**Files:**
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADCredentialStore.swift`
- Create: `platforms/ios/Tests/AvilaAADRuntimeTests/AADCredentialStoreTests.swift`
- Modify: `platforms/ios/Package.swift`

**Interfaces:**
- Produces: `AADCredentialStoring.save(_: Data) throws`, `read() throws -> Data?` e `delete() throws`.
- Produces: `KeychainCredentialStore(service:account:)`.

- [ ] Escrever testes de contrato sobre uma implementação em memória real para salvar, substituir, ler e apagar.
- [ ] Implementar o adaptador Keychain com `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` e erros sem segredo.
- [ ] Verificar compilação condicional e API pública nos dois alvos Apple.

### Task 4: App shell, acessibilidade e capturas

**Files:**
- Modify: `platforms/ios/Sources/AvilaAADRuntime/AADView.swift`
- Create: `platforms/ios/Sources/AvilaAADRuntime/AADRuntimeRootView.swift`
- Create: `platforms/ios/Tests/AvilaAADRuntimeTests/AADSnapshotTests.swift`

**Interfaces:**
- Consumes: `AADRuntimeStore` e `AADAppView`.
- Produces: `AADRuntimeRootView` com estados carregando, pronto e erro/retry.

- [ ] Escrever testes de apresentação para títulos e ações sem depender de detalhes privados.
- [ ] Implementar estados com componentes nativos, Dynamic Type e labels acessíveis.
- [ ] Gerar PNG dos estados pronto e erro via `ImageRenderer` no runner macOS.

### Task 5: CI, documentação e evidência final

**Files:**
- Modify: `.github/workflows/validar-swiftui.yml`
- Modify: `docs/app-builder/swiftui-runtime-poc.md`
- Create: `docs/app-builder/ios-runtime-pilot.md`

**Interfaces:**
- Consumes: pacote e testes completos.
- Produces: build iOS, XCTest macOS e artefato `swiftui-runtime-screenshots`.

- [ ] Configurar `xcodebuild` para destino genérico de iOS Simulator.
- [ ] Executar `swift test` com diretório de screenshots e publicar os PNGs.
- [ ] Documentar arquitetura, limites, execução no GitHub e bloqueios de produção.
- [ ] Revisar diff completo, garantir ausência de segredos e executar `git diff --check`.
- [ ] Fazer o único commit/push do módulo e aguardar todos os checks do GitHub.
