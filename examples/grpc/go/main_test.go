package main

import (
	knotv1 "knot.local/grpc-example/gen/knot/v1"
	"testing"
)

func TestMissingAndInvalidOutcome(t *testing.T) {
	for _, reply := range []*knotv1.ConfigurationResponse{nil, {}, {Outcome: &knotv1.ConfigurationResponse_ResultJson{}}, {Outcome: &knotv1.ConfigurationResponse_ResultJson{ResultJson: []byte("null")}}, {Outcome: &knotv1.ConfigurationResponse_ErrorJson{ErrorJson: []byte("{}")}}} {
		if _, err := result(reply); err == nil {
			t.Fatal("invalid outcome accepted")
		}
	}
}

func TestRejectIncompleteError(t *testing.T) {
	for _, raw := range []string{`{"error":{"code":"REVISION_CONFLICT"}}`, `{"schemaVersion":"1.0","error":{"code":"REVISION_CONFLICT"}}`} {
		if _, err := errorCode([]byte(raw)); err == nil {
			t.Fatal("incomplete error accepted")
		}
	}
}

func TestRejectInvalidNestedResult(t *testing.T) {
	raw := []byte(`{"schemaVersion":"1.0","state":{},"snapshot":{},"changes":{},"diagnostics":[]}`)
	if _, err := result(&knotv1.ConfigurationResponse{Outcome: &knotv1.ConfigurationResponse_ResultJson{ResultJson: raw}}); err == nil {
		t.Fatal("schema-invalid nested result accepted")
	}
}

func TestRejectInvalidErrorFields(t *testing.T) {
	for _, raw := range []string{
		`{"schemaVersion":"1.0","error":{"code":"UNKNOWN","message":"bad","path":"","ruleIds":[],"details":{}}}`,
		`{"schemaVersion":"1.0","error":{"code":"BAD_JSON","message":"","path":"","ruleIds":[],"details":{}}}`,
	} {
		if _, err := errorCode([]byte(raw)); err == nil {
			t.Fatal("schema-invalid error accepted")
		}
	}
}

func TestResponseFormats(t *testing.T) {
	for _, input := range []string{"0", "-0.000000001", "9007199254740991", "1.123456789000"} {
		if err := decimalFormat(input); err != nil {
			t.Fatalf("valid decimal %q: %v", input, err)
		}
	}
	for _, input := range []string{"", "+1", "1.", ".1", "1e2", "9007199254740991.1", "0.0000000001"} {
		if decimalFormat(input) == nil {
			t.Fatalf("invalid decimal %q accepted", input)
		}
	}
	for _, input := range []string{"", "a\x7f", "a\u0085"} {
		if idFormat(input) == nil {
			t.Fatalf("invalid id %q accepted", input)
		}
	}
	if code, err := errorCode([]byte(`{"schemaVersion":"1.0","error":{"code":"BAD_JSON","message":"bad","path":"","ruleIds":[],"details":{}}}`)); err != nil || code != "BAD_JSON" {
		t.Fatalf("valid error rejected: %v", err)
	}
}
