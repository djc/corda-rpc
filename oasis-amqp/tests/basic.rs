use std::collections::HashMap;

use bytes::BytesMut;
use serde_bytes::{ByteBuf, Bytes};
use tokio_util::codec::Decoder;

use oasis_amqp::proto::{Codec, Frame, Protocol};
use oasis_amqp::{amqp, sasl};

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

#[test]
fn setup() {
    let open = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Open(amqp::Open {
            container_id: "source",
            ..Default::default()
        }),
        message: None,
    });
    assert_eq!(open.to_vec().unwrap(), Vec::from(
        &b"\x00\x00\x00$\x02\x00\x00\x00\x00S\x10\xd0\x00\x00\x00\x14\x00\x00\x00\t\xa1\x06source@@@@@@@@"[..]
    ));

    let mut codec = Codec {};
    let mut server = BytesMut::new();
    server.extend_from_slice(
        &b"\x00\x00\x00\xa8\x02\x00\x00\x00\x00S\x10\xc0\x9b\n\xa1\x03foo@p\x00\x02\x00\x00`\xff\xffp\x00\x00u0@@\xe0M\x04\xa3\x1dsole-connection-for-container\x10DELAYED_DELIVERY\x0bSHARED-SUBS\x0fANONYMOUS-RELAY@\xc13\x04\xa3\x07product\xa1\x17apache-activemq-artemis\xa3\x07version\xa1\x052.6.2"[..]
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Open(amqp::Open {
                container_id: "foo",
                max_frame_size: Some(131_072),
                channel_max: Some(65_535),
                idle_timeout: Some(30_000),
                offered_capabilities: Some(vec![
                    "sole-connection-for-container",
                    "DELAYED_DELIVERY",
                    "SHARED-SUBS",
                    "ANONYMOUS-RELAY"
                ]),
                ..Default::default()
            }),
            message: None,
        })
    );

    let begin = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Begin(amqp::Begin {
            remote_channel: None,
            next_outgoing_id: 1,
            incoming_window: 8,
            outgoing_window: 8,
            ..Default::default()
        }),
        message: None,
    });
    assert_eq!(begin.to_vec().unwrap(), Vec::from(
        &b"\x00\x00\x00\x1f\x02\x00\x00\x00\x00S\x11\xd0\x00\x00\x00\x0f\x00\x00\x00\x08@R\x01R\x08R\x08@@@@"[..]
    ));

    let mut server = BytesMut::new();
    server.extend_from_slice(
        &b"\x00\x00\x00\"\x02\x00\x00\x00\x00S\x11\xc0\x15\x05`\x00\x00R\x01p\x7f\xff\xff\xffp\x7f\xff\xff\xffp\x00\x00\xff\xff"[..]
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Begin(amqp::Begin {
                remote_channel: Some(0),
                next_outgoing_id: 1,
                incoming_window: 2_147_483_647,
                outgoing_window: 2_147_483_647,
                handle_max: Some(65_535),
                ..Default::default()
            }),
            message: None,
        })
    );

    let attach = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Attach(amqp::Attach {
            name: "my-foo-sender".into(),
            handle: 0,
            role: amqp::Role::Sender,
            snd_settle_mode: None,
            rcv_settle_mode: None,
            source: Some(amqp::Source {
                address: Some("source".into()),
                ..Default::default()
            }),
            target: Some(amqp::Target {
                address: Some("target-bar".into()),
                ..Default::default()
            }),
            unsettled: None,
            incomplete_unsettled: None,
            initial_delivery_count: Some(0),
            max_message_size: None,
            offered_capabilities: None,
            desired_capabilities: None,
            properties: None,
        }),
        message: None,
    });
    assert_eq!(attach.to_vec().unwrap(), Vec::from(
        &b"\x00\x00\x00j\x02\x00\x00\x00\x00S\x12\xd0\x00\x00\x00Z\x00\x00\x00\x0e\xa1\rmy-foo-senderCB@@\x00S(\xd0\x00\x00\x00\x16\x00\x00\x00\x0b\xa1\x06source@@@@@@@@@@\x00S)\xd0\x00\x00\x00\x16\x00\x00\x00\x07\xa1\ntarget-bar@@@@@@@@C@@@@"[..]
    ));

    let mut server = BytesMut::new();
    server.extend_from_slice(
        &b"\x00\x00\x00C\x02\x00\x00\x00\x00S\x12\xc06\x07\xa1\rmy-foo-senderCAP\x02P\x00\x00S(\xc0\t\x01\xa1\x06source\x00S)\xc0\r\x01\xa1\ntarget-bar"[..]
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Attach(amqp::Attach {
                name: "my-foo-sender".into(),
                handle: 0,
                role: amqp::Role::Receiver,
                snd_settle_mode: Some(amqp::SenderSettleMode::Mixed),
                rcv_settle_mode: Some(amqp::ReceiverSettleMode::First),
                source: Some(amqp::Source {
                    address: Some("source".into()),
                    ..Default::default()
                }),
                target: Some(amqp::Target {
                    address: Some("target-bar".into()),
                    ..Default::default()
                }),
                unsettled: None,
                incomplete_unsettled: None,
                initial_delivery_count: None,
                max_message_size: None,
                offered_capabilities: None,
                desired_capabilities: None,
                properties: None,
            }),
            message: None,
        })
    );

    let mut server = BytesMut::new();
    server.extend_from_slice(
        &b"\x00\x00\x00#\x02\x00\x00\x00\x00S\x13\xc0\x16\x07R\x01p\x7f\xff\xff\xffR\x01p\x7f\xff\xff\xffCCp\x00\x00\x03\xe8"[..]
    );
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Flow(amqp::Flow {
                next_incoming_id: Some(1),
                incoming_window: 2_147_483_647,
                next_outgoing_id: 1,
                outgoing_window: 2_147_483_647,
                handle: Some(0),
                delivery_count: Some(0),
                link_credit: Some(1_000),
                available: None,
                drain: None,
                echo: None,
                properties: None,
            }),
            message: None,
        })
    );
}

#[test]
fn transfer() {
    // keeping this empty because HashMap RandomState makes the serialized output indeterministic
    let properties = HashMap::new();
    let (message_id, delivery_tag) = ("foo", "bar");
    let body = &b"baz"[..];
    let transfer = Frame::Amqp(amqp::Frame {
        channel: 0,
        extended_header: None,
        performative: amqp::Performative::Transfer(amqp::Transfer {
            handle: 0,
            delivery_id: Some(0),
            delivery_tag: Some(ByteBuf::from(delivery_tag.as_bytes().to_vec())),
            message_format: Some(0),
            ..Default::default()
        }),
        message: Some(amqp::Message {
            properties: Some(amqp::Properties {
                message_id: Some(message_id.into()),
                reply_to: Some("sender".into()),
                user_id: Some(Bytes::new(b"user1")),
                ..Default::default()
            }),
            application_properties: Some(amqp::ApplicationProperties(properties)),
            body: Some(amqp::Body::Data(amqp::Data(&body))),
            ..Default::default()
        }),
    });
    assert_eq!(transfer.to_vec().unwrap(), Vec::from(
        &b"\x00\x00\x00a\x02\x00\x00\x00\x00S\x14\xd0\x00\x00\x00\x13\x00\x00\x00\x0bCC\xa0\x03barC@@@@@@@\x00Ss\xd0\x00\x00\x00\"\x00\x00\x00\r\xa1\x03foo\xa0\x05user1@@\xa1\x06sender@@@@@@@@\x00St\xd1\x00\x00\x00\x04\x00\x00\x00\x00\x00Su\xa0\x03baz"[..]
    ));

    let mut codec = Codec {};
    let mut server = BytesMut::new();
    server
        .extend_from_slice(&b"\x00\x00\x00\x16\x02\x00\x00\x00\x00S\x15\xc0\t\x05ACCA\x00S$E"[..]);
    let wrapped = codec.decode(&mut server).unwrap().unwrap();
    assert_eq!(
        wrapped.frame,
        Frame::Amqp(amqp::Frame {
            channel: 0,
            extended_header: None,
            performative: amqp::Performative::Disposition(amqp::Disposition {
                role: amqp::Role::Receiver,
                first: 0,
                last: Some(0),
                settled: Some(true),
                state: Some(amqp::DeliveryState::Accepted(amqp::Accepted {})),
                batchable: None,
            }),
            message: None,
        })
    );
}
