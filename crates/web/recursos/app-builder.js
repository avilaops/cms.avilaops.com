(() => {
  "use strict";
  const raiz = document.querySelector("#app-builder");
  if (!raiz) return;
  const editor = document.querySelector("#definicao");
  const seletor = document.querySelector("#tela-preview");
  const previa = document.querySelector("#previa");
  const status = document.querySelector("#status-editor");
  const appId = raiz.dataset.appId;
  let revisao = Number(raiz.dataset.revision);

  const mensagem = (texto, falha = false) => {
    status.textContent = texto;
    status.className = `status-editor ${falha ? "falha" : "ok"}`;
  };
  const ler = () => JSON.parse(editor.value);
  const erroDaResposta = async (resposta) => {
    const corpo = await resposta.json().catch(() => ({}));
    return corpo.error?.message || corpo.errors?.[0]?.message || `Erro HTTP ${resposta.status}`;
  };
  const escapar = (valor) => {
    const no = document.createElement("span");
    no.textContent = String(valor ?? "");
    return no.innerHTML;
  };
  const renderizar = () => {
    try {
      const definicao = ler();
      const telas = Array.isArray(definicao.screens) ? definicao.screens : [];
      const anterior = seletor.value;
      seletor.replaceChildren(...telas.map((tela) => {
        const opcao = document.createElement("option");
        opcao.value = tela.id;
        opcao.textContent = tela.title || tela.id;
        return opcao;
      }));
      if (telas.some((tela) => tela.id === anterior)) seletor.value = anterior;
      const tela = telas.find((item) => item.id === seletor.value) || telas[0];
      if (!tela) {
        previa.innerHTML = "<p>Adicione uma tela à definição.</p>";
        return;
      }
      const componentes = (tela.components || []).map((componente) => {
        if (componente.type === "text") return `<p class="previa-componente">${escapar(componente.value)}</p>`;
        if (componente.type === "button") return `<span class="previa-componente previa-botao">${escapar(componente.label)}</span>`;
        if (componente.type === "spacer") return "<div class=\"previa-componente previa-espaco\" aria-hidden=\"true\"></div>";
        return `<p class="recado erro">Componente desconhecido: ${escapar(componente.type)}</p>`;
      }).join("");
      previa.innerHTML = `<h3>${escapar(tela.title || tela.id)}</h3>${componentes}`;
    } catch (erro) {
      previa.innerHTML = `<p class="recado erro">JSON inválido: ${escapar(erro.message)}</p>`;
    }
  };
  editor.addEventListener("input", renderizar);
  seletor.addEventListener("change", renderizar);
  document.querySelector("#formatar")?.addEventListener("click", () => {
    try {
      editor.value = JSON.stringify(ler(), null, 2);
      mensagem("Definição formatada.");
      renderizar();
    } catch (erro) {
      mensagem(`JSON inválido: ${erro.message}`, true);
    }
  });
  document.querySelector("#validar")?.addEventListener("click", async () => {
    let definicao;
    try { definicao = ler(); } catch (erro) { mensagem(`JSON inválido: ${erro.message}`, true); return; }
    const resposta = await fetch(`/api/v1/apps/${appId}/validate`, {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(definicao)
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    mensagem("Rascunho válido para AAD 1.0.0.");
  });
  document.querySelector("#salvar")?.addEventListener("click", async () => {
    let definicao;
    try { definicao = ler(); } catch (erro) { mensagem(`JSON inválido: ${erro.message}`, true); return; }
    const resposta = await fetch(`/api/v1/apps/${appId}/draft`, {
      method: "PUT",
      headers: { "content-type": "application/json", "if-match": `\"${revisao}\"` },
      body: JSON.stringify(definicao)
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    revisao = (await resposta.json()).draftRevision;
    raiz.dataset.revision = String(revisao);
    document.querySelector("#revisao-atual").textContent = `Revisão ${revisao}`;
    mensagem(`Rascunho salvo na revisão ${revisao}.`);
  });
  document.querySelector("#versionar")?.addEventListener("click", async () => {
    const resposta = await fetch(`/api/v1/apps/${appId}/versions`, {
      method: "POST",
      headers: { "content-type": "application/json", "idempotency-key": crypto.randomUUID() },
      body: "{}"
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    const versao = await resposta.json();
    mensagem(`Versão ${versao.number} criada. Recarregando histórico…`);
    window.setTimeout(() => window.location.reload(), 500);
  });
  document.querySelectorAll(".publicar").forEach((botao) => botao.addEventListener("click", async () => {
    const cartao = botao.closest("[data-environment]");
    const versao = Number(cartao.querySelector("select").value);
    if (!versao) { mensagem("Escolha uma versão para publicar.", true); return; }
    const ambiente = cartao.dataset.environment;
    const resposta = await fetch(`/api/v1/apps/${appId}/environments/${ambiente}`, {
      method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ version: versao })
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    mensagem(`Versão ${versao} publicada em ${ambiente}.`);
    window.setTimeout(() => window.location.reload(), 500);
  }));
  document.querySelector("#form-participante")?.addEventListener("submit", async (evento) => {
    evento.preventDefault();
    const account = document.querySelector("#conta-participante").value.trim();
    const role = document.querySelector("#papel-participante").value;
    const resposta = await fetch(`/api/v1/apps/${appId}/members`, {
      method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ account, role })
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    mensagem("Participante atualizado.");
    window.setTimeout(() => window.location.reload(), 400);
  });
  document.querySelectorAll(".remover-participante").forEach((botao) => botao.addEventListener("click", async () => {
    const account = botao.closest("[data-account]").dataset.account;
    const resposta = await fetch(`/api/v1/apps/${appId}/members`, {
      method: "DELETE", headers: { "content-type": "application/json" }, body: JSON.stringify({ account })
    });
    if (!resposta.ok) { mensagem(await erroDaResposta(resposta), true); return; }
    mensagem("Participante removido.");
    window.setTimeout(() => window.location.reload(), 400);
  }));
  renderizar();
})();
