"""Exercise a language client against an existing local Speaker service.

Run from the repository root. Arguments after -- select the client program.
Each RPC writes its request files, stdout and stderr to a new output directory.
"""
import argparse
import json
from pathlib import Path
import subprocess

DIGEST = "sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--endpoint", default="127.0.0.1:50051")
    parser.add_argument("client", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    client = args.client[1:] if args.client[:1] == ["--"] else args.client
    if not client:
        parser.error("provide client command after --")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    calls = []

    def call(method, payload=None, event=None, expected=0, digest=DIGEST):
        prefix = out / f"{len(calls):02d}-{method}"
        command = [*client, method, "--plaintext", "--endpoint", args.endpoint, "--digest", digest]
        for flag, value in [("input", payload), ("event", event)]:
            if value is not None:
                path = prefix.with_suffix("." + flag + ".json")
                path.write_text(json.dumps(value), encoding="utf-8")
                command.extend(["--" + flag, str(path)])
        result = subprocess.run(command, capture_output=True, timeout=30)
        prefix.with_suffix(".stdout").write_bytes(result.stdout)
        prefix.with_suffix(".stderr").write_bytes(result.stderr)
        calls.append(dict(method=method, exitCode=result.returncode, expected=expected))
        (out / "calls.json").write_text(json.dumps(calls, indent=2), encoding="utf-8")
        if result.returncode != expected:
            raise RuntimeError(f"{method}: expected {expected}, got {result.returncode}; see {prefix}.stderr")
        if expected == 1:
            return None
        value = json.loads(result.stderr if expected == 2 else result.stdout)
        if value.get("schemaVersion") != "1.0":
            raise RuntimeError("invalid envelope")
        return value

    result = call("Initial")
    state = result["state"]
    assert state["revision"] == 0
    quote = call("Quote", dict(schemaVersion="1.0", state=state))
    assert (quote["price"]["status"], quote["price"]["minMinor"], quote["price"]["maxMinor"]) == ("range", 119900, 302700)
    choices = [("speaker_type", "active"), ("material", "aluminum"),
               ("panel_color", "#315A79"), ("power", "200w"), ("extras", ["led"])]
    for question, answer in choices:
        event = dict(schemaVersion="1.0", type="set_answer", questionId=question,
                     value=answer, expectedRevision=state["revision"])
        result = call("Apply", state, event)
        state = result["state"]
        quote = call("Quote", dict(schemaVersion="1.0", state=state))
    assert quote["price"]["status"] == "final" and quote["price"]["minMinor"] == quote["price"]["maxMinor"] == 282800
    assert call("Evaluate", state)["state"] == state
    stale = call("Apply", state, event, expected=2)
    assert stale["error"]["code"] == "REVISION_CONFLICT"
    # Navigating to a previous question is a Quote focus, not a State mutation.
    focused = call("Quote", dict(schemaVersion="1.0", state=state, focusQuestionId="material"))
    assert focused["revision"] == state["revision"]
    event = dict(schemaVersion="1.0", type="set_answer", questionId="material",
                 value="walnut", expectedRevision=state["revision"])
    result = call("Apply", state, event)
    state = result["state"]
    assert "power" not in state["answers"] and "extras" not in state["answers"]
    quote = call("Quote", dict(schemaVersion="1.0", state=state))
    assert (quote["price"]["status"], quote["price"]["minMinor"], quote["price"]["maxMinor"]) == ("range", 159900, 232700)
    invalid = dict(schemaVersion="1.0", type="set_answer", questionId="power",
                   value="200w", expectedRevision=state["revision"])
    error = call("Apply", state, invalid, expected=2)
    assert error["error"]["code"] == "INVALID_OPTION"
    assert call("Evaluate", state)["state"] == state
    call("Initial", expected=1, digest="sha256:" + "0" * 64)
    print(json.dumps(dict(passed=True, calls=len(calls), finalBeforeEditMinor=282800,
                         rangeAfterEdit=quote["price"], output=str(out)), indent=2))


if __name__ == "__main__":
    main()
