# corda-rpc: Rust libraries for doing Corda RPC

[![Build status](https://github.com/djc/corda-rpc/workflows/CI/badge.svg)](https://github.com/djc/corda-rpc/actions?query=workflow%3ACI)
[![Coverage status](https://codecov.io/gh/djc/corda-rpc/branch/master/graph/badge.svg)](https://codecov.io/gh/djc/corda-rpc)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

[Corda](https://www.corda.net/) is an open source blockchain platform. To work with a Corda node,
your code needs to communicate over [AMQP 1.0](https://www.amqp.org/) with the broker run by the node.
This project targets stable Rust and uses asynchronous network I/O built on Tokio. This might be
useful for code running in constrained environments (IoT) or interfacing with other native code.

This project was written within [ING Bank](https://github.com/ing-bank/), while working on the
ValueX project to create a digital securities distribution platform for institutional investors
The provided functionality is separated into three crates, as explained below.

The **current state of the project can be described as pre-alpha**. So far I have worked to get a simple
RPC call to the Corda node to work, and everything provided is only complete insofar as needed for
that purpose. The example code in [network-map-snapshot](corda-rpc/examples/network-map-snapshot.rs)
will trigger an RPC call on the Corda node as desired and return the proper response. However, this
only works against a Corda node which has [some changes](https://github.com/corda/corda/compare/release/os/4.5...djc:amqp-rpc) applied to it.

I am happy to answer questions about the code and it's current state and discuss a future roadmap,
and I intend to provide (passive) maintenance (like PR code reviews) of this code going forward.

## corda-rpc: abstractions specific to Corda RPC

[![Documentation](https://docs.rs/corda-rpc/badge.svg)](https://docs.rs/corda-rpc/)
[![Crates.io](https://img.shields.io/crates/v/corda-rpc.svg)](https://crates.io/crates/corda-rpc)

While the Corda RPC protocol builds on top of the AMQP 1.0 standard, it defines its own encoding
schema serialization to protocol to make protocol messages self-describing. This crate contains
implementations of the required serialization and deserialization primitives, and will contain
other code specific to Corda going forward. The ideal end goal would be an implementation matching
Corda's [CordaRPCOps](https://docs.corda.net/api/kotlin/corda/net.corda.core.messaging/-corda-r-p-c-ops/index.html) interface.

## oasis-amqp: generic implementation of the AMQP 1.0 protocol

[![Documentation](https://docs.rs/oasis-amqp/badge.svg)](https://docs.rs/oasis-amqp/)
[![Crates.io](https://img.shields.io/crates/v/oasis-amqp.svg)](https://crates.io/crates/oasis-amqp)

The name "AMQP" is often used to refer to [version 0.9.1](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf)
of the protocol, as implemented by RabbitMQ and many other software components. Despite the shared name,
AMQP 1.0 as standardized by OASIS deviates substantially from the 0.9.1 protocol. This crate aims to provide
a generally usable (not specific to Corda) implementation of a protocol client.

As mentioned above, the library currently falls short of that goal. While it provides a robust version of
the parts of the protocol that are strictly needed to start exchanging messages with a broker, many parts
are missing or incomplete. Nevertheless, the building blocks provided (in particular, the serialization
and deserialization based on Rust's powerful serde framework) should in many cases make it straightforward
to fill in the missing bits.

## oasis-amqp-derive: helper macro(s)

[![Documentation](https://docs.rs/oasis-amqp-macros/badge.svg)](https://docs.rs/oasis-amqp-macros/)
[![Crates.io](https://img.shields.io/crates/v/oasis-amqp-macros.svg)](https://crates.io/crates/oasis-amqp-macros)

The implementation of the oasis-amqp crate is supported by a single procedural macro which derives
required implementations of `serde::Deserialize` and `oasis_amqp::Described` for any type definitions.
