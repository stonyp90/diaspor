"""Exception hierarchy for the Diaspor SDK.

All exceptions raised by the SDK inherit from :class:`DiasporError` so callers
can install a single ``except`` clause at the boundary of their application
and still get useful diagnostics. HTTP-level failures surface as
:class:`ApiError` (or a more specific subclass); transport-level failures
(``httpx.TransportError``, etc.) propagate as-is.
"""

from __future__ import annotations

from typing import Any


class DiasporError(Exception):
    """Base class for all SDK errors.

    Catching this lets a caller treat any SDK-originated failure uniformly
    without needing to know the specific subclass.
    """

    def __init__(self, message: str, *, details: dict[str, Any] | None = None) -> None:
        super().__init__(message)
        self.message: str = message
        self.details: dict[str, Any] = dict(details) if details else {}


class ApiError(DiasporError):
    """The API returned a non-2xx HTTP status.

    Carries the HTTP status code, the parsed error body (if the server sent
    JSON), and the request_id from the ``X-Diaspor-Request-Id`` response
    header when present. ``request_id`` is the right thing to forward to
    support — the API correlates logs by that field.
    """

    def __init__(
        self,
        message: str,
        *,
        status_code: int,
        request_id: str | None = None,
        body: Any = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(message, details=details)
        self.status_code: int = status_code
        self.request_id: str | None = request_id
        self.body: Any = body

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        rid = f" (request_id={self.request_id})" if self.request_id else ""
        return f"[HTTP {self.status_code}]{rid} {self.message}"


class RateLimitedError(ApiError):
    """HTTP 429.

    The ``retry_after_seconds`` attribute reflects the server's
    ``Retry-After`` response header (parsed as seconds). It is ``None`` if
    the server did not include one — in that case the caller should fall
    back to its own exponential backoff.
    """

    def __init__(
        self,
        message: str,
        *,
        status_code: int = 429,
        request_id: str | None = None,
        retry_after_seconds: float | None = None,
        body: Any = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(
            message,
            status_code=status_code,
            request_id=request_id,
            body=body,
            details=details,
        )
        self.retry_after_seconds: float | None = retry_after_seconds


class VerticalRefusedError(ApiError):
    """The API refused this call because the key's vertical is forbidden.

    Raised when the API gateway refuses to invoke an endpoint because the
    customer's API-key vertical attestation does not permit it. The
    credibility endpoint refuses keys attested as ``forensic``, ``hiring``,
    ``insurance``, ``law_enforcement``, ``eu_workplace``, and
    ``eu_education``. EU workplace and education are blocked under the EU
    AI Act (effective August 2026); the other four are policy refusals
    documented in the Acceptable Use Policy.

    This is a *deliberate* server-side refusal, not a misconfiguration. The
    correct remedy is either (a) using the right SDK method for your
    declared vertical, or (b) requesting an updated attestation through
    your account contact — not retrying with a different request shape.
    """

    def __init__(
        self,
        message: str,
        *,
        status_code: int = 403,
        request_id: str | None = None,
        vertical: str | None = None,
        endpoint: str | None = None,
        body: Any = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(
            message,
            status_code=status_code,
            request_id=request_id,
            body=body,
            details=details,
        )
        self.vertical: str | None = vertical
        self.endpoint: str | None = endpoint


class NotImplementedYetError(ApiError):
    """The server returned HTTP 501 because this backend route is not wired yet.

    The SDK ships ahead of the full server build-out, so some endpoints are
    intentionally stubbed. A 501 from a documented route means "the wire
    contract is final but the implementation has not landed in production
    yet" — callers can treat it as a not-yet-available signal rather than
    a permanent failure, and can subscribe to the changelog at
    https://developers.diaspor.io/changelog to learn when it goes live.
    """

    def __init__(
        self,
        message: str,
        *,
        status_code: int = 501,
        request_id: str | None = None,
        endpoint: str | None = None,
        body: Any = None,
        details: dict[str, Any] | None = None,
    ) -> None:
        super().__init__(
            message,
            status_code=status_code,
            request_id=request_id,
            body=body,
            details=details,
        )
        self.endpoint: str | None = endpoint


__all__ = [
    "ApiError",
    "DiasporError",
    "NotImplementedYetError",
    "RateLimitedError",
    "VerticalRefusedError",
]
