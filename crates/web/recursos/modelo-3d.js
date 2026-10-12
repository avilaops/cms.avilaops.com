// O visualizador de modelo 3D do CMS: o elemento <modelo-3d src="…glb">.
//
// Abre glTF 2.0 binário (GLB) com WebGL, sem biblioteca de terceiros: malhas de
// triângulos, cor e textura de base, metal e aspereza, hierarquia de nós. O
// que o elemento traz dentro (a capa) fica na tela até o modelo estar pronto,
// e continua sendo o que aparece se o navegador não tiver WebGL.

const VERTICE = `
attribute vec3 posicao;
attribute vec3 normal;
attribute vec2 uv;
uniform mat4 projecao;
uniform mat4 visao;
uniform mat4 mundo;
uniform mat3 mundoDasNormais;
varying vec3 vNormal;
varying vec3 vPosicao;
varying vec2 vUv;
void main() {
  vec4 noMundo = mundo * vec4(posicao, 1.0);
  vPosicao = noMundo.xyz;
  vNormal = mundoDasNormais * normal;
  vUv = uv;
  gl_Position = projecao * visao * noMundo;
}`;

// Luz de estúdio fixa na câmera: uma principal, uma de preenchimento e a do
// ambiente (mais clara de cima que de baixo). As contas são em cor linear.
const FRAGMENTO = `
precision mediump float;
uniform vec4 cor;
uniform vec3 emissao;
uniform float metal;
uniform float aspereza;
uniform bool temTextura;
uniform sampler2D textura;
uniform vec3 camera;
uniform vec3 luzPrincipal;
uniform vec3 luzDeApoio;
varying vec3 vNormal;
varying vec3 vPosicao;
varying vec2 vUv;
void main() {
  vec4 base = cor;
  if (temTextura) {
    vec4 t = texture2D(textura, vUv);
    base *= vec4(pow(t.rgb, vec3(2.2)), t.a);
  }
  vec3 n = normalize(vNormal);
  vec3 v = normalize(camera - vPosicao);
  // As duas faces são desenhadas: a de trás recebe a normal virada.
  if (dot(n, v) < 0.0) n = -n;
  float brilho = mix(96.0, 6.0, aspereza);
  vec3 especular = mix(vec3(0.04), base.rgb, metal) * (1.0 - aspereza * 0.75);
  vec3 difusa = base.rgb * (1.0 - metal * 0.85);
  vec3 ambiente = mix(vec3(0.32, 0.31, 0.30), vec3(0.62, 0.64, 0.68), n.y * 0.5 + 0.5);
  vec3 soma = difusa * ambiente + especular * ambiente * 0.6;
  float d1 = max(dot(n, luzPrincipal), 0.0);
  float e1 = pow(max(dot(n, normalize(luzPrincipal + v)), 0.0), brilho);
  soma += (difusa * d1 + especular * e1 * d1 * 2.0) * 1.9;
  float d2 = max(dot(n, luzDeApoio), 0.0);
  soma += difusa * d2 * 0.55;
  soma += emissao;
  gl_FragColor = vec4(pow(soma, vec3(1.0 / 2.2)), base.a);
}`;

// ---- Matrizes 4×4, em coluna, como o WebGL e o glTF usam. ----

function multiplicar(a, b) {
  const r = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let l = 0; l < 4; l++) {
      r[c * 4 + l] =
        a[l] * b[c * 4] + a[4 + l] * b[c * 4 + 1] + a[8 + l] * b[c * 4 + 2] + a[12 + l] * b[c * 4 + 3];
    }
  }
  return r;
}

function deNo(no) {
  if (no.matrix) return new Float32Array(no.matrix);
  const [x, y, z, w] = no.rotation || [0, 0, 0, 1];
  const [sx, sy, sz] = no.scale || [1, 1, 1];
  const [tx, ty, tz] = no.translation || [0, 0, 0];
  return new Float32Array([
    (1 - 2 * (y * y + z * z)) * sx, 2 * (x * y + z * w) * sx, 2 * (x * z - y * w) * sx, 0,
    2 * (x * y - z * w) * sy, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z + x * w) * sy, 0,
    2 * (x * z + y * w) * sz, 2 * (y * z - x * w) * sz, (1 - 2 * (x * x + y * y)) * sz, 0,
    tx, ty, tz, 1,
  ]);
}

// A inversa transposta da parte 3×3: o que leva as normais para o mundo
// mesmo quando o nó tem escala diferente em cada eixo.
function dasNormais(m) {
  const a = m[0], b = m[1], c = m[2], d = m[4], e = m[5], f = m[6], g = m[8], h = m[9], i = m[10];
  const A = e * i - f * h, B = f * g - d * i, C = d * h - e * g;
  const det = a * A + b * B + c * C || 1;
  return new Float32Array([
    A / det, B / det, C / det,
    (c * h - b * i) / det, (a * i - c * g) / det, (b * g - a * h) / det,
    (b * f - c * e) / det, (c * d - a * f) / det, (a * e - b * d) / det,
  ]);
}

function perspectiva(abertura, proporcao, perto, longe) {
  const f = 1 / Math.tan(abertura / 2);
  const r = new Float32Array(16);
  r[0] = f / proporcao;
  r[5] = f;
  r[10] = (longe + perto) / (perto - longe);
  r[11] = -1;
  r[14] = (2 * longe * perto) / (perto - longe);
  return r;
}

function olhar(olho, alvo) {
  let fx = olho[0] - alvo[0], fy = olho[1] - alvo[1], fz = olho[2] - alvo[2];
  const fl = Math.hypot(fx, fy, fz) || 1;
  fx /= fl; fy /= fl; fz /= fl;
  // O "para cima" é sempre o eixo Y; a órbita nunca chega aos polos.
  let rx = fz, rz = -fx;
  const rl = Math.hypot(rx, rz) || 1;
  rx /= rl; rz /= rl;
  const ux = fy * rz, uy = fz * rx - fx * rz, uz = -fy * rx;
  return new Float32Array([
    rx, ux, fx, 0,
    0, uy, fy, 0,
    rz, uz, fz, 0,
    -(rx * olho[0] + rz * olho[2]),
    -(ux * olho[0] + uy * olho[1] + uz * olho[2]),
    -(fx * olho[0] + fy * olho[1] + fz * olho[2]),
    1,
  ]);
}

// ---- Leitura do GLB. ----

const POR_TIPO = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
const VETORES = {
  5120: Int8Array, 5121: Uint8Array, 5122: Int16Array, 5123: Uint16Array, 5125: Uint32Array, 5126: Float32Array,
};
const MAXIMOS = { 5120: 127, 5121: 255, 5122: 32767, 5123: 65535 };

function abrirGlb(arquivo) {
  const dados = new DataView(arquivo);
  if (dados.byteLength < 20 || dados.getUint32(0, true) !== 0x46546c67 || dados.getUint32(4, true) !== 2) {
    throw new Error("não é GLB 2.0");
  }
  let json = null;
  let binario = null;
  for (let i = 12; i + 8 <= dados.byteLength; ) {
    const tamanho = dados.getUint32(i, true);
    const tipo = dados.getUint32(i + 4, true);
    if (tipo === 0x4e4f534a) {
      json = JSON.parse(new TextDecoder().decode(new Uint8Array(arquivo, i + 8, tamanho)));
    } else if (tipo === 0x004e4942 && !binario) {
      binario = new Uint8Array(arquivo, i + 8, tamanho);
    }
    i += 8 + tamanho + ((4 - (tamanho % 4)) % 4);
  }
  if (!json) throw new Error("GLB sem JSON");
  if ((json.extensionsRequired || []).length) throw new Error("GLB com extensão obrigatória");
  return { json, binario: binario || new Uint8Array(0) };
}

// Os valores de um acessor em um vetor só dele, sem intercalação. Quando
// `emFloat`, inteiro normalizado vira fração (é assim que UV compactada vem).
function ler(glb, indice, emFloat) {
  const acessor = glb.json.accessors[indice];
  const Vetor = VETORES[acessor.componentType];
  const porItem = POR_TIPO[acessor.type];
  const total = acessor.count * porItem;
  if (!Vetor || !porItem) throw new Error("acessor desconhecido");
  if (acessor.bufferView === undefined) return emFloat ? new Float32Array(total) : new Vetor(total);
  const vista = glb.json.bufferViews[acessor.bufferView];
  const inicio = glb.binario.byteOffset + (vista.byteOffset || 0) + (acessor.byteOffset || 0);
  const passo = vista.byteStride || porItem * Vetor.BYTES_PER_ELEMENT;
  const origem = new DataView(glb.binario.buffer, inicio);
  const pegar = {
    5120: "getInt8", 5121: "getUint8", 5122: "getInt16", 5123: "getUint16", 5125: "getUint32", 5126: "getFloat32",
  }[acessor.componentType];
  const saida = emFloat ? new Float32Array(total) : new Vetor(total);
  const divisor = emFloat && acessor.normalized ? MAXIMOS[acessor.componentType] || 1 : 1;
  for (let item = 0; item < acessor.count; item++) {
    for (let c = 0; c < porItem; c++) {
      const valor = origem[pegar](item * passo + c * Vetor.BYTES_PER_ELEMENT, true);
      saida[item * porItem + c] = valor / divisor;
    }
  }
  return saida;
}

// Normais de face, para a malha que veio sem elas.
function normaisDeFace(posicoes, indices) {
  const normais = new Float32Array(posicoes.length);
  const total = indices ? indices.length : posicoes.length / 3;
  for (let i = 0; i + 2 < total; i += 3) {
    const [a, b, c] = [0, 1, 2].map((k) => (indices ? indices[i + k] : i + k) * 3);
    const ux = posicoes[b] - posicoes[a], uy = posicoes[b + 1] - posicoes[a + 1], uz = posicoes[b + 2] - posicoes[a + 2];
    const vx = posicoes[c] - posicoes[a], vy = posicoes[c + 1] - posicoes[a + 1], vz = posicoes[c + 2] - posicoes[a + 2];
    const n = [uy * vz - uz * vy, uz * vx - ux * vz, ux * vy - uy * vx];
    for (const v of [a, b, c]) {
      normais[v] += n[0];
      normais[v + 1] += n[1];
      normais[v + 2] += n[2];
    }
  }
  return normais;
}

async function imagemDe(glb, indice) {
  const imagem = glb.json.images[indice];
  let blob;
  if (imagem.bufferView !== undefined) {
    const vista = glb.json.bufferViews[imagem.bufferView];
    const bytes = glb.binario.subarray(vista.byteOffset || 0, (vista.byteOffset || 0) + vista.byteLength);
    blob = new Blob([bytes], { type: imagem.mimeType || "image/png" });
  } else if (imagem.uri && imagem.uri.startsWith("data:")) {
    const [cabecalho, base64] = imagem.uri.split(",");
    const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
    blob = new Blob([bytes], { type: cabecalho.slice(5).split(";")[0] });
  } else {
    return null;
  }
  return createImageBitmap(blob);
}

// ---- A cena no WebGL. ----

function programa(gl) {
  const compilar = (tipo, fonte) => {
    const s = gl.createShader(tipo);
    gl.shaderSource(s, fonte);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const p = gl.createProgram();
  gl.attachShader(p, compilar(gl.VERTEX_SHADER, VERTICE));
  gl.attachShader(p, compilar(gl.FRAGMENT_SHADER, FRAGMENTO));
  gl.bindAttribLocation(p, 0, "posicao");
  gl.bindAttribLocation(p, 1, "normal");
  gl.bindAttribLocation(p, 2, "uv");
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
  const onde = {};
  for (const nome of [
    "projecao", "visao", "mundo", "mundoDasNormais", "cor", "emissao", "metal", "aspereza", "temTextura",
    "textura", "camera", "luzPrincipal", "luzDeApoio",
  ]) {
    onde[nome] = gl.getUniformLocation(p, nome);
  }
  return { p, onde };
}

async function montarCena(gl, glb) {
  const { json } = glb;
  const versao2 = typeof WebGL2RenderingContext !== "undefined" && gl instanceof WebGL2RenderingContext;
  const indices32 = versao2 || !!gl.getExtension("OES_element_index_uint");

  const texturas = await Promise.all(
    (json.textures || []).map(async (textura) => {
      const imagem = textura.source === undefined ? null : await imagemDe(glb, textura.source).catch(() => null);
      if (!imagem) return null;
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, imagem);
      const potencia = (n) => (n & (n - 1)) === 0;
      const repete = versao2 || (potencia(imagem.width) && potencia(imagem.height));
      const amostra = (json.samplers || [])[textura.sampler] || {};
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, repete ? amostra.wrapS || gl.REPEAT : gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, repete ? amostra.wrapT || gl.REPEAT : gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      if (repete) {
        gl.generateMipmap(gl.TEXTURE_2D);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      } else {
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      }
      return t;
    }),
  );

  const materiais = (json.materials || []).map((material) => {
    const pbr = material.pbrMetallicRoughness || {};
    return {
      cor: pbr.baseColorFactor || [1, 1, 1, 1],
      textura: pbr.baseColorTexture ? texturas[pbr.baseColorTexture.index] : null,
      metal: pbr.metallicFactor === undefined ? 1 : pbr.metallicFactor,
      aspereza: pbr.roughnessFactor === undefined ? 1 : pbr.roughnessFactor,
      emissao: material.emissiveFactor || [0, 0, 0],
      transparente: material.alphaMode === "BLEND",
    };
  });
  const padrao = { cor: [0.8, 0.8, 0.8, 1], textura: null, metal: 0, aspereza: 0.6, emissao: [0, 0, 0] };

  const vetor = (alvo, dados) => {
    const b = gl.createBuffer();
    gl.bindBuffer(alvo, b);
    gl.bufferData(alvo, dados, gl.STATIC_DRAW);
    return b;
  };
  const malhas = (json.meshes || []).map((malha) =>
    malha.primitives
      .filter((parte) => (parte.mode === undefined || parte.mode === 4) && parte.attributes.POSITION !== undefined)
      .map((parte) => {
        const posicoes = ler(glb, parte.attributes.POSITION, true);
        let indices = parte.indices === undefined ? null : ler(glb, parte.indices, false);
        if (indices instanceof Uint32Array && !indices32) return null;
        if (indices instanceof Uint8Array) indices = Uint16Array.from(indices);
        const normais =
          parte.attributes.NORMAL === undefined
            ? normaisDeFace(posicoes, indices)
            : ler(glb, parte.attributes.NORMAL, true);
        const uvs = parte.attributes.TEXCOORD_0 === undefined ? null : ler(glb, parte.attributes.TEXCOORD_0, true);
        const limites = glb.json.accessors[parte.attributes.POSITION];
        return {
          posicoes: vetor(gl.ARRAY_BUFFER, posicoes),
          normais: vetor(gl.ARRAY_BUFFER, normais),
          uvs: uvs && vetor(gl.ARRAY_BUFFER, uvs),
          indices: indices && vetor(gl.ELEMENT_ARRAY_BUFFER, indices),
          tipoDeIndice: indices instanceof Uint32Array ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT,
          total: indices ? indices.length : posicoes.length / 3,
          material: materiais[parte.material] || padrao,
          minimo: limites.min || [-1, -1, -1],
          maximo: limites.max || [1, 1, 1],
        };
      })
      .filter(Boolean),
  );

  // Percorre a cena e guarda cada parte com a matriz dela no mundo.
  const desenhos = [];
  const menor = [Infinity, Infinity, Infinity];
  const maior = [-Infinity, -Infinity, -Infinity];
  const visitar = (indice, mae, vistos) => {
    const no = json.nodes[indice];
    if (!no || vistos.has(indice)) return;
    vistos.add(indice);
    const mundo = multiplicar(mae, deNo(no));
    for (const parte of malhas[no.mesh] || []) {
      desenhos.push({ parte, mundo, normais: dasNormais(mundo) });
      for (let canto = 0; canto < 8; canto++) {
        const p = [0, 1, 2].map((eixo) => ((canto >> eixo) & 1 ? parte.maximo : parte.minimo)[eixo]);
        for (let eixo = 0; eixo < 3; eixo++) {
          const v = mundo[eixo] * p[0] + mundo[4 + eixo] * p[1] + mundo[8 + eixo] * p[2] + mundo[12 + eixo];
          menor[eixo] = Math.min(menor[eixo], v);
          maior[eixo] = Math.max(maior[eixo], v);
        }
      }
    }
    for (const filho of no.children || []) visitar(filho, mundo, vistos);
  };
  const cena = (json.scenes || [])[json.scene || 0];
  const raizes = cena ? cena.nodes || [] : (json.nodes || []).map((_, i) => i);
  const identidade = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
  const vistos = new Set();
  for (const raiz of raizes) visitar(raiz, identidade, vistos);
  if (!desenhos.length) throw new Error("GLB sem malha");
  // O que é transparente é desenhado por último, por cima do resto.
  desenhos.sort((a, b) => Number(!!a.parte.material.transparente) - Number(!!b.parte.material.transparente));

  const centro = [0, 1, 2].map((eixo) => (menor[eixo] + maior[eixo]) / 2);
  const raio = Math.hypot(maior[0] - menor[0], maior[1] - menor[1], maior[2] - menor[2]) / 2 || 1;
  return { desenhos, centro, raio };
}

const ABERTURA = (30 * Math.PI) / 180;

class Modelo3d extends HTMLElement {
  connectedCallback() {
    if (this.iniciado) return;
    this.iniciado = true;
    const endereco = this.getAttribute("src");
    if (!endereco) return;
    this.atalhoParaOEspaco(endereco);
    // O arquivo só é baixado quando o modelo está perto de aparecer.
    const observador = new IntersectionObserver(
      (entradas) => {
        if (entradas.some((entrada) => entrada.isIntersecting)) {
          observador.disconnect();
          this.carregar(endereco).catch(() => this.classList.add("sem-3d"));
        }
      },
      { rootMargin: "300px" },
    );
    observador.observe(this);
  }

  // "Ver no seu espaço": no iPhone, pelo USDZ; no Android, pelo próprio GLB.
  atalhoParaOEspaco(endereco) {
    const usdz = this.getAttribute("ios-src");
    const link = document.createElement("a");
    if (usdz && link.relList && link.relList.supports && link.relList.supports("ar")) {
      link.rel = "ar";
      link.href = usdz;
      // O iPhone só abre a câmera se o link tiver uma imagem dentro.
      const imagem = new Image(1, 1);
      imagem.alt = "";
      imagem.src = "data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==";
      link.append(imagem);
    } else if (/Android/.test(navigator.userAgent)) {
      const arquivo = new URL(endereco, location.href).href;
      link.href =
        `intent://arvr.google.com/scene-viewer/1.0?file=${encodeURIComponent(arquivo)}&mode=ar_preferred` +
        `#Intent;scheme=https;package=com.google.android.googlequicksearchbox;` +
        `action=android.intent.action.VIEW;S.browser_fallback_url=${encodeURIComponent(location.href)};end;`;
    } else {
      return;
    }
    link.className = "no-espaco";
    link.append("Ver no seu espaço");
    this.append(link);
  }

  async carregar(endereco) {
    const tela = document.createElement("canvas");
    const opcoes = { antialias: true, alpha: true, premultipliedAlpha: false };
    const gl = tela.getContext("webgl2", opcoes) || tela.getContext("webgl", opcoes);
    if (!gl) throw new Error("sem WebGL");
    const resposta = await fetch(endereco);
    if (!resposta.ok) throw new Error("modelo não encontrado");
    const glb = abrirGlb(await resposta.arrayBuffer());
    const { p, onde } = programa(gl);
    const cena = await montarCena(gl, glb);

    tela.tabIndex = 0;
    tela.setAttribute("role", "img");
    tela.setAttribute(
      "aria-label",
      `${this.getAttribute("alt") || "Modelo 3D"}. Arraste ou use as setas para girar; + e − aproximam e afastam.`,
    );
    this.append(tela);
    this.classList.add("pronto");

    // A órbita: ângulo em volta, ângulo de altura e a distância.
    const longe = (cena.raio / Math.sin(ABERTURA / 2)) * 1.08;
    const camera = { volta: -0.6, altura: 0.28, distancia: longe };
    const parado = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let girando = !parado;
    let visivel = true;
    let pedido = 0;
    let antes = 0;

    const desenhar = (agora) => {
      pedido = 0;
      if (girando && antes) camera.volta += Math.min(agora - antes, 64) * 0.00025;
      antes = agora;
      const densidade = Math.min(devicePixelRatio || 1, 2);
      const largura = Math.max(1, Math.round(tela.clientWidth * densidade));
      const altura = Math.max(1, Math.round(tela.clientHeight * densidade));
      if (tela.width !== largura || tela.height !== altura) {
        tela.width = largura;
        tela.height = altura;
      }
      gl.viewport(0, 0, largura, altura);
      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
      gl.enable(gl.DEPTH_TEST);
      gl.useProgram(p);

      const proporcao = largura / altura;
      // Em tela estreita o modelo é enquadrado pela largura.
      const distancia = camera.distancia * Math.max(1, 0.9 / proporcao);
      const cosseno = Math.cos(camera.altura);
      const olho = [
        cena.centro[0] + distancia * cosseno * Math.sin(camera.volta),
        cena.centro[1] + distancia * Math.sin(camera.altura),
        cena.centro[2] + distancia * cosseno * Math.cos(camera.volta),
      ];
      const visao = olhar(olho, cena.centro);
      gl.uniformMatrix4fv(onde.projecao, false, perspectiva(ABERTURA, proporcao, distancia / 100, distancia * 4));
      gl.uniformMatrix4fv(onde.visao, false, visao);
      gl.uniform3fv(onde.camera, olho);
      // As luzes acompanham a câmera: vêm de cima, uma de cada lado.
      const daCamera = (x, y, z) => {
        const d = [
          visao[0] * x + visao[1] * y + visao[2] * z,
          visao[4] * x + visao[5] * y + visao[6] * z,
          visao[8] * x + visao[9] * y + visao[10] * z,
        ];
        const tamanho = Math.hypot(d[0], d[1], d[2]) || 1;
        return d.map((valor) => valor / tamanho);
      };
      gl.uniform3fv(onde.luzPrincipal, daCamera(0.55, 0.75, 0.6));
      gl.uniform3fv(onde.luzDeApoio, daCamera(-0.7, 0.2, 0.5));
      gl.uniform1i(onde.textura, 0);

      for (const { parte, mundo, normais } of cena.desenhos) {
        const material = parte.material;
        gl.uniformMatrix4fv(onde.mundo, false, mundo);
        gl.uniformMatrix3fv(onde.mundoDasNormais, false, normais);
        gl.uniform4fv(onde.cor, material.cor);
        gl.uniform3fv(onde.emissao, material.emissao);
        gl.uniform1f(onde.metal, material.metal);
        gl.uniform1f(onde.aspereza, material.aspereza);
        gl.uniform1i(onde.temTextura, material.textura && parte.uvs ? 1 : 0);
        if (material.textura) {
          gl.activeTexture(gl.TEXTURE0);
          gl.bindTexture(gl.TEXTURE_2D, material.textura);
        }
        if (material.transparente) {
          gl.enable(gl.BLEND);
          gl.blendFuncSeparate(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA, gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
        } else {
          gl.disable(gl.BLEND);
        }
        gl.bindBuffer(gl.ARRAY_BUFFER, parte.posicoes);
        gl.enableVertexAttribArray(0);
        gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
        gl.bindBuffer(gl.ARRAY_BUFFER, parte.normais);
        gl.enableVertexAttribArray(1);
        gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);
        if (parte.uvs) {
          gl.bindBuffer(gl.ARRAY_BUFFER, parte.uvs);
          gl.enableVertexAttribArray(2);
          gl.vertexAttribPointer(2, 2, gl.FLOAT, false, 0, 0);
        } else {
          gl.disableVertexAttribArray(2);
        }
        if (parte.indices) {
          gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, parte.indices);
          gl.drawElements(gl.TRIANGLES, parte.total, parte.tipoDeIndice, 0);
        } else {
          gl.drawArrays(gl.TRIANGLES, 0, parte.total);
        }
      }
      if (girando && visivel) pedir();
    };
    const pedir = () => {
      if (!pedido) pedido = requestAnimationFrame(desenhar);
    };
    const mexer = (volta, altura, zoom) => {
      girando = false;
      camera.volta += volta;
      camera.altura = Math.max(-1.35, Math.min(1.35, camera.altura + altura));
      camera.distancia = Math.max(longe * 0.35, Math.min(longe * 2.5, camera.distancia * zoom));
      pedir();
    };

    // Um dedo ou o mouse giram; dois dedos aproximam e afastam.
    const dedos = new Map();
    let abertura = 0;
    tela.addEventListener("pointerdown", (evento) => {
      dedos.set(evento.pointerId, [evento.clientX, evento.clientY]);
      tela.setPointerCapture(evento.pointerId);
      abertura = 0;
    });
    tela.addEventListener("pointermove", (evento) => {
      const anterior = dedos.get(evento.pointerId);
      if (!anterior) return;
      dedos.set(evento.pointerId, [evento.clientX, evento.clientY]);
      if (dedos.size === 1) {
        mexer((anterior[0] - evento.clientX) * 0.008, (evento.clientY - anterior[1]) * 0.008, 1);
      } else if (dedos.size === 2) {
        const [a, b] = [...dedos.values()];
        const agora = Math.hypot(a[0] - b[0], a[1] - b[1]);
        if (abertura && agora) mexer(0, 0, abertura / agora);
        abertura = agora;
      }
    });
    const soltar = (evento) => {
      dedos.delete(evento.pointerId);
      abertura = 0;
    };
    tela.addEventListener("pointerup", soltar);
    tela.addEventListener("pointercancel", soltar);
    // A roda só aproxima com Ctrl (é o gesto de pinça no touchpad): sem isso,
    // rolar a página por cima do modelo continua rolando a página.
    tela.addEventListener(
      "wheel",
      (evento) => {
        if (!evento.ctrlKey) return;
        evento.preventDefault();
        mexer(0, 0, Math.exp(evento.deltaY * 0.01));
      },
      { passive: false },
    );
    tela.addEventListener("keydown", (evento) => {
      const passos = {
        ArrowLeft: [-0.15, 0, 1], ArrowRight: [0.15, 0, 1], ArrowUp: [0, 0.1, 1], ArrowDown: [0, -0.1, 1],
        "+": [0, 0, 0.9], "=": [0, 0, 0.9], "-": [0, 0, 1.1],
      }[evento.key];
      if (!passos) return;
      evento.preventDefault();
      mexer(...passos);
    });

    new ResizeObserver(pedir).observe(tela);
    // Fora da tela o modelo não gasta bateria.
    new IntersectionObserver((entradas) => {
      visivel = entradas.some((entrada) => entrada.isIntersecting);
      antes = 0;
      if (visivel) pedir();
    }).observe(tela);
    tela.addEventListener("webglcontextlost", (evento) => {
      evento.preventDefault();
      girando = false;
    });
    pedir();
  }
}

customElements.define("modelo-3d", Modelo3d);
