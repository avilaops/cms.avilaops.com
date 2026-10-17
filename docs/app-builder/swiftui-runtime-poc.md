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

Compilação e testes **ainda não executados em macOS**. Este pacote é a base experimental e não um app completo instalável em TestFlight.

## Próximas verificações
1. Rodar swift test no runner macOS, conferir paridade de decoding com Rust e corrigir incompatibilidades.
2. Validar rejeição de campos extras em todos os níveis do JSON (a implementação atual verifica explicitamente os componentes).
3. Reconciliar payloads máximos, versões e limites de aninhamento com AAD Rust.
4. Avaliar DynamicUI (MIT) como adaptador opcional, sem alterar o contrato proprietário.
5. Posteriormente criar app shell e pipeline de assinatura/TestFlight.

Referências: https://github.com/0xWDG/DynamicUI e https://github.com/EnesKaraosman/JSONDrivenUI.
