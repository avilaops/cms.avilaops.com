# SwiftUI AAD runtime

Runtime iOS declarativo em `platforms/ios`, desacoplado de dependências SwiftUI
JSON de terceiros. O piloto completo, seus limites e gates de produção estão
em [`ios-runtime-pilot.md`](ios-runtime-pilot.md).

## Escopo
- Swift Package `AvilaAADRuntime`, iOS 16+ e macOS 13+.
- Decodificação AAD 1.0.0, três componentes: text, button, spacer; ação navigate.
- Navegação local por NavigationStack, sem execução arbitrária e sem acesso a APIs externas.
- Testes XCTest para contrato mínimo, componente desconhecido e destino inválido.

## Validar no macOS
```bash
cd platforms/ios
swift test
```

O pacote foi compilado e testado no runner GitHub `macos-15` pela execução
[#39026475261](https://github.com/avilaops/cms.avilaops.com/actions/runs/39026475261).
Ele continua sendo uma base experimental, não um app completo instalável em
TestFlight.

## Estado do piloto

- paridade de limite total e campos desconhecidos implementada;
- app shell com carregamento, conteúdo e erro recuperável implementado;
- build para iOS Simulator, XCTest e capturas nativas executados no GitHub;
- autenticação remota, cache/rollback, assinatura e TestFlight continuam
  bloqueados pelos gates descritos na documentação do piloto.

A comparação e a decisão de não adicionar dependência na v1 estão em
[`swiftui-library-audit.md`](swiftui-library-audit.md).
