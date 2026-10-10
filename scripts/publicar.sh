#!/usr/bin/env bash
# Publica o commit de origin/main do CMS em produção.
#
# Build no apps-noclient, que não atende cliente; o servidor de aplicações só
# recebe a imagem pronta. É o mesmo desenho do build manual do avilaops/infra,
# com a migração do próprio binário no lugar do Prisma:
#
#   1. constrói ghcr.io/avilaops/cms.avilaops.com:sha-<commit>;
#   2. leva a imagem ao servidor de aplicações;
#   3. se há migração pendente, faz o dump do banco e migra; falhou, para aqui
#      e o serviço não é tocado;
#   4. troca o contêiner e espera a saúde; se não ficar saudável em 90 s, volta
#      à imagem anterior. Voltar de versão não desfaz migração.
#
# Uso, na máquina com `apps-noclient` e `applications` no ~/.ssh/config:
#   scripts/publicar.sh
set -euo pipefail

repositorio=$(cd "$(dirname "$0")/.." && pwd)
imagem=ghcr.io/avilaops/cms.avilaops.com
build=apps-noclient
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

echo "1/4 Build no $build"
git -C "$repositorio" archive --format=tar "$commit" | ssh "$build" "
  set -e
  dir=\$(mktemp -d /root/build-cms-publicar.XXXXXX)
  trap 'rm -rf \$dir' EXIT
  tar -x -C \$dir
  DOCKER_BUILDKIT=1 docker build --quiet --build-arg GIT_SHA=$commit -t $marca \$dir >/dev/null
" 2>&1 | quieto

echo "2/4 Imagem para o $producao"
ssh "$build" "docker save $marca | gzip -1" 2>/dev/null | ssh "$producao" "gunzip | docker load" 2>&1 | quieto
# O disco do build é pequeno: a imagem e o cache de mais de um dia saem.
ssh "$build" "docker rmi $marca >/dev/null 2>&1; docker builder prune -af --filter until=24h >/dev/null 2>&1" 2>&1 | quieto

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
