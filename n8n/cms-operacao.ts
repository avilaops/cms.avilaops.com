const receberEvento = trigger({
  type: 'n8n-nodes-base.webhook',
  version: 2.2,
  config: {
    name: 'Receber evento do CMS',
    parameters: {
      httpMethod: 'POST',
      path: 'cms-eventos',
      authentication: 'headerAuth',
      responseMode: 'onReceived',
      options: {},
    },
    credentials: { httpHeaderAuth: newCredential('CMS - entrada de eventos') },
  },
  output: [
    {
      body: {
        id: '0b9f6a3e-3f1c-4d0a-9d55-6f0c2a8f1e11',
        tipo: 'conteudo.publicado',
        chave: 'historico:42',
        ocorridoEm: '2026-10-09T12:00:00Z',
        site: {
          id: '5d1f0e0a-7f0b-4b56-8a39-0d6a7c1b2e33',
          slug: 'oficina',
          nome: 'Oficina Exemplo',
          origem: 'https://oficina.sites.example',
          naBusca: true,
          indexnowChave: '0123456789abcdef0123456789abcdef',
        },
        dados: {
          documentoId: '9c2b7d44-1e55-4a0f-b3a1-2f4e6d8c0a55',
          especie: 'post',
          caminho: '/blog/como-escolher',
          titulo: 'Como escolher',
          caminhoAnterior: '/blog/escolher',
        },
      },
    },
  ],
});

const normalizarEvento = node({
  type: 'n8n-nodes-base.set',
  version: 3.4,
  config: {
    name: 'Normalizar evento',
    parameters: {
      mode: 'manual',
      includeOtherFields: false,
      assignments: {
        assignments: [
          { id: 'evento-id', name: 'id', value: expr('{{ $json.body.id }}'), type: 'string' },
          { id: 'evento-tipo', name: 'tipo', value: expr('{{ $json.body.tipo }}'), type: 'string' },
          {
            id: 'site-origem',
            name: 'origem',
            value: expr('{{ $json.body.site.origem }}'),
            type: 'string',
          },
          {
            id: 'indexnow-chave',
            name: 'indexnowChave',
            value: expr('{{ $json.body.site.indexnowChave ?? "" }}'),
            type: 'string',
          },
          {
            id: 'avisar-buscadores',
            name: 'avisarBuscadores',
            value: expr(
              '{{ ["conteudo.publicado", "conteudo.despublicado"].includes($json.body.tipo) && $json.body.site.naBusca === true && !!$json.body.site.indexnowChave }}',
            ),
            type: 'boolean',
          },
          {
            id: 'urls',
            name: 'urls',
            value: expr(
              '{{ [$json.body.dados?.caminho, $json.body.dados?.caminhoAnterior].filter(Boolean).map((caminho) => $json.body.site.origem + caminho) }}',
            ),
            type: 'array',
          },
        ],
      },
      options: {},
    },
  },
  output: [
    {
      id: '0b9f6a3e-3f1c-4d0a-9d55-6f0c2a8f1e11',
      tipo: 'conteudo.publicado',
      origem: 'https://oficina.sites.example',
      indexnowChave: '0123456789abcdef0123456789abcdef',
      avisarBuscadores: true,
      urls: [
        'https://oficina.sites.example/blog/como-escolher',
        'https://oficina.sites.example/blog/escolher',
      ],
    },
  ],
});

const descartarRepetido = node({
  type: 'n8n-nodes-base.removeDuplicates',
  version: 2,
  config: {
    name: 'Descartar evento repetido',
    parameters: {
      operation: 'removeItemsSeenInPreviousExecutions',
      logic: 'removeItemsWithAlreadySeenKeyValues',
      dedupeValue: expr('{{ $json.id }}'),
      options: { scope: 'node', historySize: 10000 },
    },
  },
  output: [
    {
      id: '0b9f6a3e-3f1c-4d0a-9d55-6f0c2a8f1e11',
      tipo: 'conteudo.publicado',
      origem: 'https://oficina.sites.example',
      indexnowChave: '0123456789abcdef0123456789abcdef',
      avisarBuscadores: true,
      urls: ['https://oficina.sites.example/blog/como-escolher'],
    },
  ],
});

const deveAvisarBuscadores = ifElse({
  version: 2.2,
  config: {
    name: 'Deve avisar buscadores?',
    parameters: {
      conditions: {
        options: { caseSensitive: true, leftValue: '', typeValidation: 'strict' },
        conditions: [
          {
            leftValue: expr('{{ $json.avisarBuscadores }}'),
            operator: { type: 'boolean', operation: 'true', singleValue: true },
          },
        ],
        combinator: 'and',
      },
    },
  },
});

const avisarIndexNow = node({
  type: 'n8n-nodes-base.httpRequest',
  version: 4.5,
  config: {
    name: 'Avisar buscadores pelo IndexNow',
    retryOnFail: true,
    maxTries: 3,
    waitBetweenTries: 5000,
    onError: 'continueRegularOutput',
    parameters: {
      method: 'POST',
      url: 'https://api.indexnow.org/indexnow',
      sendBody: true,
      contentType: 'json',
      specifyBody: 'json',
      jsonBody: expr(
        '{{ { host: $json.origem.replace(/^https?:\\/\\//, ""), key: $json.indexnowChave, keyLocation: $json.origem + "/" + $json.indexnowChave + ".txt", urlList: $json.urls } }}',
      ),
      options: { response: { response: { fullResponse: true } } },
    },
  },
  output: [{ statusCode: 202, statusMessage: 'Accepted', headers: {}, body: '' }],
});

const resultadoDoAviso = node({
  type: 'n8n-nodes-base.set',
  version: 3.4,
  config: {
    name: 'Resultado do aviso',
    parameters: {
      mode: 'manual',
      includeOtherFields: false,
      assignments: {
        assignments: [
          {
            id: 'resultado-aviso',
            name: 'resultado',
            value: expr(
              '{{ { buscadores: { avisados: ($json.statusCode ?? 0) >= 200 && ($json.statusCode ?? 0) < 300, status: $json.statusCode ?? null, erro: $json.error?.message ?? null, urls: $("Normalizar evento").item.json.urls } } }}',
            ),
            type: 'object',
          },
        ],
      },
      options: {},
    },
  },
  output: [
    {
      resultado: {
        buscadores: {
          avisados: true,
          status: 202,
          erro: null,
          urls: ['https://oficina.sites.example/blog/como-escolher'],
        },
      },
    },
  ],
});

const resultadoSemAviso = node({
  type: 'n8n-nodes-base.set',
  version: 3.4,
  config: {
    name: 'Resultado sem aviso',
    parameters: {
      mode: 'manual',
      includeOtherFields: false,
      assignments: {
        assignments: [
          {
            id: 'resultado-sem-aviso',
            name: 'resultado',
            value: expr(
              '{{ { buscadores: { avisados: false, motivo: "este evento não pede aviso, o site está fora da busca ou não tem chave do IndexNow" } } }}',
            ),
            type: 'object',
          },
        ],
      },
      options: {},
    },
  },
  output: [{ resultado: { buscadores: { avisados: false, motivo: 'sem aviso' } } }],
});

const encerrarEvento = node({
  type: 'n8n-nodes-base.httpRequest',
  version: 4.5,
  config: {
    name: 'Encerrar evento no CMS',
    retryOnFail: true,
    maxTries: 3,
    waitBetweenTries: 5000,
    parameters: {
      method: 'POST',
      url: expr(
        'https://cms.avilaops.com/api/admin/eventos/{{ $("Normalizar evento").item.json.id }}/encerrar',
      ),
      authentication: 'genericCredentialType',
      genericAuthType: 'httpTemplatedCustomAuth',
      sendBody: true,
      contentType: 'json',
      specifyBody: 'json',
      jsonBody: expr('{{ { resultado: $json.resultado } }}'),
      options: {},
    },
    credentials: { httpTemplatedCustomAuth: newCredential('CMS - volta de eventos') },
  },
  output: [{ data: 'Encerrado.' }],
});

const notaDoContrato = sticky(
  '## CMS - Operação\n' +
    'Recebe os eventos do CMS (`POST /webhook/cms-eventos`, com `authorization`), responde na hora e trabalha depois.\n\n' +
    '- O mesmo fato chega com o mesmo `id`: o repetido é descartado.\n' +
    '- `conteudo.publicado` e `conteudo.despublicado` avisam o IndexNow, se o site está na busca e tem chave.\n' +
    '- Todo evento é encerrado no CMS com o resultado de verdade.\n\n' +
    'O código deste workflow fica em `n8n/cms-operacao.ts` no repositório do CMS. Editou aqui, exporte de volta.',
  [normalizarEvento, descartarRepetido],
  { color: 4 },
);

export default workflow('cms-operacao', 'CMS - Operação')
  .add(receberEvento)
  .to(normalizarEvento)
  .to(descartarRepetido)
  .to(
    deveAvisarBuscadores
      .onTrue(avisarIndexNow.to(resultadoDoAviso.to(encerrarEvento)))
      .onFalse(resultadoSemAviso.to(encerrarEvento)),
  )
  .add(notaDoContrato)
  .group('Entrada', [normalizarEvento, descartarRepetido], {
    description: 'Lê o corpo do evento e descarta o que já foi processado, pelo id do fato.',
  })
  .group('Buscadores', [avisarIndexNow, resultadoDoAviso], {
    description: 'Avisa o IndexNow das URLs que mudaram e guarda a resposta de verdade.',
  });
