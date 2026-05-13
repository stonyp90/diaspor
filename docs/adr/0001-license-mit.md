# ADR 0001: License under MIT rather than Apache-2.0 or AGPL-3.0

## Status

Accepted — 2026-02-04. Confirmed at v0.1.0-alpha tag.

## Context

`stony-vdfs` is a library aimed at downstream Rust applications, some of which will be
commercial closed-source desktop products and some of which will be publicly funded
open-source projects in EU jurisdictions. The license needs to maximise adoption
without alienating either audience. The three serious candidates were:

- **MIT** — permissive, three short paragraphs, near-universal compatibility.
- **Apache-2.0** — permissive, but adds explicit patent grants and NOTICE requirements.
- **AGPL-3.0** — copyleft, strong reciprocity, network-use trigger.

NLnet NGI Zero Commons Fund recipients are not required to pick any specific license,
only to ship under an OSI-approved open-source license.

## Decision

We license the workspace under **MIT**.

## Consequences

Positive: MIT is the lowest-friction license for both commercial integrators and the
Rust ecosystem at large (most of the Rust standard library ecosystem is dual MIT /
Apache-2.0). Downstream adopters can link `stony-vdfs` into closed products without
license-compatibility lawyering; that is a precondition for the library being used by
the kind of privacy-respecting commercial desktop apps we want to enable.

Negative: MIT lacks Apache-2.0's explicit patent grant and lacks AGPL's reciprocity. The
project accepts the patent-grant gap because individual contributor risk is low for a
filesystem library, and accepts the lack of reciprocity because forcing source release
on every downstream user would defeat the adoption goal. Contributors who want stronger
copyleft can build AGPL-licensed forks; the MIT grant permits that.
