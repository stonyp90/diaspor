"""Smoke tests: the package loads and the clients instantiate.

These tests make no real network calls. They confirm that:

* ``import diaspor`` succeeds.
* :class:`diaspor.Client` and :class:`diaspor.AsyncClient` can be
  constructed with the documented constructor.
* The configured ``api_key``, ``base_url``, and ``timeout`` surface back
  on the instance as attributes.
* The error and model types are importable from the top-level package.

A failure here means the SDK is broken before any wire traffic happens —
that is the cheapest possible check to fail loudly on.
"""

from __future__ import annotations

import diaspor
from diaspor import AsyncClient, Client


def test_package_exposes_version() -> None:
    assert isinstance(diaspor.__version__, str)
    assert diaspor.__version__ == "0.1.0a1"


def test_sync_client_constructs_with_defaults() -> None:
    client = Client(api_key="dk_test_synthetic_key_for_unit_tests")
    try:
        assert client.api_key == "dk_test_synthetic_key_for_unit_tests"
        assert client.base_url == "https://api.diaspor.io"
        assert client.timeout == 30.0
    finally:
        client.close()


def test_sync_client_respects_explicit_overrides() -> None:
    client = Client(
        api_key="dk_test_other",
        base_url="https://staging-api.diaspor.io/",  # trailing slash should be stripped
        timeout=5.0,
    )
    try:
        assert client.base_url == "https://staging-api.diaspor.io"
        assert client.timeout == 5.0
    finally:
        client.close()


def test_sync_client_is_a_context_manager() -> None:
    with Client(api_key="dk_test_ctx") as client:
        assert client.api_key == "dk_test_ctx"


def test_async_client_constructs_with_defaults() -> None:
    # Async client constructor is sync; no event loop needed to instantiate.
    client = AsyncClient(api_key="dk_test_synthetic_async")
    try:
        assert client.api_key == "dk_test_synthetic_async"
        assert client.base_url == "https://api.diaspor.io"
        assert client.timeout == 30.0
    finally:
        # AsyncClient.aclose() is async; ``httpx.AsyncClient`` is safe to
        # leave to GC for the purposes of this construction-only test.
        pass


def test_empty_api_key_is_rejected() -> None:
    import pytest

    with pytest.raises(ValueError):
        Client(api_key="")
    with pytest.raises(ValueError):
        AsyncClient(api_key="")


def test_error_classes_importable() -> None:
    # Just import them; instantiation isn't required for this smoke check.
    from diaspor import (
        ApiError,
        DiasporError,
        NotImplementedYetError,
        RateLimitedError,
        VerticalRefusedError,
    )

    # Class hierarchy invariants worth pinning so refactors don't silently
    # break ``except DiasporError`` clauses in user code.
    assert issubclass(ApiError, DiasporError)
    assert issubclass(RateLimitedError, ApiError)
    assert issubclass(VerticalRefusedError, ApiError)
    assert issubclass(NotImplementedYetError, ApiError)


def test_live_session_module_loads() -> None:
    # The streaming module imports ``websockets``; this confirms the
    # optional websocket transport is installable alongside the SDK.
    from diaspor import LiveSession

    assert LiveSession.whip is not None
    assert LiveSession.meeting_bot is not None
