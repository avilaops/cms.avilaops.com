# Runtime iOS SwiftUI — piloto

O AVL-11 transforma o pacote experimental em um runtime piloto testável. Ele
decodifica AAD 1.0, controla estados assíncronos, renderiza componentes SwiftUI
nativos e oferece uma fronteira de credenciais baseada no Keychain.

## Módulos

- `AADDefinitionDecoder`: limite de 512 KiB, decodificação fechada e validação.
- `AADRuntimeStore`: carregamento, retentativa e descarte de resposta obsoleta.
- `AADRuntimeRootView`: estados carregando, pronto e erro recuperável.
- `AADCredentialStoring`: contrato de credencial independente do protocolo de
  login, com implementações em memória e Keychain.

## Autenticação

O CMS atual autentica endpoints privados com cookie do Ávila Auth. Isso não é
um contrato OAuth/OIDC para aplicativo móvel. O piloto, portanto, não inventa
um token Bearer nem tenta extrair cookies de sessão do navegador. Ele entrega
o armazenamento seguro necessário e deixa o carregador remoto bloqueado até
existirem endpoint móvel e fluxo de autenticação documentados.

## Segurança

- configuração maior que 524.288 bytes falha antes do parse;
- campos desconhecidos ou chaves JSON duplicadas falham de forma fechada;
- somente `text`, `button`, `spacer` e `navigate` são reconhecidos;
- mensagens de erro da UI não incluem detalhes técnicos ou credenciais;
- Keychain usa `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`;
- o pacote não inclui dependências externas, WebView ou código remoto.

## Evidências no GitHub

O workflow `Validar SwiftUI AAD`:

1. compila o pacote com `xcodebuild` para iOS Simulator, sem assinatura;
2. executa XCTest no runner `macos-15`;
3. hospeda os estados pronto e erro em `NSHostingView`, valida que há conteúdo
   visível e renderiza PNGs do SwiftUI;
4. publica os PNGs no artefato `swiftui-runtime-screenshots`.

Essas capturas são do renderizador SwiftUI real. Elas não substituem UI tests
em iOS Simulator nem validação de acessibilidade em dispositivo físico.

## Antes de produção

Ainda são necessários OAuth/OIDC, endpoint de entrega, cache assinado, última
versão válida, rollback, app host assinado, UI tests e testes manuais com
tecnologias assistivas. Nenhuma publicação é automática.

## Referências Apple

- [Keychain Services](https://developer.apple.com/documentation/security/keychain-services)
- [NavigationStack](https://developer.apple.com/documentation/swiftui/navigationstack)
- [NSHostingView](https://developer.apple.com/documentation/swiftui/nshostingview)
- [Accessibility testing](https://developer.apple.com/documentation/accessibility/performing-accessibility-testing-for-your-app)
