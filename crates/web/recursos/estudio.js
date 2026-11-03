(() => {
  "use strict";

  // Mesma conta de `cms_dominio::estudio::ponto_no_quadro`.
  function pontoNoQuadro(clienteX, clienteY, origemX, origemY, panX, panY, zoom) {
    if (!Number.isFinite(zoom) || zoom < 0.25 || zoom > 4) return null;
    const numeros = [clienteX, clienteY, origemX, origemY, panX, panY];
    if (numeros.some((numero) => !Number.isFinite(numero))) return null;
    return {
      x: Math.round((clienteX - origemX - panX) / zoom),
      y: Math.round((clienteY - origemY - panY) / zoom),
    };
  }

  document.querySelectorAll("[data-abas]").forEach((raiz) => {
    const botoes = [...raiz.querySelectorAll("[data-aba]")];
    const paineis = [...raiz.querySelectorAll("[data-painel]")];
    const mostrar = (indice) => {
      botoes.forEach((botao, i) => {
        const ativo = i === indice;
        botao.setAttribute("aria-selected", ativo ? "true" : "false");
      });
      paineis.forEach((painel, i) => {
        painel.hidden = i !== indice;
      });
    };
    botoes.forEach((botao, indice) => {
      botao.addEventListener("click", () => mostrar(indice));
    });
    if (botoes.length) mostrar(0);
  });

  const quadro = document.querySelector("#quadro");
  const camera = document.querySelector("#camera");
  if (!quadro || !camera) return;

  let zoom = 1;
  let panX = 0;
  let panY = 0;
  let ferramenta = "cursor";
  let arraste = null;
  const somenteLeitura = quadro.dataset.somenteLeitura === "1";
  const rotulo = document.querySelector("#zoom");

  const aplicar = () => {
    camera.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`;
    if (rotulo) rotulo.textContent = `${Math.round(zoom * 100)}%`;
  };

  const ponto = (clienteX, clienteY) => {
    const origem = quadro.getBoundingClientRect();
    return pontoNoQuadro(clienteX, clienteY, origem.left, origem.top, panX, panY, zoom);
  };

  const selecionar = (no) => {
    quadro.querySelectorAll(".no").forEach((item) => {
      const ativo = item === no;
      item.classList.toggle("selecionado", ativo);
      item.setAttribute("aria-selected", ativo ? "true" : "false");
    });
  };

  document.querySelectorAll("[data-ferramenta]").forEach((formulario) => {
    formulario.addEventListener("submit", (evento) => {
      if (evento.submitter && evento.submitter.dataset.colocar !== "1") {
        evento.preventDefault();
        ferramenta = formulario.dataset.ferramenta;
        quadro.dataset.ferramenta = ferramenta;
      }
    });
  });

  document.querySelectorAll("[data-escolha]").forEach((botao) => {
    botao.addEventListener("click", () => {
      ferramenta = botao.dataset.escolha;
      quadro.dataset.ferramenta = ferramenta;
    });
  });

  quadro.addEventListener("wheel", (evento) => {
    evento.preventDefault();
    const origem = quadro.getBoundingClientRect();
    const antes = ponto(evento.clientX, evento.clientY);
    if (!antes) return;
    zoom = Math.min(4, Math.max(0.25, zoom * (evento.deltaY < 0 ? 1.1 : 1 / 1.1)));
    panX = evento.clientX - origem.left - antes.x * zoom;
    panY = evento.clientY - origem.top - antes.y * zoom;
    aplicar();
  }, { passive: false });

  rotulo?.addEventListener("click", () => {
    zoom = 1;
    panX = 0;
    panY = 0;
    aplicar();
  });

  quadro.addEventListener("pointerdown", (evento) => {
    if (evento.button !== 0) return;
    const no = evento.target.closest(".no");
    if (ferramenta === "mao") {
      arraste = { tipo: "pan", x: evento.clientX, y: evento.clientY, panX, panY };
      quadro.setPointerCapture(evento.pointerId);
      return;
    }
    if (no && ferramenta === "cursor") {
      selecionar(no);
      if (somenteLeitura || !no.dataset.mover) return;
      const local = ponto(evento.clientX, evento.clientY);
      if (!local) return;
      arraste = {
        tipo: "no",
        no,
        dx: local.x - Number.parseInt(no.style.left, 10),
        dy: local.y - Number.parseInt(no.style.top, 10),
      };
      quadro.setPointerCapture(evento.pointerId);
      return;
    }
    if (!no && !somenteLeitura && ["frame", "retangulo", "texto"].includes(ferramenta)) {
      const local = ponto(evento.clientX, evento.clientY);
      const formulario = document.querySelector(`form[data-ferramenta="${ferramenta}"]`);
      if (!local || !formulario) return;
      formulario.querySelector("[name=x]").value = String(local.x);
      formulario.querySelector("[name=y]").value = String(local.y);
      formulario.querySelector("[data-colocar]").click();
    }
  });

  quadro.addEventListener("pointermove", (evento) => {
    if (!arraste) return;
    if (arraste.tipo === "pan") {
      panX = arraste.panX + (evento.clientX - arraste.x);
      panY = arraste.panY + (evento.clientY - arraste.y);
      aplicar();
      return;
    }
    const local = ponto(evento.clientX, evento.clientY);
    if (!local) return;
    arraste.no.style.left = `${local.x - arraste.dx}px`;
    arraste.no.style.top = `${local.y - arraste.dy}px`;
  });

  const soltar = async () => {
    if (!arraste || arraste.tipo !== "no") {
      arraste = null;
      return;
    }
    const no = arraste.no;
    const destino = no.dataset.mover;
    arraste = null;
    const corpo = new URLSearchParams({
      x: String(Number.parseInt(no.style.left, 10) || 0),
      y: String(Number.parseInt(no.style.top, 10) || 0),
    });
    try {
      await fetch(destino, {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: corpo,
      credentials: "same-origin",
      });
    } finally {
      // Releia a posição persistida, inclusive quando o servidor limita o ponto.
      window.location.reload();
    }
  };
  quadro.addEventListener("pointerup", soltar);
  quadro.addEventListener("pointercancel", soltar);
  aplicar();
})();
