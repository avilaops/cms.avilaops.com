# CMS Architecture and Roadmap Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publicar uma arquitetura verificável e um roadmap orientado a benefícios para clientes do CMS Ávila Ops.

**Architecture:** Dois documentos separados mantêm descrição atual e evolução futura sem misturar fatos com intenção. O README fornece apenas navegação; as fontes técnicas existentes e o código sustentam todas as afirmações.

**Tech Stack:** Markdown, Mermaid, Rust/Axum/SQLx/Askama, MCP, n8n.

**Spec:** `docs/superpowers/specs/2026-10-25-cms-arquitetura-roadmap-design.md`

## Global Constraints

- Escrever em português claro e técnico.
- Tratar apenas comportamento encontrado no repositório como estado atual.
- Marcar explicitamente arquitetura-alvo e itens futuros.
- Preservar CMS como conteúdo institucional e Lojas como comércio transacional.
- Não introduzir código executável, dependências ou configuração de produção.
- Não incluir segredos, nomes ou domínios de clientes.
- Não fazer push, merge ou publicação.

## Review Focus

- Afirmações de implementação sem evidência no código devem ser corrigidas ou marcadas como alvo.
- Limites entre CMS e Lojas devem permanecer explícitos em todos os fluxos.
- Propagação de configuração deve ter fonte canônica, validação e rollback; não pode significar cópia manual.
- Extensibilidade não pode permitir código arbitrário de cliente dentro do CMS.
- Roadmap deve ter critérios verificáveis e nenhuma data inventada.

---

### Task 1: Documento de arquitetura

**Files:**
- Create: `docs/arquitetura-cms.md`

**Interfaces:**
- Consumes: spec, `README.md`, crates do workspace, migrações, rotas web, documentos de App Builder e n8n.
- Produces: vocabulário e limites arquiteturais usados pelo roadmap.

- [ ] **Step 1: Mapear evidências do estado atual**

Registrar no relatório os arquivos usados para cada limite, componente e fluxo.

- [ ] **Step 2: Escrever `docs/arquitetura-cms.md`**

Implementar todas as seções e diagramas exigidos pela spec, separando estado
atual de arquitetura-alvo.

- [ ] **Step 3: Verificar o documento**

Run: `rg -n "TODO|TBD|cliente real|example.com" docs/arquitetura-cms.md`
Expected: nenhuma pendência, dado de cliente ou placeholder indevido.

- [ ] **Step 4: Commit**

```bash
git add docs/arquitetura-cms.md
git commit -m "docs: documentar arquitetura do CMS"
```

### Task 2: Roadmap orientado a resultados

**Files:**
- Create: `docs/roadmap-cms.md`

**Interfaces:**
- Consumes: limites e vocabulário de `docs/arquitetura-cms.md`.
- Produces: fases, gates e indicadores para evolução da plataforma.

- [ ] **Step 1: Classificar capacidades**

Classificar cada frente como `existente`, `lacuna` ou `alvo` com evidência ou
dependência correspondente.

- [ ] **Step 2: Escrever `docs/roadmap-cms.md`**

Implementar as sete fases da spec, incluindo benefício ao cliente, entregáveis,
critérios de aceite, dependências, riscos e indicadores.

- [ ] **Step 3: Verificar prioridades e gates**

Confirmar que não há datas inventadas, que cada fase possui gate de saída e que
conteúdo restrito permanece condicionado a demanda comprovada.

- [ ] **Step 4: Commit**

```bash
git add docs/roadmap-cms.md
git commit -m "docs: definir roadmap do CMS"
```

### Task 3: Navegação e consistência documental

**Files:**
- Modify: `README.md`
- Test: `docs/arquitetura-cms.md`
- Test: `docs/roadmap-cms.md`

**Interfaces:**
- Consumes: os dois documentos aprovados nas tarefas anteriores.
- Produces: entrada única de navegação e validação final da documentação.

- [ ] **Step 1: Adicionar links ao README**

Adicionar links relativos para arquitetura e roadmap na seção `Documentos`, sem
duplicar o conteúdo dos documentos.

- [ ] **Step 2: Validar links e formatação**

Run: `cargo fmt --check`
Expected: exit code 0.

Run: `rg -n "arquitetura-cms|roadmap-cms" README.md`
Expected: os dois links aparecem uma vez na seção `Documentos`.

- [ ] **Step 3: Revisar coerência cruzada**

Confirmar que nomes, estados e limites não se contradizem entre README,
arquitetura, roadmap e spec.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: ligar arquitetura e roadmap do CMS"
```
