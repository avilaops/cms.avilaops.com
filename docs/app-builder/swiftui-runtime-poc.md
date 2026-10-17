# SwiftUI AAD runtime — prova de conceito

Primeiro runtime iOS declarativo em `platforms/ios`, desacoplado de dependências SwiftUI JSON de terceiros.

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

## Próximas verificações
1. Fechar a paridade de campos desconhecidos e limite total do payload com a
   implementação Rust.
2. Criar um app shell e validar o runtime em Simulator e dispositivo físico,
   incluindo VoiceOver e Dynamic Type.
3. Implementar cache, fallback e rollback de definições assinadas antes de
   permitir leitura remota.
4. Criar pipeline de assinatura/TestFlight somente depois desses gates e de
   aprovação explícita.

A comparação e a decisão de não adicionar dependência na v1 estão em
[`swiftui-library-audit.md`](swiftui-library-audit.md).
