"""Call one KNOT RPC. See docs/grpc/clients.md for setup and TLS."""
import argparse
import json
from pathlib import Path
import sys

import grpc
from knot.v1 import configuration_pb2 as pb
from knot.v1 import configuration_pb2_grpc as rpc


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("method", choices=["Initial", "Apply", "Evaluate", "Quote"])
    p.add_argument("--digest", required=True)
    p.add_argument("--endpoint", default="127.0.0.1:50051")
    p.add_argument("--input", type=Path, help="State JSON, or QuoteRequest JSON for Quote")
    p.add_argument("--event", type=Path)
    p.add_argument("--plaintext", action="store_true")
    p.add_argument("--ca", type=Path)
    p.add_argument("--cert", type=Path)
    p.add_argument("--key", type=Path)
    a = p.parse_args()
    options = [("grpc.max_receive_message_length", 4195328), ("grpc.enable_retries", 0)]
    if a.plaintext:
        if any([a.ca, a.cert, a.key]):
            p.error("do not combine plaintext and TLS options")
        channel = grpc.insecure_channel(a.endpoint, options=options)
    else:
        if not all([a.ca, a.cert, a.key]):
            p.error("supply --ca, --cert and --key, or explicitly select --plaintext")
        credentials = grpc.ssl_channel_credentials(a.ca.read_bytes(), a.key.read_bytes(), a.cert.read_bytes())
        channel = grpc.secure_channel(a.endpoint, credentials, options=options)
    fields = {"model_digest": a.digest}
    if a.method != "Initial":
        if a.input is None:
            p.error("--input is required")
        fields["quote_json" if a.method == "Quote" else "state_json"] = a.input.read_bytes()
    if a.method == "Apply":
        if a.event is None:
            p.error("--event is required")
        fields["event_json"] = a.event.read_bytes()
    with channel:
        response = getattr(rpc.ConfigurationServiceStub(channel), a.method)(
            getattr(pb, a.method + "Request")(**fields), timeout=10)
    branch = response.WhichOneof("outcome")
    success = "quote_json" if a.method == "Quote" else "result_json"
    if branch not in (success, "error_json"):
        raise ValueError("missing or unknown response outcome")
    value = json.loads(getattr(response, branch).decode("utf-8"))
    if not isinstance(value, dict) or value.get("schemaVersion") != "1.0":
        raise ValueError("invalid response envelope")
    print(json.dumps(value, ensure_ascii=True), file=sys.stderr if branch == "error_json" else sys.stdout)
    return 2 if branch == "error_json" else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (grpc.RpcError, ValueError, OSError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
