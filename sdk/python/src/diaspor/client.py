"""HTTP clients for the Diaspor REST API.

This module exposes two parallel classes — :class:`Client` (sync) and
:class:`AsyncClient` (async) — with the same method surface. The sync class
wraps ``httpx.Client``; the async class wraps ``httpx.AsyncClient``. The
shared logic for request building, response parsing, and error mapping lives
on a private base class so the two surfaces never drift.

The clients perform no caching, no retry-on-5xx, and no batching. Those are
opinionated decisions a caller should make in its own pipeline; this SDK
sticks to "one method call → one HTTP request → one typed result".
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Final

import httpx

from ._version import __version__
from .errors import (
    ApiError,
    NotImplementedYetError,
    RateLimitedError,
    VerticalRefusedError,
)
from .models import ScoreRecord

#: Default base URL for the public hosted API. Override via the ``base_url``
#: argument on a client constructor when targeting a staging or on-prem
#: deployment (e.g. ``https://staging-api.diaspor.io`` or a customer's
#: ``api.diaspor.acme.internal``).
DEFAULT_BASE_URL: Final[str] = "https://api.diaspor.io"

#: Default request timeout in seconds. Generous because ``/v1/analyze`` runs
#: synchronously for short clips; callers analyzing long files should switch
#: to the async-job pattern (``analyze`` returns a 202 + analysis_id, then
#: :meth:`Client.poll`).
DEFAULT_TIMEOUT_SECONDS: Final[float] = 30.0

#: Modalities a caller may request on a multi-modality batch endpoint. The
#: server enforces this list as well; we duplicate it here so a typo'd
#: modality fails fast at the SDK boundary instead of round-tripping.
_KNOWN_MODALITIES: Final[frozenset[str]] = frozenset(
    {"pose", "face", "prosody", "credibility", "judge"},
)


def _user_agent() -> str:
    """User-Agent header value. Stamps version + ``httpx`` version for support."""

    return f"diaspor-python/{__version__} httpx/{httpx.__version__}"


def _raise_for_status(response: httpx.Response, *, endpoint: str) -> None:
    """Map an HTTP response to one of the SDK exception classes.

    2xx returns silently. 4xx/5xx raise the most specific :class:`ApiError`
    subclass we can identify; for everything else we raise the generic
    :class:`ApiError` carrying the status code, request id, and parsed body
    (when JSON-decodable).
    """

    if response.is_success:
        return

    request_id = response.headers.get("X-Diaspor-Request-Id")
    body: Any
    try:
        body = response.json()
    except ValueError:
        body = response.text or None

    # Pull a server-supplied error message if the body carries one in the
    # documented ``{"error": {"message": "..."}}`` envelope.
    message = f"API request to {endpoint} failed."
    if isinstance(body, dict):
        err = body.get("error")
        if isinstance(err, dict) and isinstance(err.get("message"), str):
            message = str(err["message"])
        elif isinstance(body.get("message"), str):
            message = str(body["message"])

    status = response.status_code

    if status == 429:
        retry_after_raw = response.headers.get("Retry-After")
        retry_after: float | None = None
        if retry_after_raw is not None:
            try:
                retry_after = float(retry_after_raw)
            except ValueError:
                retry_after = None
        raise RateLimitedError(
            message,
            status_code=status,
            request_id=request_id,
            retry_after_seconds=retry_after,
            body=body,
        )

    if status == 403:
        # The gateway tags vertical refusals with a discriminator in the
        # error body. Anything else 403 (revoked key, network policy) maps
        # to the generic ApiError so callers don't accidentally branch on
        # VerticalRefusedError for unrelated 403s.
        refusal_block: dict[str, Any] = {}
        if isinstance(body, dict) and isinstance(body.get("error"), dict):
            refusal_block = body["error"]
        if refusal_block.get("code") == "vertical_refused":
            vert = refusal_block.get("vertical")
            raise VerticalRefusedError(
                message,
                status_code=status,
                request_id=request_id,
                vertical=vert if isinstance(vert, str) else None,
                endpoint=endpoint,
                body=body,
            )

    if status == 501:
        raise NotImplementedYetError(
            message,
            status_code=status,
            request_id=request_id,
            endpoint=endpoint,
            body=body,
        )

    raise ApiError(
        message,
        status_code=status,
        request_id=request_id,
        body=body,
    )


def _parse_score_record(payload: Any, *, endpoint: str) -> ScoreRecord:
    """Validate a JSON payload into a :class:`ScoreRecord`.

    Wraps ``pydantic.ValidationError`` in an :class:`ApiError` so callers do
    not need to import Pydantic just to handle a wire-schema mismatch.
    """

    if not isinstance(payload, dict):
        raise ApiError(
            f"Expected JSON object from {endpoint}, got {type(payload).__name__}.",
            status_code=200,
            body=payload,
        )
    try:
        return ScoreRecord.model_validate(payload)
    except Exception as exc:  # pragma: no cover - reraised with context
        raise ApiError(
            f"Response from {endpoint} did not match score-v1 schema: {exc}",
            status_code=200,
            body=payload,
        ) from exc


def _validate_modalities(modalities: list[str] | None) -> list[str] | None:
    """Reject unknown modality strings before they hit the wire."""

    if modalities is None:
        return None
    unknown = sorted(set(modalities) - _KNOWN_MODALITIES)
    if unknown:
        raise ValueError(
            f"Unknown modalities: {unknown!r}. Known: {sorted(_KNOWN_MODALITIES)!r}.",
        )
    # Preserve caller ordering for log readability; dedupe to avoid sending
    # the same modality twice.
    seen: set[str] = set()
    deduped: list[str] = []
    for m in modalities:
        if m not in seen:
            seen.add(m)
            deduped.append(m)
    return deduped


class _BaseClient:
    """Shared state for the sync and async client classes.

    Holds the configured ``api_key``, ``base_url``, ``timeout``, and the
    default headers that go on every request. Subclasses provide the
    transport-specific request method.
    """

    def __init__(
        self,
        *,
        api_key: str,
        base_url: str = DEFAULT_BASE_URL,
        timeout: float = DEFAULT_TIMEOUT_SECONDS,
    ) -> None:
        if not api_key:
            raise ValueError("api_key must be a non-empty string.")
        self._api_key: str = api_key
        # Strip the trailing slash so f"{base_url}/v1/..." builds cleanly
        # regardless of how the caller wrote it.
        self._base_url: str = base_url.rstrip("/")
        self._timeout: float = timeout

    @property
    def api_key(self) -> str:
        """The API key this client authenticates with.

        Exposed so a caller can copy auth state into a related client
        (e.g. a sibling :class:`AsyncClient`) without re-reading the
        configuration source.
        """

        return self._api_key

    @property
    def base_url(self) -> str:
        """Base URL the client targets (no trailing slash)."""

        return self._base_url

    @property
    def timeout(self) -> float:
        """Per-request timeout in seconds."""

        return self._timeout

    def _default_headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self._api_key}",
            "User-Agent": _user_agent(),
            "Accept": "application/json",
        }

    def _build_analyze_form(
        self,
        modalities: list[str] | None,
    ) -> dict[str, str]:
        """Body fields for ``POST /v1/analyze`` aside from the file part."""

        validated = _validate_modalities(modalities)
        form: dict[str, str] = {}
        if validated is not None:
            form["modalities"] = ",".join(validated)
        return form


# ---------------------------------------------------------------------------
# Sync client
# ---------------------------------------------------------------------------


class Client(_BaseClient):
    """Synchronous HTTP client for the Diaspor REST API.

    Construct once per process and reuse — the underlying ``httpx.Client``
    pools connections. Call :meth:`close` (or use as a context manager) to
    release the connection pool on shutdown.

    Example::

        with Client(api_key="dk_live_...") as client:
            record = client.analyze("clip.mp4", modalities=["pose", "judge"])
    """

    def __init__(
        self,
        *,
        api_key: str,
        base_url: str = DEFAULT_BASE_URL,
        timeout: float = DEFAULT_TIMEOUT_SECONDS,
    ) -> None:
        super().__init__(api_key=api_key, base_url=base_url, timeout=timeout)
        self._http: httpx.Client = httpx.Client(
            base_url=self._base_url,
            timeout=self._timeout,
            headers=self._default_headers(),
        )

    # --- lifecycle -------------------------------------------------------

    def close(self) -> None:
        """Close the underlying ``httpx.Client`` and release pooled connections."""

        self._http.close()

    def __enter__(self) -> Client:
        return self

    def __exit__(self, *exc_info: object) -> None:
        self.close()

    # --- batch -----------------------------------------------------------

    def analyze(
        self,
        file_path: Path | str,
        modalities: list[str] | None = None,
    ) -> ScoreRecord:
        """Upload a media file to ``POST /v1/analyze`` and return the score record.

        For short clips (under the server's synchronous-job threshold) the
        endpoint returns the score record directly. For longer jobs it
        returns HTTP 202 with an ``analysis_id`` — in that case callers
        should switch to :meth:`poll`. The split is server-side; this
        method simply surfaces whatever the server returns and lets the
        caller decide.

        :param file_path: Local path to the media file to upload.
        :param modalities: Optional subset of ``{pose, face, prosody,
            credibility, judge}``. ``None`` means "use the API key's
            default modality set".
        """

        endpoint = "/v1/analyze"
        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            data = self._build_analyze_form(modalities)
            response = self._http.post(endpoint, files=files, data=data)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    def poll(self, analysis_id: str) -> ScoreRecord:
        """Poll ``GET /v1/analyses/{id}`` for a long-running analysis.

        Returns the final :class:`ScoreRecord` once the job is complete.
        While the job is still running the server responds with HTTP 202
        and the SDK raises :class:`ApiError` carrying status 202 — callers
        should treat that as a "try again later" signal and back off
        accordingly.
        """

        if not analysis_id:
            raise ValueError("analysis_id must be a non-empty string.")
        endpoint = f"/v1/analyses/{analysis_id}"
        response = self._http.get(endpoint)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    # --- per-modality helpers --------------------------------------------

    def pose(self, file_path: Path | str) -> ScoreRecord:
        """Pose-only convenience wrapper around ``POST /v1/pose``.

        Equivalent in result to ``client.analyze(file, modalities=["pose"])``
        but hits the per-modality endpoint, which is cheaper on the
        server side because it skips the multi-modality dispatcher.
        """

        return self._single_modality_call("/v1/pose", file_path)

    def face_mesh(self, file_path: Path | str) -> ScoreRecord:
        """478-landmark face mesh via ``POST /v1/face-mesh``."""

        return self._single_modality_call("/v1/face-mesh", file_path)

    def prosody(self, file_path: Path | str) -> ScoreRecord:
        """Vocal prosody features via ``POST /v1/prosody``."""

        return self._single_modality_call("/v1/prosody", file_path)

    def credibility(self, file_path: Path | str) -> ScoreRecord:
        """Composite credibility indicator via ``POST /v1/credibility``.

        Credibility signals are not lie detection. Calls from API keys
        attested to forensic/hiring/insurance/law_enforcement/eu_workplace/
        eu_education verticals will be refused server-side with
        :class:`VerticalRefusedError`. EU workplace and education contexts
        are blocked under the EU AI Act (effective August 2026); the other
        verticals are policy refusals documented in the Acceptable Use
        Policy. The returned record always discloses the human baseline
        (~0.54) and the peer-reviewed accuracy ceiling (~0.74) alongside
        the score itself.
        """

        return self._single_modality_call("/v1/credibility", file_path)

    def judge(self, file_path: Path | str, *, discipline: str) -> ScoreRecord:
        """Sport-judging score via ``POST /v1/judge?discipline=<discipline>``.

        :param discipline: Discipline identifier (e.g. ``"diving"``,
            ``"weightlifting"``). Discovery of supported disciplines is via
            a separate ``GET /v1/judge/disciplines`` route the SDK does
            not yet expose.
        """

        if not discipline:
            raise ValueError("discipline must be a non-empty string.")
        endpoint = "/v1/judge"
        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            response = self._http.post(
                endpoint,
                files=files,
                params={"discipline": discipline},
            )
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    # --- internal --------------------------------------------------------

    def _single_modality_call(self, endpoint: str, file_path: Path | str) -> ScoreRecord:
        """Upload a file to a single-modality endpoint and parse the response."""

        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            response = self._http.post(endpoint, files=files)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)


# ---------------------------------------------------------------------------
# Async client
# ---------------------------------------------------------------------------


class AsyncClient(_BaseClient):
    """Asynchronous HTTP client for the Diaspor REST API.

    Identical method surface to :class:`Client`; every call is ``await``-able
    rather than blocking. The underlying transport is ``httpx.AsyncClient``.

    Example::

        async with AsyncClient(api_key="dk_live_...") as client:
            record = await client.analyze_async("clip.mp4", modalities=["pose"])

    The async batch method is named :meth:`analyze_async` (mirroring the
    sync :meth:`Client.analyze`) so that callers that hold both clients on
    one type do not collide on attribute names. The per-modality helpers
    keep the same names as the sync class because they are only ever
    reached via an explicit ``await``.
    """

    def __init__(
        self,
        *,
        api_key: str,
        base_url: str = DEFAULT_BASE_URL,
        timeout: float = DEFAULT_TIMEOUT_SECONDS,
    ) -> None:
        super().__init__(api_key=api_key, base_url=base_url, timeout=timeout)
        self._http: httpx.AsyncClient = httpx.AsyncClient(
            base_url=self._base_url,
            timeout=self._timeout,
            headers=self._default_headers(),
        )

    # --- lifecycle -------------------------------------------------------

    async def aclose(self) -> None:
        """Close the underlying ``httpx.AsyncClient``."""

        await self._http.aclose()

    async def __aenter__(self) -> AsyncClient:
        return self

    async def __aexit__(self, *exc_info: object) -> None:
        await self.aclose()

    # --- batch -----------------------------------------------------------

    async def analyze_async(
        self,
        file_path: Path | str,
        modalities: list[str] | None = None,
    ) -> ScoreRecord:
        """Async equivalent of :meth:`Client.analyze`."""

        endpoint = "/v1/analyze"
        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            data = self._build_analyze_form(modalities)
            response = await self._http.post(endpoint, files=files, data=data)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    async def poll(self, analysis_id: str) -> ScoreRecord:
        """Async equivalent of :meth:`Client.poll`."""

        if not analysis_id:
            raise ValueError("analysis_id must be a non-empty string.")
        endpoint = f"/v1/analyses/{analysis_id}"
        response = await self._http.get(endpoint)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    # --- per-modality helpers --------------------------------------------

    async def pose(self, file_path: Path | str) -> ScoreRecord:
        """Pose-only convenience wrapper around ``POST /v1/pose``."""

        return await self._single_modality_call("/v1/pose", file_path)

    async def face_mesh(self, file_path: Path | str) -> ScoreRecord:
        """478-landmark face mesh via ``POST /v1/face-mesh``."""

        return await self._single_modality_call("/v1/face-mesh", file_path)

    async def prosody(self, file_path: Path | str) -> ScoreRecord:
        """Vocal prosody features via ``POST /v1/prosody``."""

        return await self._single_modality_call("/v1/prosody", file_path)

    async def credibility(self, file_path: Path | str) -> ScoreRecord:
        """Composite credibility indicator via ``POST /v1/credibility``.

        Credibility signals are not lie detection. Calls from API keys
        attested to forensic/hiring/insurance/law_enforcement/eu_workplace/
        eu_education verticals will be refused server-side with
        :class:`VerticalRefusedError`. EU workplace and education contexts
        are blocked under the EU AI Act (effective August 2026); the other
        verticals are policy refusals documented in the Acceptable Use
        Policy. The returned record always discloses the human baseline
        (~0.54) and the peer-reviewed accuracy ceiling (~0.74) alongside
        the score itself.
        """

        return await self._single_modality_call("/v1/credibility", file_path)

    async def judge(self, file_path: Path | str, *, discipline: str) -> ScoreRecord:
        """Sport-judging score via ``POST /v1/judge?discipline=<discipline>``."""

        if not discipline:
            raise ValueError("discipline must be a non-empty string.")
        endpoint = "/v1/judge"
        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            response = await self._http.post(
                endpoint,
                files=files,
                params={"discipline": discipline},
            )
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    # --- internal --------------------------------------------------------

    async def _single_modality_call(
        self,
        endpoint: str,
        file_path: Path | str,
    ) -> ScoreRecord:
        path = Path(file_path)
        with path.open("rb") as fh:
            files = {"file": (path.name, fh, "application/octet-stream")}
            response = await self._http.post(endpoint, files=files)
        _raise_for_status(response, endpoint=endpoint)
        return _parse_score_record(response.json(), endpoint=endpoint)

    # Exposed for the streaming module so it can build a WSS URL against
    # the same base + auth state as the REST client.
    def _build_ws_url(self, path: str, params: dict[str, str]) -> tuple[str, dict[str, str]]:
        """Return the WSS URL and the auth headers to attach to the handshake.

        The streaming layer authenticates via a short-lived token in the
        ``token`` query parameter (so reverse proxies that strip
        ``Authorization`` headers on Upgrade don't break ingest). The SDK
        passes the API key through ``token`` and also re-sends it as
        ``Authorization`` on the WSS handshake in case the deployment
        supports both.
        """

        if self._base_url.startswith("https://"):
            ws_base = "wss://" + self._base_url[len("https://") :]
        elif self._base_url.startswith("http://"):
            ws_base = "ws://" + self._base_url[len("http://") :]
        else:  # pragma: no cover - defensive
            ws_base = self._base_url

        merged = {"token": self._api_key, **params}
        query = "&".join(f"{k}={v}" for k, v in merged.items())
        url = f"{ws_base}{path}?{query}"
        return url, {"Authorization": f"Bearer {self._api_key}"}


__all__ = [
    "DEFAULT_BASE_URL",
    "DEFAULT_TIMEOUT_SECONDS",
    "AsyncClient",
    "Client",
]
