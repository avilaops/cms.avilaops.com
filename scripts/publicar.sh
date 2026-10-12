#!/usr/bin/env bash
# Publica o commit de origin/main do CMS em produção.
#
# A imagem vem do GitHub Actions: a validação do commit a constrói e a deixa
# como artefato. O servidor de aplicações só recebe a imagem pronta, e nenhum
# servidor da casa compila:
#
#   1. espera a validação do commit e baixa a imagem
#      ghcr.io/avilaops/cms.avilaops.com:sha-<commit>;
#   2. leva a imagem ao servidor de aplicações;
#   3. se há migração pendente, faz o dump do banco e migra; falhou, para aqui
#      e o serviço não é tocado;
#   4. troca o contêiner e espera a saúde; se não ficar saudável em 90 s, volta
#      à imagem anterior. Voltar de versão não desfaz migração.
#
# Uso, na máquina com o `gh` autenticado e `applications` no ~/.ssh/config:
#   scripts/publicar.sh
set -euo pipefail

repositorio=$(cd "$(dirname "$0")/.." && pwd)
imagem=ghcr.io/avilaops/cms.avilaops.com
repo_github=avilaops/cms.avilaops.com
producao=applications
pasta=/opt/cms-avilaops-com
conteiner=cms-avilaops-com-web
banco=cms_avilaops_com

# Os servidores avisam de locale a cada conexão; não é erro.
quieto() { grep -v -E 'setlocale|^perl: warning|LC_ALL|LC_[A-Z]+ =|LANGUAGE =|LANG =|are supported and installed|Falling back to' || true; }

git -C "$repositorio" fetch --quiet origin main
commit=$(git -C "$repositorio" rev-parse origin/main)
marca="$imagem:sha-$commit"
echo "Publicando $commit"

echo "1/4 Imagem do GitHub"
# A imagem é construída pela validação do próprio commit. Sem validação verde
# não há imagem, e sem imagem não há publicação.
execucao=$(gh run list -R "$repo_github" --workflow Validar --commit "$commit" \
  --json databaseId -q '.[0].databaseId')
[ -n "$execucao" ] || { echo "FALHA: o commit $commit não tem validação no GitHub"; exit 1; }
gh run watch "$execucao" -R "$repo_github" --exit-status >/dev/null 2>&1 \
  || { echo "FALHA: a validação do commit não passou (execução $execucao)"; exit 1; }
pasta_da_imagem=$(mktemp -d)
trap 'rm -rf "$pasta_da_imagem"' EXIT
gh run download "$execucao" -R "$repo_github" -n imagem -D "$pasta_da_imagem"

echo "2/4 Imagem para o $producao"
ssh "$producao" "gunzip | docker load" < "$pasta_da_imagem/imagem.tar.gz" 2>&1 | quieto

echo "3/4 Migração"
ssh "$producao" "
  set -u
  export LC_ALL=C
  cd $pasta
  binario() { docker run --rm --env-file .env --add-host host.docker.internal:host-gateway --network edge $marca servidor \"\$@\"; }
  if binario migrar --conferir >/dev/null 2>&1; then
    echo '    nenhuma migração pendente'
  else
    copia=/opt/backups/db/host-$banco-antes-de-$commit.sql.gz
    sudo -u postgres pg_dump $banco | gzip > \$copia.parcial
    gzip -t \$copia.parcial && mv \$copia.parcial \$copia
    echo \"    dump em \$copia\"
    binario migrar >/dev/null 2>&1 || { echo '    FALHA: a migração não rodou; o serviço não foi tocado'; exit 1; }
    binario migrar --conferir >/dev/null 2>&1 || { echo '    FALHA: ficou migração pendente; o serviço não foi tocado'; exit 1; }
    echo '    migrado'
  fi
" 2>&1 | quieto

echo "4/4 Troca do contêiner"
ssh "$producao" "
  set -u
  export LC_ALL=C
  cd $pasta
  anterior=\$(docker inspect -f '{{.Image}}' $conteiner 2>/dev/null || true)
  saudavel() {
    for _ in \$(seq 1 45); do
      [ \"\$(docker inspect -f '{{.State.Health.Status}}' $conteiner 2>/dev/null)\" = healthy ] && return 0
      sleep 2
    done
    return 1
  }
  docker tag $marca $imagem:latest
  docker compose up -d >/dev/null 2>&1
  if saudavel; then
    echo \"    no ar: \$(docker exec $conteiner printenv GIT_SHA)\"
    # Ficam a imagem nova e a anterior, para voltar atrás; o resto sai.
    novo=\$(docker inspect -f '{{.Id}}' $marca)
    docker images $imagem --no-trunc --format '{{.ID}} {{.Repository}}:{{.Tag}}' | grep ':sha-' | while read -r id nome; do
      [ \"\$id\" = \"\$novo\" ] || [ \"\$id\" = \"\$anterior\" ] || docker rmi \"\$nome\" >/dev/null 2>&1 || true
    done
  else
    echo '    FALHA: o contêiner novo não ficou saudável'
    if [ -n \"\$anterior\" ]; then
      docker tag \$anterior $imagem:latest
      docker compose up -d >/dev/null 2>&1
      saudavel && echo '    voltou à imagem anterior' || echo '    ATENÇÃO: a imagem anterior também não ficou saudável'
    fi
    exit 1
  fi
" 2>&1 | quieto

echo "Conferindo por fora"
codigo=$(curl -s -o /dev/null -m 20 -w '%{http_code}' https://cms.avilaops.com/api/saude)
[ "$codigo" = 200 ] || { echo "FALHA: https://cms.avilaops.com/api/saude respondeu $codigo"; exit 1; }
echo "Publicado: $commit"
