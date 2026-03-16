"""Background memory monitor that alerts via Sentry when usage exceeds a threshold."""

import logging
import threading

import psutil
import sentry_sdk

logger = logging.getLogger(__name__)

_peak_usage: float = 0.0
_peak_lock = threading.Lock()
_memory_limit: int | None = None


def _read_cgroup_memory_limit() -> int | None:
    """Read the container memory limit from cgroup filesystem.

    Returns:
        Memory limit in bytes, or None if not running in a container.
    """
    # cgroup v2
    try:
        value = open("/sys/fs/cgroup/memory.max").read().strip()
        if value != "max":
            return int(value)
    except (FileNotFoundError, ValueError):
        pass

    # cgroup v1
    try:
        value = int(open("/sys/fs/cgroup/memory/memory.limit_in_bytes").read().strip())
        # cgroup v1 returns a very large number when unlimited
        if value < 2**62:
            return value
    except (FileNotFoundError, ValueError):
        pass

    return None


def get_peak_memory_usage() -> tuple[float, int] | None:
    """Return (peak_ratio, limit_bytes), or None if no cgroup limit."""
    if _memory_limit is None:
        return None
    with _peak_lock:
        return _peak_usage, _memory_limit


def start_memory_monitor(threshold: float = 0.85, interval: float = 5.0) -> None:
    """Start a daemon thread that monitors memory usage.

    When RSS exceeds the threshold percentage of the container memory limit,
    logs a warning and sends a Sentry alert. Only alerts once per crossing
    to avoid spam.

    Args:
        threshold: Memory usage ratio (0.0-1.0) to trigger alert.
        interval: Seconds between checks.
    """
    global _memory_limit
    _memory_limit = _read_cgroup_memory_limit()
    if _memory_limit is None:
        logger.info("Memory monitor: no cgroup limit detected, skipping")
        return

    limit = _memory_limit

    logger.info(
        "Memory monitor: started (limit=%dMB, threshold=%.0f%%, interval=%.0fs)",
        limit // 1024 // 1024,
        threshold * 100,
        interval,
    )

    def _monitor() -> None:
        global _peak_usage
        process = psutil.Process()
        alerted = False

        while True:
            rss = process.memory_info().rss
            usage = rss / limit

            with _peak_lock:
                if usage > _peak_usage:
                    _peak_usage = usage

            if usage > threshold and not alerted:
                msg = (
                    f"Memory usage critical: {usage:.0%} "
                    f"({rss // 1024 // 1024}MB / {limit // 1024 // 1024}MB)"
                )
                logger.warning(msg)
                sentry_sdk.capture_message(msg, level="warning")
                alerted = True
            elif usage <= threshold:
                alerted = False

            threading.Event().wait(interval)

    t = threading.Thread(target=_monitor, daemon=True)
    t.start()
