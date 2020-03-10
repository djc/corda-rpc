use bytes::BytesMut;
use serde_bytes::Bytes;
use tokio_util::codec::Decoder;

use oasis_amqp::sasl;
use oasis_amqp::{Codec, Frame, Protocol};

#[test]
fn login() {
    let client_header = Frame::Header(Protocol::Sasl);
    assert_eq!(&*client_header.to_vec().unwrap(), b"AMQP\x03\x01\x00\x00");

    let mut codec = Codec {};
    let mut server = BytesMut::new();
    server.extend_from_slice(
        b"AMQP\x03\x01\x00\x00\x00\x00\x00\"\x02\x01\x00\x00\x00S@\xc0\x15\x01\xe0\x12\x02\xa3\x05PLAIN\tANONYMOUS"
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(wrapped.frame, Frame::Header(Protocol::Sasl));
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Sasl(sasl::Frame::Mechanisms(sasl::Mechanisms {
            sasl_server_mechanisms: vec![sasl::Mechanism::Plain, sasl::Mechanism::Anonymous],
        }))
    );

    let bytes = Frame::Sasl(sasl::Frame::Init(sasl::Init {
        mechanism: sasl::Mechanism::Plain,
        initial_response: Some(Bytes::new(b"\x00user1\x00psswd")),
        hostname: None,
    }))
    .to_vec()
    .unwrap();
    assert_eq!(
        &bytes[4..],
        &b"\x02\x01\x00\x00\x00SA\xd0\x00\x00\x00\x1a\x00\x00\x00\x03\xa3\x05PLAIN\xa0\x0c\x00user1\x00psswd@"[..],
    );

    let mut server = BytesMut::new();
    server.extend_from_slice(
        b"\x00\x00\x00\x10\x02\x01\x00\x00\x00SD\xc0\x03\x01P\x00AMQP\x00\x01\x00\x00",
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Sasl(sasl::Frame::Outcome(sasl::Outcome {
            code: sasl::Code::Ok,
            additional_data: None,
        }))
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(wrapped.frame, Frame::Header(Protocol::Amqp));
}
