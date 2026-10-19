package validation

import (
	"strings"
	"testing"
)

func TestQuoteSchemaRejectsMalformedIndependentEnvelope(t *testing.T) {
	valid := `{"schemaVersion":"1.0","modelDigest":"sha256:` + strings.Repeat("0", 64) + `","revision":0,"price":{"status":"range","currency":"CNY","minMinor":100,"maxMinor":200,"reason":null},"focus":null}`
	if err := Validate("quote", []byte(valid)); err != nil {
		t.Fatal(err)
	}
	for _, bad := range []string{`{}`, strings.Replace(valid, `"revision":0`, `"revision":9007199254740991.1`, 1), strings.Replace(valid, `"minMinor":100`, `"minMinor":-1`, 1), strings.Replace(valid, `"status":"range"`, `"status":"unavailable"`, 1), strings.Replace(valid, `"focus":null`, `"focus":null,"state":{}`, 1)} {
		if Validate("quote", []byte(bad)) == nil {
			t.Fatalf("accepted invalid Quote: %s", bad)
		}
	}
}
