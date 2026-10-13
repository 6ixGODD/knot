package knot

import (
	"encoding/json"
	"os"
	"testing"
)

func assertNativeReleased(t *testing.T) {
	t.Helper()
	// Test-only fixture command; never run these tests against a production SDK.
	m, raw, err := Compile([]byte("stats"))
	if err != nil {
		t.Fatal(err)
	}
	defer m.Close()
	var stats struct{ Buffers, Models, Freed int }
	if err = json.Unmarshal(raw, &stats); err != nil {
		t.Fatal(err)
	}
	if stats.Buffers != 0 || stats.Models != 0 || stats.Freed == 0 {
		t.Fatalf("native resources not released: %+v", stats)
	}
}

func TestNativeBoundary(t *testing.T) {
	if os.Getenv("KNOT_TEST_NATIVE_LIB") == "" {
		t.Fatal("test-only native fixture required")
	}
	m, create, err := Compile([]byte("{}"))
	if err != nil {
		t.Fatal(err)
	}
	defer m.Close()
	if !json.Valid(create) {
		t.Fatal("invalid create result")
	}
	raw, err := m.Initial()
	if err != nil {
		t.Fatal(err)
	}
	var result struct {
		State    json.RawMessage `json:"state"`
		Snapshot struct {
			Price struct {
				Total int64 `json:"totalMinor"`
			} `json:"price"`
		} `json:"snapshot"`
	}
	if err = json.Unmarshal(raw, &result); err != nil {
		t.Fatal(err)
	}
	if result.Snapshot.Price.Total != 282800 {
		t.Fatal("fixture was not copied correctly")
	}
	if _, err = m.Apply(result.State, []byte(`{"schemaVersion":"1.0","type":"clear_answer","questionId":"material","expectedRevision":5}`)); err != nil {
		t.Fatal(err)
	}
	if _, err = m.Evaluate(result.State); err != nil {
		t.Fatal(err)
	}
	alias := *m
	alias.Close()
	m.Close()
	if _, err = m.Initial(); err == nil {
		t.Fatal("closed handle accepted")
	}
	if result.Snapshot.Price.Total != 282800 {
		t.Fatal("native free corrupted owning result")
	}
	assertNativeReleased(t)
}
func TestErrors(t *testing.T) {
	for _, input := range []string{"no-json", "bad-json"} {
		m, _, err := Compile([]byte(input))
		if m != nil {
			m.Close()
		}
		if err == nil {
			t.Fatalf("expected %s failure", input)
		}
		e, ok := err.(*Error)
		if !ok {
			t.Fatal("error type")
		}
		if input == "no-json" && (e.Status != 1999 || e.Code != "INTERNAL") {
			t.Fatal("lost INTERNAL")
		}
	}
	if _, _, err := Compile(make([]byte, 4194305)); err == nil {
		t.Fatal("input limit ignored")
	}
	assertNativeReleased(t)
}
