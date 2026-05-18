"""WebSocket-based live ingest sessions.

Two ingest shapes are supported, both via the same
``wss://api.diaspor.io/v1/stream`` endpoint distinguished by query string:

* **WHIP push** (``?ingest=whip``) — a WebRTC publisher (browser, OBS,
  GStreamer, a hardware encoder) pushes media into the server's SFU
  sidecar; the server analyzes the frames and emits score events back
  over this WebSocket.
* **Meeting-bot** (``?bot=meeting&platform=<...>&meeting_url=<...>``) — the
  server dispatches a Recall.ai bot into a Zoom/Meet/Teams meeting, which
  delivers raw audio+video frames into the same analysis pipeline.

Both shapes deliver :class:`~diaspor.models.IngestEvent` records over the
WebSocket, so callers iterate the session identically once it is open.
"""

from __future__ import annotations

import json
from types import TracebackType
from typing import TYPE_CHECKING, Final, Literal

from websockets import ConnectionClosedOK
from websockets.asyncio.client import ClientConnection, connect

from .errors import ApiError
from .models import IngestEvent

if TYPE_CHECKING:
    from .client import AsyncClient

#: Supported meeting-bot providers. Only Recall.ai is wired today; other
#: providers (Read.ai, Zoom's first-party bots API, etc.) get added here as
#: they are integrated server-side.
BotProvider = Literal["recall_ai"]

#: Supported meeting platforms via Recall.ai's unified API.
MeetingPlatform = Literal["zoom", "google_meet", "teams"]

_DEFAULT_PLATFORM: Final[MeetingPlatform] = "zoom"


class LiveSession:
    """Async context manager for a live ingest session over WebSocket.

    Construct via the :meth:`whip` or :meth:`meeting_bot` factory methods —
    direct instantiation is not part of the public API because the
    constructor arguments depend on the ingest shape and lifecycle.

    Once entered, the session yields :class:`IngestEvent` records as the
    server emits them. Iteration ends cleanly when the server closes the
    connection (end of stream); unexpected closures raise
    :class:`~diaspor.errors.ApiError`.

    For ``whip`` sessions, ``ingest_url`` and ``ingest_token`` are
    populated on enter — wire those into your WHIP publisher to begin
    pushing media. For ``meeting_bot`` sessions the bot is dispatched by
    the server on enter; the WebSocket starts emitting events as soon as
    the bot is admitted to the meeting.
    """

    def __init__(
        self,
        *,
        client: AsyncClient,
        path: str,
        params: dict[str, str],
    ) -> None:
        # Private constructor — callers should use the factory methods.
        self._client: AsyncClient = client
        self._path: str = path
        self._params: dict[str, str] = dict(params)
        self._ws: ClientConnection | None = None

        #: For WHIP sessions: the URL the publisher should connect to.
        #: Populated by the server's first protocol message after the
        #: WebSocket is established. ``None`` for meeting-bot sessions.
        self.ingest_url: str | None = None

        #: For WHIP sessions: the token the publisher should pass through.
        #: ``None`` for meeting-bot sessions.
        self.ingest_token: str | None = None

        #: Session identifier assigned by the server, available after the
        #: WebSocket has been entered. Stable for the lifetime of the
        #: session — log this alongside customer events for support.
        self.session_id: str | None = None

    # --- factories -------------------------------------------------------

    @classmethod
    def whip(
        cls,
        *,
        client: AsyncClient,
        stream_id: str | None = None,
    ) -> LiveSession:
        """Build a WHIP-ingest live session.

        :param client: An :class:`~diaspor.client.AsyncClient` that holds
            the auth state for the session.
        :param stream_id: Optional caller-provided stream identifier. If
            omitted, the server allocates one on connect.
        """

        params: dict[str, str] = {"ingest": "whip"}
        if stream_id is not None:
            params["stream_id"] = stream_id
        return cls(client=client, path="/v1/stream", params=params)

    @classmethod
    def meeting_bot(
        cls,
        *,
        client: AsyncClient,
        meeting_url: str,
        consent_script: str,
        provider: BotProvider = "recall_ai",
        platform: MeetingPlatform = _DEFAULT_PLATFORM,
    ) -> LiveSession:
        """Build a meeting-bot live session (Zoom / Google Meet / Teams).

        :param client: Authenticated :class:`AsyncClient`.
        :param meeting_url: Full meeting URL the bot should join.
        :param consent_script: Text the bot reads to attendees on join
            before any frames are scored. **Required** — the API enforces
            all-party consent server-side and refuses sessions without
            this field. EU workplace and EU education contexts are
            blocked under the EU AI Act regardless of consent.
        :param provider: Meeting-bot provider. Currently only
            ``"recall_ai"`` is supported.
        :param platform: Target platform. Recall.ai unifies Zoom, Google
            Meet, and Microsoft Teams behind a single API; the platform
            tag here is metadata for routing and audit.
        """

        if not meeting_url:
            raise ValueError("meeting_url must be a non-empty string.")
        if not consent_script:
            raise ValueError(
                "consent_script must be a non-empty string. All-party consent "
                "is required server-side; the bot reads this script before "
                "any frames are scored.",
            )
        params: dict[str, str] = {
            "bot": "meeting",
            "provider": provider,
            "platform": platform,
            "meeting_url": meeting_url,
            "consent_script": consent_script,
        }
        return cls(client=client, path="/v1/stream", params=params)

    # --- async context manager ------------------------------------------

    async def __aenter__(self) -> LiveSession:
        url, headers = self._client._build_ws_url(self._path, self._params)
        # ``connect`` returns an async iterator of inbound messages; the
        # connection itself is the value yielded into ``async with``.
        self._ws = await connect(url, additional_headers=headers)

        # The server's first message is a control frame describing the
        # session. WHIP sessions get ingest_url/ingest_token; meeting-bot
        # sessions just get a session_id and an admission status. The SDK
        # surfaces both on ``self``.
        hello_raw = await self._ws.recv()
        try:
            hello = json.loads(hello_raw)
        except (ValueError, TypeError) as exc:
            raise ApiError(
                f"Live session opened but server hello frame was not JSON: {exc}",
                status_code=0,
                body=hello_raw,
            ) from exc

        if not isinstance(hello, dict) or hello.get("type") != "session.hello":
            raise ApiError(
                "Live session opened but first frame was not a 'session.hello'.",
                status_code=0,
                body=hello,
            )

        sid = hello.get("session_id")
        self.session_id = sid if isinstance(sid, str) else None

        ingest = hello.get("ingest")
        if isinstance(ingest, dict):
            url_val = ingest.get("url")
            tok_val = ingest.get("token")
            self.ingest_url = url_val if isinstance(url_val, str) else None
            self.ingest_token = tok_val if isinstance(tok_val, str) else None

        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        if self._ws is not None:
            await self._ws.close()
            self._ws = None

    # --- iteration -------------------------------------------------------

    def __aiter__(self) -> LiveSession:
        return self

    async def __anext__(self) -> IngestEvent:
        if self._ws is None:
            raise RuntimeError(
                "LiveSession used outside its 'async with' block. "
                "Wrap iteration in `async with LiveSession.whip(...) as session:`.",
            )
        try:
            raw = await self._ws.recv()
        except ConnectionClosedOK as exc:
            # Normal end-of-stream close from the server.
            raise StopAsyncIteration from exc

        try:
            payload = json.loads(raw)
        except (ValueError, TypeError) as exc:
            raise ApiError(
                f"Received non-JSON frame on live session: {exc}",
                status_code=0,
                body=raw,
            ) from exc

        if not isinstance(payload, dict):
            raise ApiError(
                "Received non-object frame on live session.",
                status_code=0,
                body=payload,
            )

        # Skip server-side heartbeats; they are an implementation detail.
        if payload.get("type") == "session.ping":
            return await self.__anext__()

        if payload.get("type") != "score.event":
            raise ApiError(
                f"Unexpected frame type on live session: {payload.get('type')!r}.",
                status_code=0,
                body=payload,
            )

        try:
            return IngestEvent.model_validate(payload.get("data"))
        except Exception as exc:  # pragma: no cover - re-raised with context
            raise ApiError(
                f"Live event payload did not validate as IngestEvent: {exc}",
                status_code=0,
                body=payload,
            ) from exc


__all__ = [
    "BotProvider",
    "LiveSession",
    "MeetingPlatform",
]
