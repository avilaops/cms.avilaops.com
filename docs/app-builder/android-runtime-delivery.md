# Android AAD authenticated delivery (incremento de base)

O cliente Android recebe um token já autorizado por uma camada externa e consulta
somente o AAD publicado no endpoint do CMS. Não inclui tela OAuth, segredo
embutido, armazenamento de tokens ou acesso direto a dados do TMS.

- HTTPS obrigatório antes de consultar credenciais; app slug e ambiente são fechados.
- `GET /api/runtime/v1/apps/{slug}/environments/{environment}`.
- `Authorization: Bearer`, `If-None-Match` opcional, retorno 200 ou 304.
- Envelope estrito com `appId`, `environment`, `version`, `definition`.
- Definição aprovada pelo `AADDecoder` Kotlin; sem scripts executáveis.

## Segurança a concluir antes de uso real

O transporte é **injetado**, não implementado como cliente HTTP pronto para
produção. O adaptador HTTP deve recusar todos os redirecionamentos, conferir TLS
e garantir que tokens não sejam logados. A obtenção do token requer Android
Custom Tabs / Authorization Code + PKCE e persistência cifrada por Android
Keystore. O cache deve ser privado e manter última definição válida. Não
conectar a serviços produtivos antes dessas etapas.

## Verificar
`gradle :runtime:testDebugUnitTest :runtime:assembleDebug --no-daemon`
no GitHub Actions hospedado, sem builds nos servidores da Ávila Ops.
