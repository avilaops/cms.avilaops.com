# Roadmap do CMS Ávila Ops

Este roadmap traduz os limites da [arquitetura do CMS](arquitetura-cms.md)
em resultados verificáveis. Segue a
[spec aprovada](superpowers/specs/2026-10-25-cms-arquitetura-roadmap-design.md).
Não define datas, prazos comerciais ou disponibilidade em produção.

## Estados, prioridades e evidências

- **Existente:** capacidade identificada no checkout, com fonte indicada;
  não comprova implantação ou operação em produção.
- **Lacuna:** limitação identificada na arquitetura ou no caminho atual;
  precisa ser resolvida para alcançar o resultado proposto.
- **Alvo:** entrega futura, sujeita às dependências e aos critérios desta fase.

As fases 1 e 2 são prioridade obrigatória: governança, recuperação e segurança
sustentam a evolução. A fase 3 vem em seguida para tornar distribuição e
descoberta verificáveis. As fases 4 a 6 são incrementos de produto nessa ordem
de referência; a entrada depende de caso de uso priorizado e capacidade de
manutenção. A fase 7 é opcional e só pode começar com demanda comprovada de
cliente real. Este documento não identifica clientes nem presume tal demanda.

Um gate de saída exige evidências anexadas à entrega: revisão do contrato,
testes reproduzíveis, ambiente e versão verificados, resultado dos cenários
negativos e responsável pela operação. Não basta marcar uma tarefa como feita.
Fase opcional não iniciada deve ser registrada como tal, nunca como concluída.
Suas dependências obrigatórias permanecem válidas para fases posteriores.

Os indicadores abaixo são propostas de medição, não resultados já observados.
Antes de cada implementação, registrar fonte, responsável, linha de base,
janela de medição e limiar de aceite dos indicadores de desempenho. Os gates
de segurança e integridade exigem todos os cenários definidos aprovados,
independentemente de ganho de velocidade ou adoção.

## Fase 1 — Fundação de governança e catálogo canônico

**Prioridade:** obrigatória, antes de distribuir configuração compartilhada.

**Estado e evidência:**

- **Existente:** workspace Rust com `motor-web` fixado por revisão no
  [Cargo.toml](../Cargo.toml), migrações SQLx e configuração de processo em
  [configuracao.rs](../crates/servidor/src/configuracao.rs).
- **Lacuna:** esses mecanismos não constituem catálogo canônico compartilhado
  entre CMS, Lojas, plugins e n8n, conforme a seção de propagação da arquitetura.
- **Alvo:** catálogo declarativo versionado, validado pelo Tester, distribuído
  automaticamente e com confirmação da versão aplicada por consumidor.

**Benefício ao cliente:** mudanças consistentes entre interfaces e integrações,
com menos divergência de configuração e recuperação de atualização incompatível.

**Entregáveis:**

- Definição do repositório canônico, proprietário de cada contrato, schema,
  compatibilidade e consumidores; segredos apenas por referência.
- Validação no Tester e artefato aprovado identificável por revisão.
- Distribuição automatizada, registro de aplicação e retorno a uma versão
  anterior compatível. Transporte e repositório são decisões desta fase.

**Critérios de aceite:**

1. Configuração válida chega aos consumidores declarados com a mesma revisão;
   schema inválido ou versão incompatível é bloqueado antes da distribuição.
2. Falha de um consumidor fica visível, sem declarar sucesso global; retomar
   a distribuição não duplica efeitos e permite confirmar o estado de cada um.
3. Um ensaio aplica a versão anterior compatível e verifica o resultado;
   artefatos e logs não contêm valores secretos.

**Dependências:** inventário de contratos e responsáveis dos produtos, ambiente
de validação do Tester e definição do mecanismo de distribuição.

**Riscos:** divergência entre versões, falha parcial e concentração de mudanças
num único catálogo. Mitigar com compatibilidade explícita, aplicação gradual
e evidência por consumidor, sem cópia manual como mecanismo de integração.

**Indicadores:** proporção de consumidores com revisão confirmada; tempo entre
aprovação e aplicação; rejeições por incompatibilidade; sucesso dos ensaios
de retorno à versão anterior.

**Gate de saída:** demonstrar distribuição e retorno completos em ambiente de
validação, incluindo falha parcial, com contratos revisados e operação atribuída.

## Fase 2 — Recuperação e segurança editorial

**Prioridade:** obrigatória, antes de ampliar o fluxo editorial.

**Estado e evidência:**

- **Existente:** rascunho, revisão, publicação, histórico e agendamento passam
  pelo [fluxo editorial](../crates/dados/src/fluxo.rs), com permissões do
  [domínio](../crates/dominio/src/fluxo.rs). Mídia tem armazenamento separado.
- **Lacuna:** versões e histórico não comprovam restauração e comparação
  editoriais completas. O agendamento reconstrói o ator como Editor sem
  reconsultar a participação vigente, conforme a arquitetura.
- **Alvo:** recuperação editorial autorizada, ensaio conjunto de banco e mídia
  e revalidação de autorização no momento da publicação agendada.

**Benefício ao cliente:** recuperar conteúdo após erro e impedir que uma
permissão removida continue produzindo publicações futuras.

**Entregáveis:**

- Fluxo de comparação e restauração com escolha explícita da versão,
  autorização, validação atual, histórico e preservação da versão anterior.
- Revalidação da participação e do poder de publicação no agendamento,
  com recusa registrada quando o acesso tiver mudado.
- Procedimento de backup e recuperação consistente de banco e volume de mídia,
  com objetivos acordados de perda máxima de dados e tempo de recuperação.

**Critérios de aceite:**

1. Restaurar conteúdo válido não apaga evidências anteriores; conteúdo antigo
   incompatível, mídia ausente ou endereço em conflito bloqueia a publicação.
2. Autor, conta sem participação e conta de outro site não restauram nem
   publicam fora de suas permissões por painel, HTTP ou MCP.
3. Revogar ou rebaixar quem agendou impede a publicação sem a permissão exigida;
   um agendamento autorizado continua passando pela validação editorial.
4. Ensaio isolado de recuperação confere documentos, referências e arquivos
   de mídia e atende aos objetivos acordados, sem atingir dados operacionais.

**Dependências:** limites e contratos da fase 1, política de retenção,
armazenamento de backup e ambiente isolado de recuperação.

**Riscos:** versões antigas incompatíveis, referências órfãs e falsa sensação
de proteção por backup nunca restaurado. Mitigar com revalidação e ensaio
reproduzível; snapshots do App Builder não equivalem a recuperação editorial.

**Indicadores:** sucesso e duração da restauração; perda de dados medida no
ensaio; falhas por mídia ausente; agendamentos recusados por autorização vigente.

**Gate de saída:** restauração e recuperação ensaiadas, objetivos atendidos e
matriz de autorização aprovada, incluindo mudança de papel após agendamento.

## Fase 3 — Distribuição e descoberta de conteúdo

**Prioridade:** próxima evolução após os gates obrigatórios.

**Estado e evidência:**

- **Existente:** leitura pública, SEO e artefatos de descoberta descritos na
  arquitetura; [negociação Markdown](../crates/web/src/descoberta.rs),
  [fila transacional](../crates/dados/src/eventos.rs) e
  [cache local](../crates/web/src/cache.rs).
- **Lacuna:** aceite HTTP do evento não comprova efeito concluído. Recuperar
  entregues sem resultado, garantir idempotência durável dos efeitos e manter
  coerência entre instâncias são alvos na arquitetura.
- **Alvo:** contratos de leitura e eventos versionados, distribuição observável
  e estratégia explícita de cache para a topologia adotada.

**Benefício ao cliente:** conteúdo publicado chega aos canais previstos e
falhas de distribuição podem ser identificadas e recuperadas.

**Entregáveis:**

- Matriz de documentos públicos e representações, com validação de SEO,
  descoberta e compatibilidade após mudanças do `motor-web`.
- Contrato versionado de eventos, correlação entre publicação e efeito,
  recuperação de entregues sem resultado e idempotência persistida por efeito.
- Política de invalidação e métricas de fila, mídia e leitura; alerta com
  responsável e procedimento de recuperação.

**Critérios de aceite:**

1. HTML, Markdown e artefatos de descoberta refletem apenas conteúdo publicado
   elegível; mudança de endereço e despublicação preservam as regras do CMS.
2. Falhas antes e depois do aceite HTTP, retorno atrasado e entrega repetida
   deixam resultado rastreável, sem repetir o efeito protegido por idempotência.
3. Publicar e despublicar atualizam todas as instâncias da topologia de teste
   dentro do limite de atualização acordado; não se presume cache distribuído.
4. Indisponibilidade de integração gera alerta verificável e pode ser resolvida
   pelo procedimento documentado, sem mover autorização síncrona para o n8n.

**Dependências:** fases 1 e 2; contratos e ambiente de teste das integrações;
definição da topologia e dos limites operacionais de atraso e atualização.

**Riscos:** efeitos duplicados, conteúdo antigo em cache e tratar descoberta
como garantia de indexação externa. Mitigar com ensaios de falha e métricas
separadas de publicação, entrega e conclusão; não prometer execução exatamente
uma vez nem posição em mecanismos de busca.

**Indicadores:** tempo da publicação ao efeito concluído; idade dos eventos sem
resultado; proporção de efeitos encerrados; duplicações detectadas; tempo de
atualização do cache e cobertura dos testes de descoberta.

**Gate de saída:** cenários de leitura, falha, recuperação e repetição aprovados,
com atraso e atualização dentro dos limites acordados e alerta exercitado.

## Fase 4 — Multilinguismo

**Prioridade:** incremento condicionado a caso de uso editorial priorizado.

**Estado e evidência:**

- **Existente:** idioma no
  [perfil do site](../crates/dominio/src/site.rs).
- **Lacuna:** idioma do perfil não constitui modelo de traduções relacionadas,
  conforme o estado atual versus alvo da arquitetura.
- **Alvo:** variantes editoriais por idioma, com identidade comum, revisão
  independente e regras explícitas para ausência de tradução.

**Benefício ao cliente:** publicar para diferentes públicos sem perder o vínculo
entre versões do mesmo conteúdo nem exibir uma tradução ainda não aprovada.

**Entregáveis:** modelo de relação entre variantes; endereços e metadados por
idioma; navegação entre traduções; indicação de tradução ausente ou desatualizada;
migração preservando o comportamento de sites com um idioma.

**Critérios de aceite:**

1. Publicar uma variante não publica as demais; relações ficam no mesmo site
   e não aceitam duplicação incompatível de idioma ou endereço.
2. Variante ausente segue a política documentada, sem servir rascunho nem
   apresentar silenciosamente outro idioma como se fosse o solicitado.
3. Alternativas por idioma, URLs canônicas e descoberta apontam apenas para
   variantes públicas válidas; mudança de origem sinaliza traduções a revisar.
4. Migração e retorno compatível são ensaiados com site de um idioma e site
   com múltiplas variantes, preservando conteúdo e endereços anteriores.

**Dependências:** fases 1 a 3; contrato compatível no `motor-web`; idiomas,
política de tradução e responsáveis editoriais definidos para o caso de uso.

**Riscos:** duplicação de conteúdo, traduções divergentes e migração de URLs.
Mitigar com identidade comum, revisão por variante e redirecionamentos
verificados. Tradução automática não é pré-requisito nem publicação automática.

**Indicadores:** proporção de variantes aprovadas entre as previstas; traduções
pendentes após alteração da origem; tempo de revisão; links de idioma inválidos.

**Gate de saída:** jornada com múltiplos idiomas aprovada, regressão do site de
um idioma sem perda e nenhuma variante não publicada exposta nos cenários de teste.

## Fase 5 — Colaboração editorial

**Prioridade:** incremento condicionado a fluxo de trabalho priorizado.

**Estado e evidência:**

- **Existente:** Autor, Editor e Dono e envio/devolução de revisão no
  [domínio editorial](../crates/dominio/src/fluxo.rs) e no fluxo de dados.
- **Lacuna:** os papéis e a solicitação de revisão não comprovam coordenação
  editorial completa, conforme a arquitetura.
- **Alvo:** responsabilidades de revisão, feedback contextual e tratamento de
  edição concorrente integrados às mesmas regras de publicação.

**Benefício ao cliente:** saber quem precisa agir e evitar perda silenciosa de
trabalho ou publicação de uma versão diferente da revisada.

**Entregáveis:** atribuição e estado da revisão; comentários ligados à revisão
do conteúdo; detecção de alteração concorrente; notificações posteriores pelo
n8n e histórico suficiente para reconstruir as decisões editoriais.

**Critérios de aceite:**

1. Duas gravações concorrentes não sobrescrevem silenciosamente o trabalho;
   conflito é apresentado com caminho seguro para recuperar e conciliar versões.
2. Mudança no conteúdo após revisão invalida ou exige renovação da aprovação
   segundo política explícita; fica registrada a versão efetivamente aprovada.
3. Autor não ganha poder de publicação por receber atribuição; participantes
   de outro site não leem comentários nem executam ações daquele conteúdo.
4. Falha de notificação não desfaz a decisão persistida; sua recuperação usa
   os mecanismos de eventos da fase 3.

**Dependências:** fases 1 a 3; política de revisão e responsáveis definidos.
Se houver variantes da fase 4, aplicar atribuição e aprovação por variante.

**Riscos:** burocracia sem benefício e aprovação desatualizada. Mitigar com o
menor fluxo que atenda ao caso priorizado e vínculo entre decisão e revisão.
Edição simultânea em tempo real não é requisito desta fase.

**Indicadores:** tempo entre envio e decisão; revisões sem responsável; conflitos
resolvidos sem perda; devoluções e aprovações invalidadas por alteração.

**Gate de saída:** jornada de autoria, revisão, devolução e publicação aprovada
com conflito concorrente e perda de acesso exercitados, sem ampliar permissões.

## Fase 6 — Extensibilidade governada

**Prioridade:** incremento condicionado a integração ou componente priorizado.

**Estado e evidência:**

- **Existente:** MCP com OAuth e escopos, regras compartilhadas e contrato AAD
  próprio do App Builder, descritos na arquitetura e na
  [API do App Builder](app-builder/api-v1.md).
- **Lacuna:** esses contratos não constituem catálogo governado de extensões
  distribuídas entre produtos e assistentes.
- **Alvo:** plugins, componentes aprovados e integrações consumindo contratos
  versionados, sem executar código arbitrário do cliente no processo do CMS.

**Benefício ao cliente:** acrescentar integrações compatíveis sem depender de
alterações isoladas que enfraqueçam segurança ou quebrem outros canais.

**Entregáveis:** catálogo de componentes e contratos aprovados; manifestos com
permissões mínimas e compatibilidade; validação pelo Tester; ativação, revogação,
distribuição e retorno rastreáveis. Integrações com Lojas usam referências
estáveis e API/eventos, mantendo o domínio transacional naquele produto.

**Critérios de aceite:**

1. Versão incompatível, permissão não aprovada ou componente fora do catálogo
   é recusado; os adaptadores não contornam autorização ou validação Rust.
2. Atualizar, revogar e retornar uma extensão é verificável por consumidor;
   a revogação bloqueia novas ações e registra o tratamento das já iniciadas.
3. Um contrato representativo percorre Tester, distribuição e consumo em
   ambiente de teste, com evidência de falha e recuperação.
4. Plugin não acessa banco diretamente nem injeta código no processo;
   integração comercial não duplica carrinho, checkout, pedidos ou estoque.

**Dependências:** catálogo da fase 1, segurança da fase 2 e contratos e efeitos
da fase 3. Se a extensão usar idiomas ou colaboração, depende dos respectivos
contratos das fases 4 e 5; sua adoção não é obrigatória para toda extensão.

**Riscos:** permissões excessivas, versões divergentes e duplicação de domínio.
Mitigar com revisão, compatibilidade testada e revogação exercitada. Componentes
AAD de apps não são automaticamente componentes editoriais do `motor-web`.

**Indicadores:** cobertura de contratos validados; falhas por incompatibilidade;
consumidores com versão confirmada; tempo de revogação e sucesso de retorno.

**Gate de saída:** contrato representativo aprovado de ponta a ponta, incluindo
incompatibilidade e revogação, sem execução arbitrária ou escrita entre bancos.

## Fase 7 — Portais e conteúdo restrito

**Prioridade:** opcional; demanda comprovada é gate de entrada obrigatório.

**Estado e evidência:**

- **Existente:** participação administrativa por site e acesso autenticado ao
  runtime de apps, descritos na arquitetura. Não equivalem a portal editorial.
- **Lacuna:** essas permissões não comprovam controle de leitura de conteúdo
  restrito para visitantes.
- **Alvo:** acesso de visitantes a conteúdo e mídia protegidos, limitado ao
  caso de uso validado, sem ampliar o CMS para comércio transacional.

**Benefício ao cliente:** disponibilizar materiais apenas ao público autorizado
quando houver necessidade comprovada e capacidade de operar esse acesso.

**Gate de entrada:** registrar demanda de cliente real, público, conteúdo,
regra de acesso, resultado esperado, responsável e capacidade de suporte em
registro privado apropriado. Sem essa evidência, manter a fase não iniciada.
O roadmap público não deve receber identificadores do cliente.

**Entregáveis:** política de autorização de leitura distinta da participação
editorial; integração de identidade; proteção de arquivos e representações;
revogação, auditoria e operação do portal. Cobrança, quando necessária, depende
de contrato com Lojas; pagamentos continuam fora do domínio do CMS.

**Critérios de aceite:**

1. Conta sem acesso, acesso revogado e conta de outro site não recebem conteúdo
   ou mídia protegidos por URL direta, HTML, Markdown, API ou cache.
2. Sitemaps, artefatos de descoberta e demais listagens públicas não revelam
   o conteúdo restrito; impedir indexação não substitui autorização.
3. Revogação tem efeito dentro do limite acordado e testado, incluindo sessão
   já aberta e cache; falha de autenticação não libera leitura por padrão.
4. Pessoa autorizada conclui a jornada definida na demanda; leitura não concede
   papel editorial, e suporte consegue investigar recusas sem expor conteúdo.

**Dependências:** demanda comprovada; fases 1 a 3; contrato de identidade e
política de acesso aprovados. Extensões usadas dependem da fase 6; traduções
ou colaboração usadas dependem das fases correspondentes.

**Riscos:** vazamento por mídia, cache ou descoberta e custo de suporte sem
demanda sustentável. Mitigar com análise de todos os caminhos de leitura,
testes negativos e validação do benefício antes de ampliar o escopo.

**Indicadores:** sucesso de acesso autorizado; recusas indevidas; tempo de
revogação; cobertura dos caminhos protegidos; chamados de acesso e adoção
entre o público definido na demanda.

**Gate de saída:** demanda atendida em piloto autorizado, matriz de leitura e
revogação aprovada em todos os canais e responsável preparado para operar.
Sem gate de entrada satisfeito, não há entrega de portal a declarar.

## Limites permanentes e revisão do roadmap

O CMS permanece fonte de verdade de conteúdo institucional e editorial.
Catálogo comercial, carrinho, checkout, pedidos, estoque e pagamentos pertencem
ao Lojas. Autorização, validação e consistência síncronas ficam no Rust;
efeitos posteriores ficam no n8n. Painel, HTTP e MCP compartilham as regras.

A cada gate, atualizar estados somente com evidências verificáveis e registrar
decisões de escopo e dependências. A implementação de uma capacidade não
comprova implantação. Este documento não executa push, merge ou deploy e não
transforma indicadores propostos em métricas já disponíveis.
