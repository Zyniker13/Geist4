#!/usr/bin/env python3
"""Temporary CI diagnostic: publish the tail of a failed check log as
commit statuses so the failing stage and message can be read through
the unauthenticated commit-status API. Remove once CI is diagnosed."""
import json
import sys
import urllib.error
import urllib.request

lines = [line[:130] for line in open("check.log", encoding="utf-8", errors="replace").read().splitlines()[-6:]]
sha, repo, token, os_name = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
posted = 0
last_error = ""
for i, line in enumerate(lines):
    body = json.dumps({"state": "failure", "context": f"ci/{os_name}/tail{i}", "description": line}).encode()
    req = urllib.request.Request(
        f"https://api.github.com/repos/{repo}/statuses/{sha}",
        data=body,
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    try:
        urllib.request.urlopen(req)
        posted += 1
    except urllib.error.HTTPError as e:
        last_error = f"HTTP {e.code}"
    except Exception as e:  # noqa: BLE001 - diagnostic must never fail the job
        last_error = str(e)[:60]
summary = json.dumps({"state": "failure", "context": "ci/diag", "description": f"posted={posted} last={last_error}"}).encode()
req = urllib.request.Request(
    f"https://api.github.com/repos/{repo}/statuses/{sha}",
    data=summary,
    headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
)
try:
    urllib.request.urlopen(req)
except Exception:
    pass
