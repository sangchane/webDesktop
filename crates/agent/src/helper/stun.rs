//! 공인 주소 알아내기 — STUN Binding 하나(RFC 8489). str0m 은 후보를 모으지 않는다(04 「NIC enumeration and TURN」).
//! 이 주소를 후보로 알려야 LTE 의 폰(직접·TURN 중계 모두)이 집 PC 로 UDP 를 보낼 수 있다.
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_SUCCESS: u16 = 0x0101;
const XOR_MAPPED_ADDRESS: u16 = 0x0020;
const MAPPED_ADDRESS: u16 = 0x0001;

pub fn request(txid: [u8; 12]) -> [u8; 20] {
    let mut m = [0u8; 20];
    m[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    m[4..8].copy_from_slice(&MAGIC.to_be_bytes());
    m[8..20].copy_from_slice(&txid);
    m
}

/// 성공 응답에서 IPv4 매핑 주소를 꺼낸다. 다른 거래·꼴이 틀린 글은 None.
pub fn parse(msg: &[u8], txid: [u8; 12]) -> Option<SocketAddr> {
    if msg.len() < 20
        || u16::from_be_bytes([msg[0], msg[1]]) != BINDING_SUCCESS
        || msg[4..8] != MAGIC.to_be_bytes()
        || msg[8..20] != txid
    {
        return None;
    }
    let end = (20 + u16::from_be_bytes([msg[2], msg[3]]) as usize).min(msg.len());
    let mut i = 20;
    let mut plain = None;
    while i + 4 <= end {
        let kind = u16::from_be_bytes([msg[i], msg[i + 1]]);
        let len = u16::from_be_bytes([msg[i + 2], msg[i + 3]]) as usize;
        let v = msg.get(i + 4..i + 4 + len)?;
        // IPv4 만(값 8바이트, family 1).
        if len == 8 && v[1] == 0x01 {
            let port = u16::from_be_bytes([v[2], v[3]]);
            let ip = u32::from_be_bytes([v[4], v[5], v[6], v[7]]);
            match kind {
                XOR_MAPPED_ADDRESS => {
                    let ip = Ipv4Addr::from(ip ^ MAGIC);
                    return Some(SocketAddr::new(IpAddr::V4(ip), port ^ (MAGIC >> 16) as u16));
                }
                MAPPED_ADDRESS => {
                    plain = Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port))
                }
                _ => {}
            }
        }
        i += 4 + len.div_ceil(4) * 4;
    }
    plain
}

/// 같은 소켓으로 물어본다(이 소켓의 NAT 매핑이어야 후보로 쓸 수 있다). 3번, 번마다 300ms.
pub fn query(sock: &UdpSocket, server: SocketAddr) -> Option<SocketAddr> {
    let mut txid = [0u8; 12];
    getrandom::fill(&mut txid).ok()?;
    let mut buf = [0u8; 576];
    for _ in 0..3 {
        sock.send_to(&request(txid), server).ok()?;
        let end = Instant::now() + Duration::from_millis(300);
        while let Some(left) = end
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
        {
            sock.set_read_timeout(Some(left)).ok()?;
            match sock.recv_from(&mut buf) {
                Ok((n, from)) if from == server => {
                    if let Some(a) = parse(&buf[..n], txid) {
                        return Some(a);
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(txid: [u8; 12], attr: u16, value: [u8; 8]) -> Vec<u8> {
        let mut m = vec![];
        m.extend_from_slice(&BINDING_SUCCESS.to_be_bytes());
        m.extend_from_slice(&12u16.to_be_bytes());
        m.extend_from_slice(&MAGIC.to_be_bytes());
        m.extend_from_slice(&txid);
        m.extend_from_slice(&attr.to_be_bytes());
        m.extend_from_slice(&8u16.to_be_bytes());
        m.extend_from_slice(&value);
        m
    }

    #[test]
    fn xor_mapped_address() {
        let t = [7; 12];
        // 203.0.113.5:54321 를 XOR 로 숨긴 값.
        let port = 54321u16 ^ 0x2112;
        let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ MAGIC;
        let mut v = [0, 1, 0, 0, 0, 0, 0, 0];
        v[2..4].copy_from_slice(&port.to_be_bytes());
        v[4..8].copy_from_slice(&ip.to_be_bytes());
        assert_eq!(
            parse(&response(t, XOR_MAPPED_ADDRESS, v), t),
            Some("203.0.113.5:54321".parse().unwrap())
        );
        // 다른 거래 번호는 버린다.
        assert_eq!(parse(&response(t, XOR_MAPPED_ADDRESS, v), [8; 12]), None);
    }

    #[test]
    fn plain_mapped_and_garbage() {
        let t = [1; 12];
        let v = [0, 1, 0x1F, 0x90, 192, 0, 2, 9];
        assert_eq!(
            parse(&response(t, MAPPED_ADDRESS, v), t),
            Some("192.0.2.9:8080".parse().unwrap())
        );
        assert_eq!(parse(&[0; 10], t), None);
        let mut cut = response(t, XOR_MAPPED_ADDRESS, v);
        cut.truncate(26);
        assert_eq!(parse(&cut, t), None);
        let r = request(t);
        assert_eq!(&r[0..2], &[0, 1]);
        assert_eq!(&r[8..20], &t);
    }
}
