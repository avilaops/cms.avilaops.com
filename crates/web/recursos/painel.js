// O script do painel. Toda tela funciona sem ele: aqui entram só a janela
// de cada imagem (que sem script abre pelo endereço #imagem-...) e a prévia
// ao vivo da identidade.
"use strict";

document.documentElement.classList.add("com-script");

// A janela de cada imagem.
function abrirJanela(janela) {
  if (janela instanceof HTMLDialogElement && !janela.open) {
    janela.showModal();
  }
}

document.addEventListener("click", (evento) => {
  const alvo = evento.target;
  if (!(alvo instanceof Element)) {
    return;
  }
  const abre = alvo.closest("[data-abre]");
  if (abre) {
    const janela = document.getElementById(abre.dataset.abre);
    if (janela instanceof HTMLDialogElement) {
      evento.preventDefault();
      abrirJanela(janela);
    }
    return;
  }
  const fecha = alvo.closest("[data-fecha]");
  if (fecha) {
    const janela = fecha.closest("dialog");
    if (janela && janela.open) {
      evento.preventDefault();
      janela.close();
    }
    return;
  }
  // O clique no fundo escuro cai no próprio diálogo: fecha.
  if (alvo instanceof HTMLDialogElement && alvo.open) {
    alvo.close();
  }
});

if (location.hash.startsWith("#imagem-")) {
  abrirJanela(document.getElementById(location.hash.slice(1)));
}

// A prévia da identidade.
const previa = document.querySelector("[data-previa]");
const formulario = document.getElementById("identidade");

if (previa && formulario) {
  const COR = /^#[0-9a-fA-F]{6}$/;
  const campo = (nome) => formulario.elements.namedItem(nome);
  const miniatura = (id) => {
    const escolha = document.getElementById(id);
    const opcao = escolha instanceof HTMLSelectElement ? escolha.selectedOptions[0] : null;
    return (opcao && opcao.dataset.miniatura) || "";
  };
  const mostrar = (papel, endereco) => {
    for (const imagem of document.querySelectorAll(`img[data-pv="${papel}"]`)) {
      if (endereco) {
        imagem.src = endereco;
        imagem.hidden = false;
      } else {
        imagem.removeAttribute("src");
        imagem.hidden = true;
      }
    }
  };
  const escrever = (papel, texto) => {
    for (const lugar of document.querySelectorAll(`[data-pv="${papel}"]`)) {
      lugar.textContent = texto;
    }
  };

  const seletorDeCor = document.getElementById("cor-escolha");
  const textoDaCor = campo("cor");
  if (seletorDeCor instanceof HTMLInputElement && textoDaCor instanceof HTMLInputElement) {
    seletorDeCor.hidden = false;
    if (COR.test(textoDaCor.value.trim())) {
      seletorDeCor.value = textoDaCor.value.trim();
    }
    seletorDeCor.addEventListener("input", () => {
      textoDaCor.value = seletorDeCor.value;
      atualizar();
    });
  }

  function atualizar() {
    const nome = campo("nome").value.trim() || "Nome do site";
    escrever("nome", nome);
    escrever("titular", campo("razao_social").value.trim() || nome);

    const logo = miniatura("logo");
    const favicon = miniatura("favicon") || logo;
    mostrar("logo", logo);
    mostrar("favicon", favicon);
    mostrar("icone-apple", miniatura("icone_apple") || favicon);

    const escolhido = formulario.querySelector('input[name="tema"]:checked');
    const amostra = escolhido && escolhido.closest("label").querySelector(".miniatura-do-tema");
    const site = previa.querySelector(".pv-site");
    if (amostra && site) {
      for (const variavel of ["--fundo", "--texto", "--fonte", "--cantos", "--destaque"]) {
        site.style.setProperty(variavel, amostra.style.getPropertyValue(variavel));
      }
    }
    const cor = textoDaCor ? textoDaCor.value.trim() : "";
    if (COR.test(cor)) {
      site && site.style.setProperty("--destaque", cor);
      if (seletorDeCor) {
        seletorDeCor.value = cor;
      }
    }
    const destaque = site ? site.style.getPropertyValue("--destaque") : "";
    for (const amostraDaCor of document.querySelectorAll(".amostra-da-cor")) {
      amostraDaCor.style.setProperty("--destaque", destaque);
    }
  }

  formulario.addEventListener("input", atualizar);
  formulario.addEventListener("change", atualizar);
}
