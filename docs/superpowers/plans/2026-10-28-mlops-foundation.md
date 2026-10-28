# Base de desenvolvimento MLOps do CMS

## Objetivo e arquitetura

Adotar o ciclo MLOps existente no módulo CMS: definir regra e aceite, implementar,
verificar na CI e preservar evidência vinculada ao commit. O CMS continua Rust;
o laboratório é o cadastro e avaliador, sem dependência síncrona das publicações.
O responsável pelo módulo e pelas demandas sem responsável é Codex, conforme
instrução do proprietário em 28/10/2026. Isso não cria identidade humana no Linear.

## Primeiro incremento

- [x] Versionar seis fichas adicionais no contrato PolicyDraft do laboratório.
- [x] Registrar manifesto de fontes com SHA-256 e seletores dos testes existentes.
- [x] Implementar gate sem dependências externas para contrato, fontes e cobertura.
- [x] Implementar testes de recusa de duplicação, referência sem cobertura, hash divergente,
  seletor inexistente e classificação indevida como verificada.
- [x] Integrar gate e relatório no workflow Validar existente, incluindo falhas.
- [x] Enviar ao GitHub no PR 21; conferir resultado da execução exata no registro
  técnico vinculado à issue antes de declarar validação concluída.
- [x] Vincular a entrega à AVL-355 no Linear, mantendo o escopo dos outros módulos.

O resultado de CI é registrado externamente na issue para preservar o commit
imutável validado. A revisão desta etapa não autoriza merge ou implantação.

## Critérios de aceite

O gate deve recusar metadados inconsistentes e gerar relatório com revisão do
catálogo, commit da execução, run URL, resultado dos testes Rust e hashes das
fontes. Integridade de fonte e presença de testes não equivalem a aprovação da
regra ou integração em produção. Fichas permanecem implemented nesta entrega.
Testes e builds são executados exclusivamente no GitHub CI.

## Sequência de desenvolvimento

1. Base de políticas, critérios e evidências automatizadas (este incremento).
2. Contratos de API e compatibilidade de consumidores (AVL-343), após inventário
   das interfaces reais; não presumir Utoipa neste workspace.
3. Executor CMS instrumentado com identidade, autorização, resultado observável,
   idempotência e reconciliação, alinhado à AVL-342.
4. Integrações e novas capacidades do módulo em incrementos com aceite e evidência.

O inventário é parcial. Recuperação de eventos, autorização vigente no agendamento
e integração privada do catálogo continuam explícitas como próximas entregas.
Não instalar serviços nem modificar produção para implantar esta base.
