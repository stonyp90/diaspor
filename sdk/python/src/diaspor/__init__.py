"""Diaspor — official Python SDK for the non-verbal video AI API.

This package wraps ``api.diaspor.io`` with typed Pydantic v2 models and
parallel sync (:class:`Client`) and async (:class:`AsyncClient`) HTTP
clients, plus a :class:`LiveSession` async context manager for WHIP push
and meeting-bot ingest.

The SDK is licensed Apache-2.0 so it can be embedded inside closed-source
customer applications without forcing AGPL. The heavy self-hosted core
(github.com/stonyp90/diaspor) is dual-licensed AGPL-3.0 or commercial.

See the README for quickstart snippets, or
https://developers.diaspor.io for the full API reference.
"""

from __future__ import annotations

from ._version import __version__
from .client import (
    DEFAULT_BASE_URL,
    DEFAULT_TIMEOUT_SECONDS,
    AsyncClient,
    Client,
)
from .errors import (
    ApiError,
    DiasporError,
    NotImplementedYetError,
    RateLimitedError,
    VerticalRefusedError,
)
from .models import (
    CredibilityModality,
    FaceModality,
    GazeDirection,
    InferenceRuntime,
    IngestEvent,
    JudgeModality,
    Keypoint3d,
    Modalities,
    ModelProvenance,
    PoseModality,
    ProsodyModality,
    RecordKind,
    ScoreRecord,
    Severity,
    VerticalAttestation,
)
from .streaming import BotProvider, LiveSession, MeetingPlatform

__all__ = [
    # Clients
    "AsyncClient",
    "Client",
    "DEFAULT_BASE_URL",
    "DEFAULT_TIMEOUT_SECONDS",
    # Streaming
    "BotProvider",
    "LiveSession",
    "MeetingPlatform",
    # Errors
    "ApiError",
    "DiasporError",
    "NotImplementedYetError",
    "RateLimitedError",
    "VerticalRefusedError",
    # Models — top-level record
    "ScoreRecord",
    "Modalities",
    "RecordKind",
    "ModelProvenance",
    "IngestEvent",
    # Models — per-modality
    "CredibilityModality",
    "FaceModality",
    "GazeDirection",
    "JudgeModality",
    "PoseModality",
    "ProsodyModality",
    # Models — primitives + enums
    "InferenceRuntime",
    "Keypoint3d",
    "Severity",
    "VerticalAttestation",
    # Version
    "__version__",
]
