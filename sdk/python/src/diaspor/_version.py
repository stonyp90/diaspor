"""Single source of truth for the package version.

Kept in its own module so build backends, ``diaspor.__init__``, and the
``Client`` user-agent header can all import it without circular dependencies.
"""

from __future__ import annotations

__version__: str = "0.1.0a1"
