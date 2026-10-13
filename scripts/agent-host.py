#!/usr/bin/env python3
"""Receive company work in a user-managed external Agent process (stdlib only)."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request


class HostError(Exception):
    """A fixed, credential-free message suitable for display."""


class Stopped(HostError):
    pass


class SafeParser(argparse.ArgumentParser):
    def error(self, message):
        raise HostError("Invalid options; use --help for the supported arguments.")


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def contains_secret(value, token):
    if isinstance(value, str):
        return token in value
    if isinstance(value, dict):
        return any(contains_secret(key, token) or contains_secret(item, token)
                   for key, item in value.items())
    if isinstance(value, list):
        return any(contains_secret(item, token) for item in value)
    return False


class Service:
    def __init__(self, url, company, token, timeout):
        parsed = urllib.parse.urlsplit(url)
        if (
            parsed.scheme not in ("http", "https")
            or not parsed.hostname
            or parsed.username is not None
            or parsed.password is not None
            or parsed.query
            or parsed.fragment
            or token in urllib.parse.unquote(url)
        ):
            raise HostError("Use a service URL without credentials, query or fragment.")
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,128}", company):
            raise HostError("A valid company ID is required.")
        self.url = url.rstrip("/")
        self.base = self.url + "/api/v1/companies/" + company
        self.token = token
        self.timeout = timeout

    def request(self, method, route, body=None, query=None):
        url = self.base + route
        if query:
            url += "?" + urllib.parse.urlencode(query)
        data = None if body is None else json.dumps(body).encode("utf-8")
        if self.token in urllib.parse.unquote(url) or (
            data is not None and self.token.encode("utf-8") in data
        ):
            raise HostError("Credentials must stay in the environment and authorization header.")
        request = urllib.request.Request(
            url,
            data=data,
            method=method,
            headers={
                "Authorization": "Bearer " + self.token,
                "Accept": "application/json",
                "Content-Type": "application/json",
            },
        )
        try:
            opener = urllib.request.build_opener(NoRedirect())
            with opener.open(request, timeout=self.timeout) as response:
                raw = response.read(8 * 1024 * 1024 + 1)
            if len(raw) > 8 * 1024 * 1024:
                raise HostError("Service response is too large; review the connection.")
            value = json.loads(raw.decode("utf-8"))
            if not isinstance(value, dict):
                raise ValueError()
            return value
        except urllib.error.HTTPError as error:
            # Neither server error text nor redirect targets are safe log output.
            if error.code in (401, 403):
                raise HostError("Authorization unavailable; stopped.") from None
            if error.code == 409:
                raise HostError("Connection or saved work changed; review the current state.") from None
            raise HostError("Service request failed; review the connection.") from None
        except (urllib.error.URLError, OSError, TimeoutError):
            raise HostError("Service connection failed; stopped without replaying work.") from None
        except (ValueError, UnicodeError):
            raise HostError("Service response is invalid; review the connection.") from None


class Journal:
    """Local deduplication across restarts; no task text or credentials are stored."""

    def __init__(self, path):
        self.path = Path(path)
        self.entries = {}
        try:
            if self.path.exists():
                saved = json.loads(self.path.read_text(encoding="utf-8"))
                entries = saved["requests"]
                if saved["version"] != 1 or not isinstance(entries, dict):
                    raise ValueError()
                states = {"received", "running", "completed", "failed", "interrupted"}
                if any(not re.fullmatch(r"[a-f0-9]{64}", key) or state not in states
                       for key, state in entries.items()):
                    raise ValueError()
                self.entries = entries
        except (OSError, ValueError, KeyError, TypeError):
            raise HostError("Local receipt state is unavailable; review it before reconnecting.") from None

    @staticmethod
    def key(request_key):
        return hashlib.sha256(request_key.encode("utf-8")).hexdigest()

    def get(self, request_key):
        return self.entries.get(self.key(request_key))

    def mark(self, request_key, state):
        self.entries[self.key(request_key)] = state
        temporary = None
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(
                mode="w", encoding="utf-8", dir=self.path.parent, delete=False
            ) as output:
                temporary = output.name
                json.dump({"version": 1, "requests": self.entries}, output)
                output.flush()
                os.fsync(output.fileno())
            os.replace(temporary, self.path)
        except OSError:
            raise HostError("Local receipt state could not be saved; work was not replayed.") from None
        finally:
            if temporary and os.path.exists(temporary):
                os.unlink(temporary)


def completed(kind, before, after):
    """Require the relevant authoritative transition, not an unrelated write."""
    if after.get("company_id") != before.get("company_id"):
        return False
    old = before.get("current_event") or {}
    if kind == "prepare_event":
        event = after.get("current_event") or {}
        return (
            before.get("phase") == "awaiting_event"
            and after.get("phase") in ("awaiting_choice", "awaiting_agent", "agent_processing")
            and bool(event.get("id"))
            and event.get("id") != old.get("id")
            and bool(event.get("data", {}).get("options"))
        )
    if kind == "process_selection":
        decision = (after.get("latest_result") or {}).get("decision") or {}
        result = decision.get("data") or {}
        return (
            before.get("phase") == "awaiting_agent"
            and bool(old.get("id"))
            and decision.get("id") == old.get("id")
            and result.get("status") == "resolved"
            and bool(result.get("resolution", {}).get("result_refs"))
        )
    return False


class Host:
    def __init__(self, options, token):
        self.options = options
        self.service = Service(options.service_url, options.company, token, options.request_timeout)
        try:
            self.argv = json.loads(options.handler_argv)
        except ValueError:
            raise HostError("Handler arguments must be a JSON array of strings.") from None
        if (not isinstance(self.argv, list) or not self.argv
                or any(not isinstance(value, str) or not value or "\x00" in value for value in self.argv)):
            raise HostError("Handler arguments must be a nonempty JSON array of strings.")
        if (token in options.label or token in options.company
                or any(token in value for value in self.argv)
                or (options.state_file and token in options.state_file)):
            raise HostError("Credentials must be supplied only through COMPANY_AGENT_TOKEN.")
        if (not options.label.strip() or len(options.label) > 120
                or any(ord(character) < 32 for character in options.label)):
            raise HostError("Provide a host label of 1 to 120 characters.")
        if not 1 <= options.lease_seconds <= 120:
            raise HostError("Lease seconds must be between 1 and 120.")
        if (not math.isfinite(options.handler_timeout) or not math.isfinite(options.request_timeout)
                or options.handler_timeout <= 0 or options.request_timeout < 1):
            raise HostError("Timeout values must be positive; request timeout must be at least one second.")
        scope = hashlib.sha256((self.service.url + "\n" + options.company).encode("utf-8")).hexdigest()
        path = options.state_file or Path.home() / ".transplane-company" / "agent-host" / (scope + ".json")
        self.journal = Journal(path)
        self.stop = threading.Event()
        self.heartbeat_error = None
        self.session = None
        self.heartbeat_thread = None
        self.active_key = None

    def heartbeat(self):
        interval = max(0.1, self.options.lease_seconds / 3)
        while not self.stop.wait(interval):
            try:
                result = self.service.request("POST", "/agent-link/connect", {
                    "label": self.options.label,
                    "lease_seconds": self.options.lease_seconds,
                    "session_id": self.session,
                })
                if result.get("session_id") != self.session or result.get("connected") is not True:
                    raise HostError("Connection changed; review the current state.")
            except HostError as error:
                self.heartbeat_error = error
                self.stop.set()
            except Exception:
                self.heartbeat_error = HostError("Connection renewal failed; stopped without replaying work.")
                self.stop.set()

    def check_stopped(self):
        if self.heartbeat_error:
            raise self.heartbeat_error
        if self.stop.is_set():
            raise Stopped("Disconnected; review any claimed work before reconnecting.")

    @staticmethod
    def terminate(process):
        if process.poll() is not None:
            return
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGTERM)
            else:
                process.terminate()
            process.wait(timeout=2)
        except (OSError, subprocess.TimeoutExpired):
            try:
                if os.name == "posix":
                    os.killpg(process.pid, signal.SIGKILL)
                else:
                    process.kill()
                process.wait(timeout=2)
            except (OSError, subprocess.TimeoutExpired):
                pass

    def run_handler(self, envelope):
        encoded = json.dumps(envelope, ensure_ascii=False).encode("utf-8")
        if contains_secret(envelope, self.service.token):
            raise HostError("Task contains credential text; stopped before handing it to the handler.")
        env = os.environ.copy()
        env["COMPANY_AGENT_TOKEN"] = self.service.token
        kwargs = {}
        if os.name == "posix":
            kwargs["start_new_session"] = True
        elif os.name == "nt":
            kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
            startup = subprocess.STARTUPINFO()
            startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            startup.wShowWindow = subprocess.SW_HIDE
            kwargs["startupinfo"] = startup
        try:
            process = subprocess.Popen(
                self.argv, shell=False, stdin=subprocess.PIPE,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env, **kwargs
            )
        except OSError:
            raise HostError("Handler could not start; review work before reconnecting.") from None
        deadline = time.monotonic() + self.options.handler_timeout
        payload = encoded
        try:
            while True:
                self.check_stopped()
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise HostError("Handler timed out; review work before reconnecting.")
                try:
                    process.communicate(input=payload, timeout=min(0.2, remaining))
                    break
                except subprocess.TimeoutExpired:
                    payload = None
            if process.returncode != 0:
                raise HostError("Handler stopped without confirmation; review work before reconnecting.")
        finally:
            self.terminate(process)

    def receive(self, response):
        work = response.get("work")
        if work is None:
            return
        if not isinstance(work, dict):
            raise HostError("Pending work is invalid; review the connection.")
        kind, key, receipt = (work.get(field) for field in ("kind", "request_key", "receipt_id"))
        play = response.get("play")
        if (kind not in ("prepare_event", "process_selection") or not isinstance(key, str)
                or not key or not isinstance(receipt, str) or not receipt
                or not isinstance(play, dict) or play.get("company_id") != self.options.company
                or not isinstance(play.get("player_output_policy"), dict)):
            raise HostError("Pending work or player guidance is invalid; review the connection.")
        previous = self.journal.get(key)
        if previous is not None and previous != "completed":
            raise HostError("This work was already received; review it before another attempt.")
        if previous is None:
            self.journal.mark(key, "received")
        ack = self.service.request("POST", "/agent-link/ack", {
            "session_id": self.session, "request_key": key, "receipt_id": receipt,
        })
        if (ack.get("acknowledged") is not True or ack.get("request_key") != key
                or ack.get("receipt_id") != receipt):
            raise HostError("Receipt was not confirmed; review the current work.")
        if previous == "completed":
            return
        if ack.get("replayed") is True:
            raise HostError("This work was already acknowledged; review it before another attempt.")
        print("Received.", flush=True)
        self.active_key = key
        self.journal.mark(key, "running")
        envelope = {
            "service_url": self.service.url, "company_id": self.options.company,
            "kind": kind, "request_key": key, "play": play,
            "policy": play["player_output_policy"],
        }
        try:
            self.run_handler(envelope)
            self.check_stopped()
            after = self.service.request("GET", "/play")
            if not completed(kind, play, after):
                raise HostError("Handler ended without confirmed progress; review work before reconnecting.")
            self.journal.mark(key, "completed")
            print("Completed.", flush=True)
        except (HostError, KeyboardInterrupt):
            self.journal.mark(key, "interrupted" if self.stop.is_set() else "failed")
            raise
        finally:
            self.active_key = None

    def run(self):
        try:
            connected = self.service.request("POST", "/agent-link/connect", {
                "label": self.options.label, "lease_seconds": self.options.lease_seconds,
            })
            self.session = connected.get("session_id")
            if not isinstance(self.session, str) or not self.session or connected.get("connected") is not True:
                raise HostError("Connection was not confirmed.")
            print("Connected.", flush=True)
            self.heartbeat_thread = threading.Thread(target=self.heartbeat, daemon=True)
            self.heartbeat_thread.start()
            revision = None
            request_key = None
            while True:
                self.check_stopped()
                query = {"session_id": self.session, "wait_ms": 0 if self.options.once else 25000}
                if revision is not None:
                    query["after_revision"] = revision
                if request_key is not None:
                    query["after_request_key"] = request_key
                response = self.service.request("GET", "/agent-link/pending-work", query=query)
                self.check_stopped()
                self.receive(response)
                revision = response.get("data_revision")
                request_key = (response.get("link") or {}).get("request_key")
                if self.options.once:
                    return 0
                # A server may return early without a changed ledger. Avoid busy polling.
                self.stop.wait(0.05)
        finally:
            self.stop.set()
            if self.heartbeat_thread:
                self.heartbeat_thread.join(timeout=self.options.request_timeout + 1)
            if self.session:
                try:
                    self.service.request("POST", "/agent-link/disconnect", {"session_id": self.session})
                except HostError:
                    pass


def parser():
    result = SafeParser(description=__doc__)
    result.add_argument("--service-url", required=True, help="Service base URL, without credentials")
    result.add_argument("--company", required=True, help="Authorized company ID")
    result.add_argument("--label", required=True, help="Name shown for this external host")
    result.add_argument("--handler-argv", required=True, help="Owner-configured JSON array of executable arguments")
    result.add_argument("--lease-seconds", type=int, default=60, help="Lease duration (1–120, default 60)")
    result.add_argument("--handler-timeout", type=float, default=600, help="Handler timeout in seconds (default 600)")
    result.add_argument("--request-timeout", type=float, default=35, help="HTTP timeout in seconds (default 35)")
    result.add_argument("--state-file", help="Local deduplication receipt file; contains no task text or token")
    result.add_argument("--once", action="store_true", help="Check once, handle at most one task, then disconnect")
    return result


def main(argv=None):
    host = None
    previous_signals = {}
    try:
        options = parser().parse_args(argv)
        token = os.environ.get("COMPANY_AGENT_TOKEN", "")
        if not token or any(character.isspace() for character in token):
            raise HostError("Set COMPANY_AGENT_TOKEN in the external host's secure environment.")
        host = Host(options, token)
        if threading.current_thread() is threading.main_thread():
            def stop(signum, frame):
                host.stop.set()
            for name in ("SIGINT", "SIGTERM", "SIGBREAK"):
                value = getattr(signal, name, None)
                if value is not None:
                    previous_signals[value] = signal.signal(value, stop)
        return host.run()
    except Stopped as error:
        print(str(error), file=sys.stderr, flush=True)
        return 130
    except HostError as error:
        print(str(error), file=sys.stderr, flush=True)
        return 1
    except KeyboardInterrupt:
        if host:
            host.stop.set()
        print("Disconnected; review any claimed work before reconnecting.", file=sys.stderr, flush=True)
        return 130
    except Exception:
        # Do not emit exception text, child output, configuration or server bodies.
        print("External host stopped; review the connection and current work.", file=sys.stderr, flush=True)
        return 1
    finally:
        for value, handler in previous_signals.items():
            signal.signal(value, handler)


if __name__ == "__main__":
    raise SystemExit(main())
