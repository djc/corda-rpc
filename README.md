# corda-rpc: Rust libraries for doing Corda RPC

[![Build status](https://github.com/djc/corda-rpc/workflows/CI/badge.svg)](https://github.com/djc/corda-rpc/actions?query=workflow%3ACI)
[![Coverage status](https://codecov.io/gh/djc/corda-rpc/branch/master/graph/badge.svg)](https://codecov.io/gh/djc/corda-rpc)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

[Corda](https://www.corda.net/) is an open source blockchain platform. To communicate with it,
your code needs to talk [AMQP 1.0](https://www.amqp.org/) with the broker run by the Corda node.
This code runs on stable Rust, and uses async/await using the Tokio runtime for networking.

This project was written for [@ing-bank](https://github.com/ing-bank/), working on the ValueX
project to create a digital securities distribution platform for institutional investors. The
repository contains three crates:

* corda-rpc: Corda-specific code to interact with the RPC mechanism
* oasis-amqp: a generic library that implements (most of) the AMQP 1.0 protocol
* oasis-amqp-macros: some procedural macros to support the oasis-amqp code

Current state of the project: unfortunately this is not fully functional yet. The example
program in `corda-rpc/examples` will run and trigger the RPC on the Corda node, but I
have not yet been able to receive and process the call's result. This code is provided
as-is -- I am happy to answer questions and review pull requests. My current project at
work still needs this, but working on this is currently not a high priority.
