# diaspor — Python SDK

Official Python client for the [Diaspor](https://diaspor.io) non-verbal video AI
API: batch + live pose estimation, facial landmarks, vocal prosody, credibility
indicators, and per-discipline sport judging.

```bash
pip install diaspor
```

Requires Python 3.10+.

## What this is

A thin HTTP and WebSocket client that wraps `api.diaspor.io`. The package
contains no models, no inference code, and makes no decisions of its own — it
serializes your request, calls the API, and returns Pydantic-typed responses
that match the public [score-v1 schema](https://github.com/stonyp90/diaspor/blob/main/docs/schema/score-v1.json).

## Why Apache-2.0 (and not AGPL-3.0)

The **heavy self-hosted core** of Diaspor — the Rust workspace at
[github.com/stonyp90/diaspor](https://github.com/stonyp90/diaspor) that does
the actual frame decoding, model inference, and adapter training — is
dual-licensed **AGPL-3.0-or-later or commercial**. AGPL is the right license
for that code: the network-use clause keeps SaaS forks honest.

This **client SDK** is licensed **Apache-2.0** on purpose. It is meant to be
embedded inside closed-source customer applications (mobile apps, internal
tooling, judging software at federation events, video pipelines that ship to
broadcast partners, etc.) without forcing those applications to also adopt
AGPL. Apache-2.0 gives downstream embedders the patent grant and trademark
clarity they need to ship with confidence; AGPL would make this SDK unusable
for that audience and defeat the point of having an SDK at all.

The split is intentional: the data plane is AGPL, the wire client is
Apache-2.0. See the [LICENSE](./LICENSE) file for the full text.

## Quickstart

### Batch analyze a file

```python
from diaspor import Client

client = Client(api_key="dk_live_...")

record = client.analyze(
    "diving-attempt-03.mp4",
    modalities=["pose", "judge"],
)

print(record.modalities.judge.score)
print(record.modalities.pose.keypoints[0].visibility)
```

The returned `ScoreRecord` is a Pydantic v2 model that mirrors
`docs/schema/score-v1.json` field-for-field. It validates the response shape
at parse time, so a wire schema drift surfaces as a `pydantic.ValidationError`
rather than a silent attribute error deep in your pipeline.

### Stream a live WHIP push

```python
import asyncio
from diaspor import AsyncClient, LiveSession

async def main() -> None:
    client = AsyncClient(api_key="dk_live_...")

    async with LiveSession.whip(client=client) as session:
        # Connect your WHIP publisher to session.ingest_url with session.ingest_token.
        print(f"Publish to {session.ingest_url}")

        async for event in session:
            # Threshold-crossing score events arrive as they happen.
            print(event.t_start_ms, event.modalities.pose.keypoints[0])

asyncio.run(main())
```

### Drop a bot into a Zoom / Meet / Teams meeting

```python
import asyncio
from diaspor import AsyncClient, LiveSession

async def main() -> None:
    client = AsyncClient(api_key="dk_live_...")

    async with LiveSession.meeting_bot(
        client=client,
        provider="recall_ai",
        meeting_url="https://zoom.us/j/123456789",
        consent_script=(
            "Hi everyone — this meeting is being analyzed by an AI tool that "
            "produces non-verbal signal scores. By staying in the call you "
            "consent to that analysis. Object now to be excluded."
        ),
    ) as session:
        async for event in session:
            print(event.t_start_ms, event.modalities.prosody)

asyncio.run(main())
```

All meeting-bot sessions require a `consent_script`. The bot reads it in the
call before any frames are scored — that's an all-party-consent requirement
the API enforces server-side as well, but stating it here keeps the SDK
contract honest.

## Compliance note on credibility signals

The `/v1/credibility` endpoint returns a stress + incongruence indicator. It
is **not** a lie detector and the SDK is wired to refuse to pretend
otherwise. Calls from API keys whose vertical attestation is `forensic`,
`hiring`, `insurance`, `law_enforcement`, `eu_workplace`, or `eu_education`
are refused server-side and surface here as `VerticalRefusedError`. EU
workplace and EU education contexts are additionally blocked under the EU AI
Act (effective August 2026).

See [docs/POSITIONING.md](https://github.com/stonyp90/diaspor/blob/main/docs/POSITIONING.md)
in the main repo for the longer rationale.

## Versioning

This SDK ships independently from the core Rust workspace. The version on
PyPI tracks the wire-API version it targets:

- `0.x` series: pre-GA, tracks the `/v1` API as it stabilizes.
- `1.0`: tagged when `/v1` reaches GA and the score-v1 schema is frozen.

## Links

- [API docs](https://developers.diaspor.io)
- [Core repo (Rust, AGPL-3.0 / commercial)](https://github.com/stonyp90/diaspor)
- [Score record schema](https://github.com/stonyp90/diaspor/blob/main/docs/schema/score-v1.json)
- [Issue tracker](https://github.com/stonyp90/diaspor/issues)
