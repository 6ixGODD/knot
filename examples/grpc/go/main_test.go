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
