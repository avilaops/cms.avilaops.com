//! Busca de arquivo em endereço informado por quem usa o CMS.
//!
//! O endereço vem de fora, então o servidor não pode ser levado a pedir algo
//! à própria rede: só `https` na porta padrão, o nome é resolvido antes e
//! recusado se algum endereço for de rede interna, a conexão vai para o
//! endereço que foi conferido e redirecionamento não é seguido.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use reqwest::Url;
use reqwest::header::CONTENT_LENGTH;
use reqwest::redirect::Policy;

const PRAZO: Duration = Duration::from_secs(20);
const PORTA_DE_HTTPS: u16 = 443;

/// As mensagens são para quem informou o endereço.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ErroDeBusca {
    #[error("O endereço precisa ser https://, sem usuário, senha nem porta.")]
    EnderecoRecusado,
    #[error("Este endereço aponta para uma rede interna e não pode ser buscado.")]
    RedeInterna,
    #[error("Não foi possível encontrar o servidor deste endereço.")]
    NomeNaoResolvido,
    #[error(
        "O servidor do endereço não entregou o arquivo (resposta {0}). Confira se o endereço abre direto no arquivo, sem redirecionar."
    )]
    Recusado(u16),
    #[error("O arquivo passa do tamanho máximo permitido.")]
    GrandeDemais,
    #[error("O servidor do endereço não respondeu a tempo.")]
    SemResposta,
}

/// Endereço que não sai para a internet: da própria máquina, de rede privada,
/// de enlace local, reservado ou de difusão.
fn eh_interno(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_interno(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4_interno(v4),
            None => v6_interno(v6),
        },
    }
}

fn v4_interno(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        // 0.0.0.0/8, a faixa de operadora (100.64.0.0/10) e a reservada (240.0.0.0/4).
        || a == 0
        || (a == 100 && (64..128).contains(&b))
        || a >= 240
}

fn v6_interno(ip: Ipv6Addr) -> bool {
    let primeiro = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        // Local única (fc00::/7) e de enlace (fe80::/10).
        || (primeiro & 0xfe00) == 0xfc00
        || (primeiro & 0xffc0) == 0xfe80
}

/// O host de um endereço aceito: `https`, porta padrão e sem credencial.
fn host_aceito(endereco: &str) -> Result<(Url, String), ErroDeBusca> {
    let url = Url::parse(endereco.trim()).map_err(|_| ErroDeBusca::EnderecoRecusado)?;
    let limpo = url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none();
    match url.host_str() {
        Some(host) if limpo => {
            let host = host.to_string();
            Ok((url, host))
        }
        _ => Err(ErroDeBusca::EnderecoRecusado),
    }
}

/// Resolve o host e devolve um endereço para conectar, desde que nenhum dos
/// endereços dele seja interno. O host pode ser um nome ou o próprio endereço.
async fn resolver(host: &str) -> Result<SocketAddr, ErroDeBusca> {
    // Endereço IPv6 vem entre colchetes na URL.
    let literal = host.trim_start_matches('[').trim_end_matches(']');
    let enderecos: Vec<SocketAddr> = match literal.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, PORTA_DE_HTTPS)],
        Err(_) => tokio::net::lookup_host((host, PORTA_DE_HTTPS))
            .await
            .map_err(|_| ErroDeBusca::NomeNaoResolvido)?
            .collect(),
    };
    if enderecos.iter().any(|endereco| eh_interno(endereco.ip())) {
        return Err(ErroDeBusca::RedeInterna);
    }
    enderecos
        .into_iter()
        .next()
        .ok_or(ErroDeBusca::NomeNaoResolvido)
}

/// Baixa o arquivo de um endereço informado por cliente, até `limite` bytes.
pub async fn baixar(endereco: &str, limite: usize) -> Result<Vec<u8>, ErroDeBusca> {
    let (url, host) = host_aceito(endereco)?;
    let destino = resolver(&host).await?;
    // A conexão vai para o endereço conferido: o nome não é resolvido de novo.
    let cliente = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(PRAZO)
        .resolve(&host, destino)
        .build()
        .map_err(|_| ErroDeBusca::SemResposta)?;
    let mut resposta = cliente
        .get(url)
        .send()
        .await
        .map_err(|_| ErroDeBusca::SemResposta)?;
    if !resposta.status().is_success() {
        return Err(ErroDeBusca::Recusado(resposta.status().as_u16()));
    }
    let anunciado = resposta
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.parse::<usize>().ok());
    if anunciado.is_some_and(|tamanho| tamanho > limite) {
        return Err(ErroDeBusca::GrandeDemais);
    }
    // O tamanho anunciado pode mentir: o corpo é lido aos pedaços, com teto.
    let mut bytes = Vec::with_capacity(anunciado.unwrap_or(0).min(limite));
    while let Some(pedaco) = resposta
        .chunk()
        .await
        .map_err(|_| ErroDeBusca::SemResposta)?
    {
        if bytes.len() + pedaco.len() > limite {
            return Err(ErroDeBusca::GrandeDemais);
        }
        bytes.extend_from_slice(&pedaco);
    }
    Ok(bytes)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn rede_interna_e_reconhecida_em_v4_e_v6() {
        for interno in [
            "127.0.0.1",
            "10.0.0.5",
            "172.31.0.17",
            "192.168.1.10",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "::ffff:10.0.0.5",
            "::ffff:127.0.0.1",
        ] {
            let ip: IpAddr = interno.parse().expect("endereço");
            assert!(eh_interno(ip), "{interno}");
        }
        for externo in ["1.1.1.1", "178.105.82.48", "2606:4700:4700::1111"] {
            let ip: IpAddr = externo.parse().expect("endereço");
            assert!(!eh_interno(ip), "{externo}");
        }
    }

    #[test]
    fn so_https_na_porta_padrao_e_sem_credencial() {
        assert!(host_aceito("https://exemplo.example/foto.webp").is_ok());
        for recusado in [
            "http://exemplo.example/foto.webp",
            "https://exemplo.example:8443/foto.webp",
            "https://usuario:senha@exemplo.example/foto.webp",
            "ftp://exemplo.example/foto.webp",
            "file:///etc/passwd",
            "foto.webp",
            "",
        ] {
            assert_eq!(
                host_aceito(recusado).map(|_| ()),
                Err(ErroDeBusca::EnderecoRecusado),
                "{recusado}"
            );
        }
    }

    #[tokio::test]
    async fn endereco_interno_e_recusado_antes_de_qualquer_pedido() {
        for interno in [
            "https://127.0.0.1/segredo",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/segredo",
            "https://localhost/segredo",
        ] {
            assert_eq!(
                baixar(interno, 1024).await,
                Err(ErroDeBusca::RedeInterna),
                "{interno}"
            );
        }
    }
}
